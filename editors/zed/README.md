# husako Zed Extension

Language support and IDE intelligence for `.husako` files in Zed.

## Features

- TypeScript syntax highlighting for `.husako` files
- Code completions filtered by builder context (chain method completions inside `.metadata()`, `.containers()`, etc.)
- Kubernetes quantity value completions inside `cpu("...")` and `memory("...")`
- Diagnostics: 7 rules covering missing required fields, invalid quantities, invalid image formats, enum violations, and the `husako.build()` contract

## Requirements

- `husako` binary on your `PATH` — the extension starts `husako lsp` as a subprocess

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

The extension registers `.husako` as a language and starts `husako lsp` as a secondary
language server when a `.husako` file is opened. Zed handles the TypeScript syntax
highlighting; husako-lsp adds completions and diagnostics on top.

## Running husako gen

Run `husako gen` (or `husako gen --skip-k8s`) before opening `.husako` files to enable
the full set of completions and diagnostic rules. Without the generated schema metadata,
context-aware completions and schema-derived diagnostics are unavailable.
