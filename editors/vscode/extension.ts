import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
  // Start husako lsp as a subprocess communicating over stdio.
  const serverOptions: ServerOptions = {
    command: "husako",
    args: ["lsp"],
    transport: TransportKind.stdio,
  };

  const clientOptions: LanguageClientOptions = {
    // Activate for .husako files
    documentSelector: [{ scheme: "file", language: "husako" }],
    synchronize: {
      // Re-send diagnostics when _chains.meta.json changes (husako gen output)
      fileEvents: vscode.workspace.createFileSystemWatcher(
        "**/.husako/types/_chains.meta.json"
      ),
    },
  };

  client = new LanguageClient(
    "husako-lsp",
    "husako Language Server",
    serverOptions,
    clientOptions
  );

  client.start();
  context.subscriptions.push(client);
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
