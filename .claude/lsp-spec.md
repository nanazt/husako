# husako LSP Specification

This document defines the behavior of the husako Language Server Protocol (LSP) server. The LSP server provides IDE-level intelligence for `.husako` files, running alongside the TypeScript language server.

---

### 1. Overview

The husako LSP server (`husako-lsp` crate) speaks LSP over stdio. Started by the editor as a separate process when a `.husako` file is opened.

**Responsibilities:**
- Context-sensitive code completion (call site context → filtered method list)
- Kubernetes quantity value completions
- Auto-import for chain starter functions
- Diagnostic rules (7 rules derived from OpenAPI schema + husako contracts)
- Suppression of TypeScript duplicate-import errors for `k8s/*` modules

**Not responsible for:**
- TypeScript type checking (delegated to `tsserver` / TypeScript LSP)
- General JavaScript/TypeScript completions (delegated to TypeScript LSP)

**Crate:** `crates/husako-lsp/src/`

```
lib.rs          — tower-lsp server, stdio entry
analysis.rs     — oxc AST: context detection, chain analysis, data flow
diagnostics.rs  — 7 diagnostic rules
completion.rs   — context-filtered completions, auto-import, quantity completions
workspace.rs    — husako.toml + _chains.meta.json metadata loading
```

---

### 2. File Handling

husako LSP activates **only for `.husako` files**. Regular `.ts` files are handled by TypeScript LSP exclusively.

`.husako` files are parsed as TypeScript ESM modules (oxc_parser with TypeScript mode). The LSP reads the raw text buffer directly — no disk I/O for open files.

---

### 3. Context Detection

#### Algorithm

When a completion or diagnostic request is triggered, the LSP determines the **chain context** by walking up the oxc AST from the cursor position:

1. Start at the cursor's AST node
2. Walk up parent nodes
3. Find the nearest enclosing `CallExpression` that is a **builder method call** with a known schema-typed parameter (`.metadata()`, `.containers()`, `.tolerations()`, etc.)
4. The parameter type of that builder method determines the chain context

```
Deployment()
  .metadata(name("nginx").█)        → context: MetadataChain
  .containers([name("nginx").█])    → context: ContainerChain
  .tolerations([operator("█")])     → context: TolerationChain
```

#### Context Types

| Builder method | Chain context | Completion source |
|----------------|---------------|-------------------|
| `.metadata()` | `MetadataChain` | ObjectMeta fields |
| `.containers()` items | `ContainerChain` | Container fields |
| `.tolerations()` items | `TolerationChain` | Toleration fields |
| `.volumeMounts()` items | `VolumeMountChain` | VolumeMount fields |
| `.env()` items | `EnvVarChain` | EnvVar fields |
| `.ports()` items | `ContainerPortChain` | ContainerPort fields |
| Top-level / standalone var | `SpecFragment` or starter return type | All methods |

#### Standalone Variable Context

When a chain is assigned to a variable (not inside a builder method argument):

- `const a = namespace(); a.█` → `namespace()` returns `MetadataChain` → MetadataChain methods only
- `const a = name(); a.█` → `name()` returns `SpecFragment` → all methods shown
- Variable's inferred type comes from the starter function's return type in `_chains.d.ts`

---

### 4. Code Completion

#### Method Completions

Completions are filtered by context:
- Inside `.metadata()` argument → show only `MetadataChain` methods
- Inside `.containers([...])` item → show only `ContainerChain` methods
- Standalone (ambiguous) → show all chain methods

Each completion item includes:
- Method name
- Parameter types (from `_chains.d.ts`)
- JSDoc description (from OpenAPI `description` field)
- Schema constraint annotations (`@required`, enum values)

#### Auto-Import

When a chain starter function name is typed and no matching import exists, the LSP suggests an import edit:

| Context | Input | Import added |
|---------|-------|--------------|
| `.metadata()` | `name` | `import { name } from "k8s/meta/v1"` |
| `.containers()` | `name` | `import { name } from "k8s/core/v1"` |
| `.containers()` | `image` | `import { image } from "k8s/core/v1"` |
| any | `Deployment` | `import { Deployment } from "k8s/apps/v1"` |

If the same name is already imported from a different `k8s/*` module, the new import is added as a duplicate (allowed in `.husako`).

#### Enum Value Completions

For methods with enum-typed parameters (e.g., `imagePullPolicy(v: "Always" | "IfNotPresent" | "Never")`), completions include all enum values with string literal quoting.

#### Quantity Value Completions

When the cursor is inside the string argument of `cpu()` or `memory()` (and `request()` / `limit()`), the LSP provides completions for common Kubernetes quantity values:

**`cpu("...")` completions:**
- `"100m"`, `"250m"`, `"500m"`, `"1000m"` — millicores
- `"1"`, `"2"`, `"4"` — full cores

**`memory("...")` completions:**
- `"64Mi"`, `"128Mi"`, `"256Mi"`, `"512Mi"` — mebibytes
- `"1Gi"`, `"2Gi"`, `"4Gi"`, `"8Gi"` — gibibytes

