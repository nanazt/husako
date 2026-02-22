# Plan: Rename `init` command to `generate` (M14)

## Context

`husako init` generates type definitions and `tsconfig.json` from OpenAPI specs. The name "init" implies one-time project initialization, but the command is actually a repeatable type generation step. `husako new` is the real project initializer.

Rename `init` → `generate` (with `gen` as alias) to match what it actually does.

## Approach

Primary subcommand: `generate`. Alias: `gen`. Both work identically.

```
husako generate                          # full name
husako gen                               # alias
husako gen --skip-k8s                    # with flags
husako generate --api-server https://... # with flags
```

## Changes

### 1. CLI subcommand (`crates/husako-cli/src/main.rs`)

- Rename `Init` variant → `Generate` with `#[command(alias = "gen")]`
- Update doc comment: "Generate type definitions and tsconfig.json"
- Update handler match arm
- Update "Next steps" output: `"husako init"` → `"husako generate"`

### 2. Core function + types (`crates/husako-core/src/lib.rs`)

- `InitOptions` → `GenerateOptions`
- `pub fn init()` → `pub fn generate()`
- `HusakoError::InitIo` → `HusakoError::GenerateIo`
- Error message: `"init I/O error"` → `"generate I/O error"`
- Update test function names (`init_skip_k8s_*` → `generate_skip_k8s_*`, etc.)

### 3. Schema source (`crates/husako-core/src/schema_source.rs`)

- All `HusakoError::InitIo(...)` → `HusakoError::GenerateIo(...)`

### 4. Runtime resolver errors (`crates/husako-runtime-qjs/src/resolver.rs`)

- `"husako init"` → `"husako generate"` in 2 error messages
- Update test assertion in `crates/husako-runtime-qjs/src/lib.rs`

### 5. Integration tests (`crates/husako-cli/tests/integration.rs`)

- All `.args(["init", ...])` → `.args(["generate", ...])`
- Rename test functions: `init_*` → `generate_*`
- Update string assertions: `"husako init"` → `"husako generate"`
- Update milestone comment: "husako init" → "husako generate"

### 6. E2E tests (`crates/husako-cli/tests/real_spec_e2e.rs`)

- All `.args(["init", ...])` → `.args(["generate", ...])`
- Update comments

### 7. Documentation

- `CLAUDE.md` — update Architecture section, Configuration section, any "husako init" references
- `.claude/PLAN.md` — update M5 title, M13 sections
- `.claude/plans/m13-husako-toml.md` — update all "husako init" references
- `README.md` — update usage examples, command table, section header

### Not changed

- `.initContainers()` — Kubernetes builder method, unrelated
- `CustomResourceDefinition` — Kubernetes CRD type, unrelated

## Verification

```bash
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check

# Verify alias works
cargo run -- generate --help
cargo run -- gen --help

# Verify old command fails gracefully
cargo run -- init 2>&1  # should show "unrecognized subcommand"
```
