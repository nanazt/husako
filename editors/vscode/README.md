# husako VS Code Extension

Language support and IDE intelligence for `.husako` files.

## Features

- TypeScript syntax highlighting for `.husako` files
- Code completions filtered by builder context (chain method completions inside `.metadata()`, `.containers()`, etc.)
- Kubernetes quantity value completions inside `cpu("...")` and `memory("...")`
- Diagnostics: 7 rules covering missing required fields, invalid quantities, invalid image formats, enum violations, and the `husako.build()` contract
- Automatic reload when `husako gen` regenerates schema metadata

## Requirements

- `husako` binary on your `PATH` — the extension starts `husako lsp` as a subprocess

## Installation

The extension is not published to the VS Code Marketplace. Install from source:

1. Clone the repository
2. Build the extension:
   ```
   cd editors/vscode
   npm install
   npm run compile
   ```
3. Copy the `editors/vscode/` directory to your VS Code extensions folder:
   ```
   cp -r editors/vscode ~/.vscode/extensions/husako
   ```
4. Restart VS Code.

## How it works

The extension registers `.husako` as a language with TypeScript syntax highlighting.
When a `.husako` file is opened, it starts `husako lsp` as a subprocess and connects
to it over JSON-RPC on stdin/stdout.

It also watches `.husako/types/_chains.meta.json` — when `husako gen` runs and updates
this file, the extension signals the server to reload its schema metadata so completions
and diagnostics reflect the latest generated types.

## Running husako gen

Run `husako gen` (or `husako gen --skip-k8s`) before opening `.husako` files to enable
the full set of completions and diagnostic rules. Without the generated schema metadata,
context-aware completions and schema-derived diagnostics are unavailable.
