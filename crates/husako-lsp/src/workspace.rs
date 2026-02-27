//! Workspace state: husako.toml + `_chains.meta.json` metadata loading.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tower_lsp::lsp_types::Url;

/// Field constraint metadata from `_chains.meta.json`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct FieldMeta {
    #[serde(rename = "type")]
    pub field_type: Option<String>,
    pub required: Option<bool>,
    pub pattern: Option<String>,
    pub values: Option<Vec<String>>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
}

/// A `*Chain` interface's field map, keyed by field name.
pub type ChainMeta = HashMap<String, FieldMeta>;

/// The full `_chains.meta.json` content.
/// Outer key = chain name (e.g. `"MetadataChain"`), inner = field map.
pub type ChainsMetaJson = HashMap<String, ChainMeta>;

/// Workspace state loaded from the project root.
pub struct Workspace {
    root: Option<PathBuf>,
    chains_meta: ChainsMetaJson,
    documents: HashMap<String, String>,
}

impl Workspace {
    pub fn new() -> Self {
        Self {
            root: None,
            chains_meta: HashMap::new(),
            documents: HashMap::new(),
        }
    }

    /// Load workspace state from the given project root.
    pub async fn load(&mut self, root: PathBuf) {
        self.root = Some(root.clone());
        self.reload_chains_meta(&root).await;
        self.refresh_tsconfig(&root).await;
    }

    /// Write (or overwrite) `tsconfig.json` at the project root.
    ///
    /// tsconfig.json is a husako-managed artifact. Opening a `.husako` file in
    /// the editor triggers this so the IDE always has up-to-date path mappings,
    /// even without a prior `husako gen` run. Errors are non-fatal — the LSP
    /// degrades gracefully.
    async fn refresh_tsconfig(&self, root: &Path) {
        let config = husako_config::load(root).ok().flatten();
        let plugin_paths = husako_core::scan_installed_plugin_paths(root);
        let content = husako_core::build_tsconfig_content(config.as_ref(), &plugin_paths);
        if let Ok(json) = serde_json::to_string_pretty(&content) {
            let _ = tokio::fs::write(root.join("tsconfig.json"), json + "\n").await;
        }
    }

    /// Reload `_chains.meta.json` from `.husako/types/_chains.meta.json`.
    /// Silently succeeds with empty metadata when the file is absent.
    pub async fn reload_chains_meta(&mut self, root: &Path) {
        let meta_path = root.join(".husako/types/_chains.meta.json");
        match tokio::fs::read_to_string(&meta_path).await {
            Ok(content) => match serde_json::from_str::<ChainsMetaJson>(&content) {
                Ok(meta) => self.chains_meta = meta,
                Err(_) => self.chains_meta = HashMap::new(),
            },
            Err(_) => {
                // File absent or unreadable — use empty metadata.
                // The LSP degrades gracefully: no schema-derived completions,
                // no RequiredFieldCheck errors.
                self.chains_meta = HashMap::new();
            }
        }
    }

    /// Return the project root path, if known.
    pub fn root(&self) -> Option<PathBuf> {
        self.root.clone()
    }

    /// Return the full `_chains.meta.json` map.
    pub fn chains_meta(&self) -> &ChainsMetaJson {
        &self.chains_meta
    }

    /// Return all fields for a given chain name.
    pub fn chain_fields(&self, chain: &str) -> Option<&ChainMeta> {
        self.chains_meta.get(chain)
    }

    /// Store the full text of an open document.
    pub fn set_document_text(&mut self, uri: &Url, text: String) {
        self.documents.insert(uri.to_string(), text);
    }

    /// Retrieve the text of an open document.
    pub fn get_document_text(&self, uri: &Url) -> Option<String> {
        self.documents.get(&uri.to_string()).cloned()
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lsp_writes_tsconfig_on_workspace_load() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();

        // No tsconfig.json before load
        assert!(!root.join("tsconfig.json").exists());

        let mut ws = Workspace::new();
        ws.load(root.clone()).await;

        // tsconfig.json must be created by load()
        assert!(root.join("tsconfig.json").exists());

        let content = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed["compilerOptions"]["paths"]["husako"].is_array());
        assert!(parsed["compilerOptions"]["paths"]["k8s/*"].is_array());
    }
}
