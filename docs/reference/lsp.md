# LSP Setup

The husako LSP server provides IDE intelligence for `.husako` files.

It runs alongside the TypeScript language server, adding husako-specific capabilities without replacing TypeScript's type checking.

---

## What the LSP provides

- **Context-sensitive completions** — chain methods filtered by call-site context (`.metadata()` shows ObjectMeta methods, `.containers()` shows Container methods)
- **Quantity completions** — common values suggested inside `cpu("...")` and `memory("...")`
- **Auto-import** — when a chain starter name is typed, the LSP offers to add the matching import automatically
- **Diagnostic rules** — 7 rules covering required fields, invalid quantities, invalid image formats, enum value violations, pattern violations, and the `husako.build()` contract
- **Duplicate import suppression** — TypeScript flags duplicate identifiers when the same name is imported from two `k8s/*` modules; the husako LSP suppresses that warning for `.husako` files

---

## How it starts

The LSP server speaks JSON-RPC over stdin/stdout. Editor extensions start it as a subprocess:

```
husako lsp
```

This requires `husako` to be on your `PATH`.

---

## VS Code

The husako VS Code extension is in `editors/vscode/` in the husako repository.

Install it by building from source or by loading it as an unpacked extension.

The extension registers `.husako` as a TypeScript file and starts `husako lsp` automatically when a `.husako` file is opened.

### Manual configuration (without the extension)

If you prefer to configure VS Code manually, add to `settings.json`:

```json
{
  "files.associations": { "*.husako": "typescript" }
}
```

Then configure the LSP client of your choice to run `husako lsp` for the `husako` filetype.

---

## Zed

The husako Zed extension is in `editors/zed/` in the husako repository.

Build the extension with `cargo build --release` and load it in Zed's extension settings.

The extension registers `.husako` with TypeScript syntax highlighting and starts `husako lsp` as a secondary language server.

---

## Neovim

Add to your Neovim config (lua):

```lua
vim.lsp.start({
  name = "husako",
  cmd = { "husako", "lsp" },
  filetypes = { "husako" },
  root_dir = vim.fs.dirname(vim.fs.find({ "husako.toml" }, { upward = true })[1]),
})

-- Associate .husako extension with the husako filetype
vim.filetype.add({ extension = { husako = "husako" } })
```

For TypeScript syntax highlighting on `.husako` files, set the filetype to `typescript` or configure treesitter to use the TypeScript parser:

```lua
vim.filetype.add({ extension = { husako = "typescript" } })
```

---

## Auto-import

When you type a chain starter function name (e.g., `name`, `image`, `namespace`) and no matching import exists, the LSP suggests adding the import:

| Context | Typed | Import added |
|---------|-------|--------------|
| Inside `.metadata()` | `name` | `import { name } from "k8s/meta/v1"` |
| Inside `.containers()` | `name` | `import { name } from "k8s/core/v1"` |
| Inside `.containers()` | `image` | `import { image } from "k8s/core/v1"` |
| Any | `Deployment` | `import { Deployment } from "k8s/apps/v1"` |

If the same name is already imported from a different `k8s/*` module, the new import is added as a duplicate. Duplicate imports from different `k8s/*` modules are valid in `.husako` files — the call site determines which is used.

---

## Diagnostic rules

The LSP runs 7 diagnostic rules on every save (debounced ~300ms after the last edit):

| Rule | Severity | Trigger |
|------|----------|---------|
| RequiredFieldCheck | Error | Missing required OpenAPI field in a chain passed to a typed builder method |
| QuantityLiteralCheck | Error | Invalid Kubernetes quantity grammar in `cpu()` or `memory()` arguments |
| BuildContractCheck | Error | `husako.build()` called 0 or 2+ times in the file |
| ImageFormatCheck | Warning | Invalid OCI image reference format in an `image()` argument |
| PatternCheck | Warning | String argument violates the OpenAPI `pattern` constraint for that field |
| EnumValueCheck | Warning | Enum-typed argument uses a value outside the defined enum |
| RangeCheck | Warning | Numeric argument violates `minimum`/`maximum` schema constraint |

RequiredFieldCheck tracks chains through direct variable assignments in the same scope. It stays silent when it cannot determine with certainty that a required field is missing — `husako render` is the safety net.

---

## Workspace initialization

When the LSP receives the workspace root on startup, it writes a fresh `tsconfig.json` to the project root using the current `husako.toml` and installed plugin paths. This means path mappings are updated automatically every time a `.husako` file is opened — even without running `husako gen` first.

---

## Schema metadata

The LSP reads `_chains.meta.json` in `.husako/types/` for machine-readable constraint data (field types, required fields, enum values, patterns, ranges).

This file is generated alongside `_chains.d.ts` by `husako gen`. The LSP watches the file and reloads it automatically when `husako gen` is run.

Run `husako gen` (or `husako gen --skip-k8s`) before opening `.husako` files to enable the full set of diagnostic rules.
