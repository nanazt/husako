# CLAUDE.md

## Project Overview

**husako** is a Rust CLI tool that enables type-safe Kubernetes Resources authoring in TypeScript.
Users write Resources in TypeScript with full type safety and autocomplete, and husako compiles them to YAML files that Kubernetes understands.

The project was inspired by **[gaji](https://github.com/dodok8/gaji)**.

- **Repository**: https://github.com/nanazt/husako
- **Language**: Rust (2024 Edition)
- **License**: MIT
- **Version**: defined in `Cargo.toml`

## Quick Reference Commands

```bash
# Build
cargo build

# Release build (optimized for size)
cargo build --release

# Lint
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Format
cargo fmt --all --check   # check only
cargo fmt --all           # apply
```

## Architecture

The core pipeline is: **TypeScript → Compile → Execute → Validate → Emit YAML**

1. **Compiler** (`husako-compile-oxc`): Strips TypeScript types with oxc, producing plain JavaScript
2. **Runtime** (`husako-runtime-qjs`): Executes compiled JS in QuickJS, loads builtin modules (`"husako"`, `"k8s/*"`), captures `husako.build()` output via Rust-side sink
3. **Core** (`husako-core`): Orchestrates the pipeline, validates strict JSON contract and Kubernetes quantity grammar
4. **Emitter** (`husako-yaml`): Converts validated `serde_json::Value` to YAML or JSON output
5. **OpenAPI** (`husako-openapi`): Fetches and caches Kubernetes OpenAPI v3 specs; CRD YAML→OpenAPI conversion; kubeconfig credential resolution; GitHub release spec download
6. **Type Generator** (`husako-dts`): Generates `.d.ts` type definitions and `_schema.json` from OpenAPI specs
7. **SDK** (`husako-sdk`): Builtin JS runtime sources and base `.d.ts` for the `"husako"` module
8. **Config** (`husako-config`): Parses `husako.toml` project configuration (entry aliases, schema dependencies)

## Project Structure

```
crates/
├── husako-cli/            # CLI entry point (clap), thin — no business logic
│   └── src/main.rs
├── husako-config/         # husako.toml parser (entry aliases, schema deps, cluster config)
│   └── src/lib.rs
├── husako-core/           # Pipeline orchestration + validation + schema source resolution
│   └── src/
│       ├── lib.rs
│       └── schema_source.rs
├── husako-compile-oxc/    # TS → JS compilation via oxc
│   └── src/lib.rs
├── husako-runtime-qjs/    # QuickJS runner + module loader + build output capture
│   └── src/lib.rs
├── husako-openapi/        # OpenAPI v3 fetch + disk cache + CRD/kubeconfig/release
│   └── src/
│       ├── lib.rs
│       ├── crd.rs          # CRD YAML → OpenAPI JSON conversion
│       ├── kubeconfig.rs   # Bearer token extraction from ~/.kube/
│       └── release.rs      # GitHub k8s release spec download + cache
├── husako-dts/            # OpenAPI → .d.ts + _validation.json generation
│   └── src/lib.rs
├── husako-yaml/           # JSON → YAML/JSON emitter
│   └── src/lib.rs
└── husako-sdk/            # Builtin JS sources + base .d.ts + project templates
    └── src/lib.rs
```

Boundary rules:

- CLI crate is thin — delegates to `husako-core`.
- Runtime boundary payload is `serde_json::Value`.
- Error enums at crate boundaries (`thiserror`); user-facing formatting lives in CLI.

## Key Design Patterns

- **Immutable builders**: Fragment objects (metadata, resource quantities) are immutable or copy-on-write, safe to assign to variables and reuse
- **Merge semantics**: Last-argument-wins for scalars, deep-merge by key for maps (labels/annotations), replace for arrays

## Code Conventions

- **Modules**: `snake_case`
- **Types/Structs**: `PascalCase`
- **Functions/Methods**: `snake_case`
- **Constants**: `UPPER_SNAKE_CASE`
- **Tests**: Inline `#[cfg(test)]` blocks in each module

## Exit Codes (Stable)

| Code | Meaning                                    |
| ---- | ------------------------------------------ |
| 0    | Success                                    |
| 1    | Unexpected failure                         |
| 2    | Invalid args/config                        |
| 3    | Compile failure (oxc)                      |
| 4    | Runtime failure (QuickJS / module loading) |
| 5    | Type generation failure                    |
| 6    | OpenAPI fetch/cache failure                |
| 7    | Emit/validation/contract failure           |

## Hard Contracts

- Entrypoint is executed as an ESM module.
- `husako.build(input)` must be called exactly once with builder instances (items must have `_render()`). Missing call → exit 7. Multiple calls → exit 7. Plain objects → TypeError.
- Strict JSON enforcement by default (`--strict-json=true`): no `undefined`, `bigint`, `symbol`, functions, class instances, `Date`, `Map`, `Set`, `RegExp`, or cyclic references.
- Validation errors must include `doc[index]`, JSON path (`$.spec...`), and value kind.
- Supported imports: relative (`./`, `../`) and builtins (`"husako"`, `"k8s/<group>/<version>"`). No npm/bare specifiers, Node built-ins, or network imports.
- Resolved imports must stay within project root by default. `--allow-outside-root` overrides this.

## Testing

```bash
# All tests
cargo test --workspace --all-features

# Specific crate
cargo test -p husako-runtime-qjs

# Specific test
cargo test -p husako-core test_name
```

- **Unit tests**: Inline `#[cfg(test)]` in each source file
- **Integration tests**: `assert_cmd` for exit code mapping, import resolution, strict JSON contract failures, quantity validation with JSON path
- **Snapshot tests**: `insta` for YAML output comparison
- **No external network**: Use a local mock server for OpenAPI tests

## Gotchas

- `.husako/` directory (cache + generated types) must be in `.gitignore` -- it is auto-managed and should never be committed or edited manually
- The binary name is `husako` (set in `husako-cli/Cargo.toml` as `package.name`), not the repo name

## Writing Docs

- Always respond in English and write documents in English.
- Before writing docs, see <https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing> and avoid these patterns.

## Configuration (`husako.toml`)

Project-level configuration file created by `husako new`. Supports:

- **Entry aliases**: `[entries]` maps short names to file paths (`dev = "env/dev.ts"`)
- **Schema dependencies**: `[schemas]` declares sources with 4 types: `release`, `cluster`, `git`, `file`
- **Cluster config**: `[cluster]` (single) or `[clusters.*]` (multiple named clusters)

The `Render` command resolves the file argument as: direct path → entry alias → error with available aliases.

The `Generate` command priority chain: `--skip-k8s` → CLI flags (legacy) → `husako.toml [schemas]` → skip.

## Design Documents

Read `.claude/*.md` before making changes to related areas. Key documents:

- `.claude/builder-spec.md` — Authoritative reference for the builder DSL rules
- `.claude/plans/m13-husako-toml.md` — `husako.toml` config design (M13a/M13b/M13c)

## Plan Details

Always check `.claude/PLAN.md` before proceeding.
Before implementing a plan, write the plan document first.

Plan files live in `.claude/plans/` and follow the naming convention `mN-<topic>.md` (e.g., `m13-husako-toml.md`). The `N` matches the milestone number in `PLAN.md`.
