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
2. **Runtime** (`husako-runtime-qjs`): Executes compiled JS in QuickJS, loads builtin modules (`"husako"`, `"k8s/*"`, `"helm/*"`, plugin modules), captures `husako.build()` output via Rust-side sink
3. **Core** (`husako-core`): Orchestrates the pipeline, validates strict JSON contract and Kubernetes quantity grammar, manages plugin lifecycle
4. **Emitter** (`husako-yaml`): Converts validated `serde_json::Value` to YAML or JSON output
5. **OpenAPI** (`husako-openapi`): Fetches and caches Kubernetes OpenAPI v3 specs; CRD YAML→OpenAPI conversion; kubeconfig credential resolution; GitHub release spec download
6. **Type Generator** (`husako-dts`): Generates `.d.ts` type definitions and `_schema.json` from OpenAPI specs; JSON Schema → TypeScript for Helm charts
7. **Helm** (`husako-helm`): Resolves Helm chart `values.schema.json` from file, registry, ArtifactHub, or git sources
8. **SDK** (`husako-sdk`): Builtin JS runtime sources and base `.d.ts` for the `"husako"` module
9. **Config** (`husako-config`): Parses `husako.toml` project configuration (entry aliases, resource/chart/plugin dependencies)

## Project Structure

```
crates/
├── husako-cli/            # CLI entry point (clap), thin — no business logic
│   └── src/main.rs
├── husako-config/         # husako.toml parser (entry aliases, resource/chart/plugin deps, cluster config)
│   └── src/
│       ├── lib.rs              # Config structs, plugin manifest parser
│       └── edit.rs             # Format-preserving TOML editing
├── husako-core/           # Pipeline orchestration + validation + schema source resolution + plugins
│   └── src/
│       ├── lib.rs              # generate(), render(), scaffold(), JSONC tsconfig handling
│       ├── plugin.rs           # Plugin install/remove/list, preset merging, tsconfig paths
│       ├── quantity.rs         # Kubernetes quantity grammar validation
│       ├── schema_source.rs    # Schema source dispatch (file, cluster, release, git)
│       └── validate.rs         # JSON Schema validation engine
├── husako-compile-oxc/    # TS → JS compilation via oxc
│   └── src/lib.rs
├── husako-runtime-qjs/    # QuickJS runner + module loader + build output capture
│   └── src/
│       ├── lib.rs              # QuickJS runtime, build() capture
│       ├── loader.rs           # Module loader (compile + resolve chain)
│       └── resolver.rs         # Import resolvers (builtin, plugin, k8s/*, helm/*, file)
├── husako-openapi/        # OpenAPI v3 fetch + disk cache + CRD/kubeconfig/release
│   └── src/
│       ├── lib.rs
│       ├── crd.rs              # CRD YAML → OpenAPI JSON conversion
│       ├── kubeconfig.rs       # Bearer token extraction from ~/.kube/
│       └── release.rs          # GitHub k8s release spec download + cache
├── husako-helm/           # Helm chart values.schema.json resolution (file, registry, artifacthub, git)
│   └── src/
│       ├── lib.rs              # Dispatch + cache_hash
│       ├── file.rs             # Local file source
│       ├── registry.rs         # HTTP Helm repository source
│       ├── artifacthub.rs      # ArtifactHub API source
│       └── git.rs              # Git repository source
├── husako-dts/            # OpenAPI → .d.ts + _validation.json generation; JSON Schema → TS for Helm
│   └── src/
│       ├── lib.rs              # OpenAPI → .d.ts + .js generation
│       ├── emitter.rs          # Code emitter (builders, interfaces, factory functions)
│       ├── json_schema.rs      # JSON Schema → .d.ts + .js for Helm chart values
│       ├── schema.rs           # Schema classification and extraction
│       └── schema_store.rs     # _schema.json generation for validation
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
- Supported imports: relative (`./`, `../`), builtins (`"husako"`, `"k8s/<group>/<version>"`, `"helm/<chart-name>"`), and plugin modules (`"<plugin>"`, `"<plugin>/sub"`). No npm/bare specifiers, Node built-ins, or network imports.
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
- `tsconfig.json` is parsed with JSONC support (comments + trailing commas) via `strip_jsonc()` in `husako-core`, so existing tsconfig files from `tsc --init` or IDE tooling are handled correctly

## Writing Docs

- Always respond in English and write documents in English.
- Before writing docs, see <https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing> and avoid these patterns.

## Configuration (`husako.toml`)

Project-level configuration file created by `husako new`. Supports:

- **Entry aliases**: `[entries]` maps short names to file paths (`dev = "env/dev.ts"`)
- **Resource dependencies**: `[resources]` declares k8s schema sources with 4 types: `release`, `cluster`, `git`, `file` (aliased from legacy `[schemas]`)
- **Chart dependencies**: `[charts]` declares Helm chart sources with 4 types: `registry`, `artifacthub`, `git`, `file`
- **Plugins**: `[plugins]` declares plugin sources with 2 types: `git` (URL), `path` (local directory)
- **Cluster config**: `[cluster]` (single) or `[clusters.*]` (multiple named clusters)

The `Render` command resolves the file argument as: direct path → entry alias → error with available aliases.

The `Generate` command priority chain for k8s types: `--skip-k8s` → CLI flags (legacy) → `husako.toml [resources]` → skip. Chart types from `[charts]` are always generated when configured.

## CI/CD

Workflows are authored in TypeScript using [gaji](https://github.com/dodok8/gaji) and compiled to YAML. Source files in `workflows/`, output in `.github/workflows/`.

```bash
gaji dev     # generate types
gaji build   # compile TS → YAML
```

| Workflow | Trigger | Purpose |
| --- | --- | --- |
| `check.yml` | PRs + push to `master` | fmt, clippy, tests |
| `version.yml` | Push to `master` | release-plz: release PR + crates.io publish |
| `distribute.yml` | `v*` tag | Cross-platform binaries, GitHub release assets, npm publish |
| `audit.yml` | Weekly | `cargo audit` |
| `sync-workflows.yml` | `workflows/**` changed | Regenerate YAML from TS sources |

Release flow: push to `master` → release-plz creates release PR → merge PR → crates.io publish + git tag → binary builds + npm publish.

Key files: `release-plz.toml`, `gaji.config.ts`, `npm/` (package structure), `scripts/sync-versions.sh`.

## Design Documents

Read `.claude/*.md` before making changes to related areas:

- `.claude/dsl-spec.md` — Builder DSL rules
- `.claude/cli-design.md` — CLI visual design system
- `.claude/architecture.md` — Deep implementation details (schema classification, CRD conversion, validation engine, codegen, caching, plugins)
- `.claude/plugin-spec.md` — Plugin system specification (manifest format, module resolution, helper authoring)

## Plans

When implementing non-trivial features, write a plan document first in `.claude/plans/`.
