# Plan: Rename `husako generate` → `husako gen` (primary command)

## Context

`husako generate` is the most-used command but verbose to type. `gen` already exists
as a clap alias, but docs and error messages all say `generate`. The goal is to flip
the primary name to `gen`, keep `generate` as a hidden backwards-compat alias, and
update all user-facing references (messages, docs) to use `gen`. Internal Rust names
(`Generate`, `GenerateOptions`, `generate()`) are **unchanged** for code readability.

---

## Changes

### 1. `crates/husako-cli/src/main.rs`

#### a) Flip primary command name, keep `generate` as hidden alias

```rust
// Before
#[command(alias = "gen")]
Generate { ... }

// After
#[command(name = "gen", alias = "generate")]
Generate { ... }
```

`clap` derives the subcommand name from the variant by default (`Generate` → `"generate"`).
`name = "gen"` overrides the primary name to `gen`. `alias = "generate"` keeps the old
name working for backwards compatibility — it does not appear in `--help` output because
clap aliases are hidden by default.
`Commands::Generate` (the Rust match arm) is unchanged.

#### b) Three `eprintln!` suggestion lines → `husako gen`

| Line (approx) | Location |
|---------------|----------|
| ~427 | `Commands::New` success — "Next steps: husako generate" |
| ~461 | `Commands::Init` success — "Next steps: husako generate" |
| ~1060 | `Commands::Plugin { add }` — "Run 'husako generate' to install..." |

---

### 2. `crates/husako-core/src/lib.rs`

Four error message strings that say `"Run 'husako generate' to ..."` → `"Run 'husako gen' to ..."`:

- ~line 1525: `"Run 'husako generate' to create type definitions"`
- ~line 1539: `"Run 'husako generate' to update tsconfig.json"`
- ~line 1550: `"Run 'husako generate' to create tsconfig.json"`
- ~line 1562: `"Run 'husako generate' to update"`

---

### 3. `crates/husako-runtime-qjs/src/resolver.rs`

Three error strings → `husako gen`:

- ~line 34: `"... Run 'husako generate' to install plugins"`
- ~line 68: `"... require 'husako generate' to be run first"`
- ~line 92: `"... Run 'husako generate' to generate {} modules"`

---

### 4. `crates/husako-runtime-qjs/src/lib.rs` (unit tests)

Two assertions that check `contains("husako generate")` → `contains("husako gen")`.

---

### 5. `crates/husako-cli/tests/integration.rs`

- **6 test args**: `.args(["generate", ...])` → `.args(["gen", ...])`
- **2 error message assertions**: `contains("husako generate")` → `contains("husako gen")`

---

### 6. Docs (`.worktrees/docs-site/docs/`)

Bulk replace `husako generate` → `husako gen` across all 7 affected files:

| File | Occurrences |
|------|-------------|
| `guide/getting-started.md` | 5 |
| `guide/configuration.md` | 3 |
| `guide/helm.md` | 4 |
| `guide/testing.md` | 7 |
| `reference/cli.md` | 7 (including section heading) |
| `reference/import-system.md` | 4 |
| `advanced/plugins.md` | 4 |
| `advanced/benchmarks.md` | 2 |

`reference/cli.md` heading `## husako generate` → `## husako gen`.

---

### 7. `CLAUDE.md` + `.claude/*.md`

- `CLAUDE.md` already uses `husako gen` on line 38. Other occurrences of `husako generate` → `husako gen`.
- `.claude/plugin-spec.md` and `.claude/testing.md`: update ~6 references.

---

## What is NOT changed

- `Commands::Generate` enum variant
- `GenerateOptions` struct
- `generate()` function
- `HusakoError::GenerateIo` etc.
- Any other internal Rust identifiers

---

## Verification

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Manual check:
```bash
husako gen --help        # ✔ works, primary name shown
husako generate --help   # ✔ works via hidden alias (not shown in husako --help)
husako --help            # shows "gen" only, no "generate"
```
