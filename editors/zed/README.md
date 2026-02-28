# husako Zed Extension

Language support and IDE intelligence for `.husako` files in Zed.

## Features

- TypeScript syntax highlighting for `.husako` files
- Code completions filtered by builder context (chain method completions inside `.metadata()`, `.containers()`, etc.)
- Kubernetes quantity value completions inside `cpu("...")` and `memory("...")`
- Diagnostics: 7 rules covering missing required fields, invalid quantities, invalid image formats, enum violations, and the `husako.build()` contract

## Requirements

- `husako` binary on your `PATH` — the extension starts `husako lsp` as a subprocess
- `typescript-language-server` on your `PATH` — provides full TypeScript support (type checking, completions, go-to-definition):
  ```
  npm install -g typescript-language-server typescript
  ```

## Installation

The extension is not published to the Zed extension registry. Install from source:

1. Clone the repository
2. In Zed, open **Extensions** (`Cmd+Shift+X`), click **Install Dev Extension**, and
   select the `editors/zed/` directory.

Zed compiles the extension to WASM automatically on load.

### Manual build

To build the WASM binary yourself:

```
rustup target add wasm32-wasip1
cd editors/zed
cargo build --release --target wasm32-wasip1
```

## How it works

The extension registers `.husako` as a language and starts two language servers in parallel:

- **`typescript-language-server`** — handles type checking, completions, hover docs, and
  go-to-definition. Husako files are sent with `languageId: "typescript"` so the server
  treats them as TypeScript.
- **`husako-lsp`** (`husako lsp`) — adds husako-specific chain completions and diagnostic
  rules on top.

By default `husako-lsp` is listed first. To make TypeScript LSP primary (recommended for
go-to-definition and Find References), add to your Zed `settings.json`:

```json
{
  "languages": {
    "husako": {
      "language_servers": ["typescript-language-server", "husako-lsp", "..."]
    }
  }
}
```

## Running husako gen

Run `husako gen` (or `husako gen --skip-k8s`) before opening `.husako` files to enable
the full set of completions and diagnostic rules. Without the generated schema metadata,
context-aware completions and schema-derived diagnostics are unavailable.