These are hardcoded common values in the LSP (not schema-derived — quantity fields have no enum). Users can type any valid Kubernetes quantity; completions are suggestions for the most common values.

---

### 5. Diagnostic Rules

All diagnostics are produced per-file on save or after a brief idle delay (debounced ~300ms).

#### Rule 1: RequiredFieldCheck

**What:** Detects missing required fields (from OpenAPI `required` array) in a chain passed to a schema-typed builder method.

**Severity:** Error

**Error location:** The `.metadata(expr)` / `.containers([expr])` call site — where context is confirmed and the required fields are known.

**Data flow analysis:** Tracks chain through direct variable assignments in the same scope (SSA-style). Skips if:
- Conditional expression (ternary `? :`, `if` branch)
- Function boundary (chain returned from a function)
- Loop variable
- External module import

**No false positives:** If the LSP cannot determine with 100% certainty that a required field is missing, it stays silent. `husako render` is the safety net.

```typescript
// Error: image is required for ContainerChain
.containers([name("nginx")])
//           ^^^^^^^^^^^^^^ Error: ContainerChain missing required field: image

// OK: image present
.containers([name("nginx").image("nginx:1.25")])

// Skip (conditional — cannot determine): no error shown
const c = flag ? name("x").image("a") : name("y");
.containers([c])
```

#### Rule 2: QuantityLiteralCheck

**Severity:** Error
**Trigger:** Invalid Kubernetes quantity grammar in `cpu()` / `memory()` string arguments.
**Error location:** The string literal argument.

#### Rule 3: BuildContractCheck

**Severity:** Error
**Trigger:** `husako.build()` called 0 or 2+ times.
**Error location:** The duplicate `husako.build()` call site (for 2+ calls); file-level diagnostic (for 0 calls).

#### Rule 4: ImageFormatCheck

**Severity:** Warning
**Trigger:** Invalid OCI image reference format.
**Error location:** The string literal argument.
**Valid format:** `(registry/)?name(:tag)?(@sha256:[a-f0-9]{64})?`

#### Rule 5: PatternCheck

**Severity:** Warning
**Trigger:** String argument violates the OpenAPI `pattern` constraint for that field.
**Error location:** The string literal argument.

#### Rule 6: EnumValueCheck

**Severity:** Warning
**Trigger:** Enum-typed method argument uses a value not in the defined enum.
**Error location:** The string literal argument.

#### Rule 7: RangeCheck

**Severity:** Warning
**Trigger:** Numeric argument violates `minimum`/`maximum` schema constraint.
**Error location:** The numeric literal argument.

---

### 6. Duplicate Import Suppression

TypeScript LSP reports duplicate identifier errors when the same name is imported from two different modules. husako LSP **suppresses** this diagnostic when:
- Both imports are from `k8s/*` modules
- The file has `.husako` extension

---

### 7. Schema Metadata Loading

The LSP reads `_chains.meta.json` (generated alongside `_chains.d.ts` by `husako gen`) for machine-readable constraint data:

```json
{
  "MetadataChain": {
    "name": { "type": "string", "required": true, "pattern": "^[a-z0-9][a-z0-9-]*$" },
    "namespace": { "type": "string", "required": false },
    "label": { "type": "map<string, string>", "required": false }
  },
  "ContainerChain": {
    "name": { "type": "string", "required": true },
    "image": { "type": "string", "required": true },
    "imagePullPolicy": { "type": "enum", "values": ["Always", "IfNotPresent", "Never"] }
  }
}
```

The LSP reloads this file when `husako gen` is run (file-watcher on `.husako/types/`).

---

### 8. Editor Integration

#### VS Code (`editors/vscode/`)

`package.json`:
```json
{
  "contributes": {
    "languages": [{ "id": "typescript", "extensions": [".husako"] }]
  }
}
```
Extension starts `husako lsp` as a subprocess, connects via stdio.

#### Zed (`editors/zed/`)

Registers `.husako` → TypeScript grammar + husako LSP as a secondary language server.

#### Neovim (user-configured)

```lua
vim.lsp.start({ name = "husako", cmd = { "husako", "lsp" }, filetypes = { "husako" } })
```

---

### 9. CLI Integration

```bash
husako lsp   # starts LSP server, speaks JSON-RPC over stdin/stdout
```

This is the only entry point. Editor extensions invoke this command.

---

### 10. Implementation Notes

- **oxc_parser**: parse `.husako` file as TypeScript ESM → AST for context detection
- **tower-lsp**: async LSP framework over stdio
- **No network I/O in LSP hot path**: all validation is local (schema metadata + regex)
- **Debounced diagnostics**: ~300ms idle delay after last edit before re-running rules
- **Incremental parsing**: full re-parse on each edit (oxc is fast enough)
- **Workspace state**: `husako.toml` read once at startup, re-read on file change
