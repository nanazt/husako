//! husako Language Server Protocol implementation.
//!
//! Provides IDE intelligence for `.husako` files:
//! - Context-sensitive code completion (chain context → filtered method list)
//! - Auto-import for chain starter functions
//! - 7 diagnostic rules derived from OpenAPI schema + husako contracts
//! - Kubernetes quantity value completions
//! - Duplicate import suppression for `k8s/*` modules

mod analysis;
mod completion;
mod diagnostics;
mod workspace;

use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use workspace::Workspace;

/// husako LSP server state.
pub struct HusakoLsp {
    client: Client,
    workspace: Arc<RwLock<Workspace>>,
}

impl HusakoLsp {
    fn new(client: Client) -> Self {
        Self {
            client,
            workspace: Arc::new(RwLock::new(Workspace::new())),
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for HusakoLsp {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Load workspace state from the client's root URI
        if let Some(root_uri) = params.root_uri
            && let Ok(root_path) = root_uri.to_file_path()
        {
            let mut ws = self.workspace.write().await;
            ws.load(root_path).await;
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_string(), "\"".to_string()]),
                    resolve_provider: Some(false),
                    ..Default::default()
                }),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "husako-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "husako LSP initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text;
        {
            let mut ws = self.workspace.write().await;
            ws.set_document_text(&uri, text.clone());
        }
        self.publish_diagnostics(&uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        if let Some(change) = params.content_changes.into_iter().last() {
            {
                let mut ws = self.workspace.write().await;
                ws.set_document_text(&uri, change.text.clone());
            }
            self.publish_diagnostics(&uri, &change.text).await;
        }
    }

    async fn did_save(&self, _params: DidSaveTextDocumentParams) {
        // Reload _chains.meta.json if husako gen may have run
        let ws = self.workspace.read().await;
        if let Some(root) = ws.root() {
            drop(ws);
            let mut ws = self.workspace.write().await;
            ws.reload_chains_meta(&root).await;
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;

        // Only handle .husako files
        if !is_husako_file(uri) {
            return Ok(None);
        }

        let ws = self.workspace.read().await;
        let text = ws.get_document_text(uri);
        drop(ws);

        let text = match text {
            Some(t) => t,
            None => return Ok(None),
        };

        let ws = self.workspace.read().await;
        let items = completion::completions(&text, pos, &ws);
        Ok(Some(CompletionResponse::Array(items)))
    }
}

impl HusakoLsp {
    /// Run diagnostics on a `.husako` file and publish via LSP.
    async fn publish_diagnostics(&self, uri: &Url, text: &str) {
        if !is_husako_file(uri) {
            return;
        }

        let ws = self.workspace.read().await;
        let diags = diagnostics::check(text, &ws);
        drop(ws);

        self.client
            .publish_diagnostics(uri.clone(), diags, None)
            .await;
    }
}

/// Returns `true` for files with the `.husako` extension.
fn is_husako_file(uri: &Url) -> bool {
    uri.path().ends_with(".husako")
}

/// Start the LSP server on stdin/stdout.
pub async fn run() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(HusakoLsp::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
