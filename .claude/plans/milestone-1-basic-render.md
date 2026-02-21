# Milestone 1: Minimal `husako render` (Single File, Build-Capture)

**Status**: Completed
**Commit**: `3c7d7fe` (bundled with Milestone 2)

## Goal

Deliver the core pipeline: TypeScript source -> oxc compile -> QuickJS execute -> capture build output -> emit YAML.

## Deliverables

- oxc compile TS -> JS (strip types, preserve ESM)
- QuickJS eval entry module
- Builtin `"husako"` module with `build()` that sends output to Rust sink
- Strict JSON enforcement (reject undefined, functions, class instances, etc.)
- YAML emitter
- CLI with `husako render <file>` command
- Exit code mapping (0/1/3/4/7)

## Architecture Decisions

### Pipeline

```
source.ts -> husako-compile-oxc::compile() -> JS string
          -> husako-runtime-qjs::execute() -> serde_json::Value
          -> husako-yaml::emit_yaml()      -> YAML string
```

### Crate Boundaries

| Crate | Responsibility |
|---|---|
| `husako-cli` | Thin CLI: clap parsing, file I/O, exit code mapping |
| `husako-core` | Pipeline orchestration: compile -> execute -> emit |
| `husako-compile-oxc` | oxc TypeScript -> JavaScript compilation |
| `husako-runtime-qjs` | QuickJS runtime + `"husako"` module loader + build capture |
| `husako-yaml` | `serde_json::Value` -> YAML string |
| `husako-sdk` | Builtin JS source for `"husako"` module |

### Strict JSON Contract

The `build()` call in JS sends data through a custom QuickJS-to-Rust bridge that:
1. Recursively walks the JS value tree
2. Rejects: `undefined`, `bigint`, `symbol`, functions, class instances, Date, Map, Set, RegExp, cyclic references
3. Produces `serde_json::Value` on the Rust side
4. Reports errors with `doc[index]`, JSON path, and value kind

### Error Handling

- `thiserror` enums at crate boundaries
- `HusakoError` in `husako-core` wraps all downstream errors
- CLI maps `HusakoError` variants to exit codes

## Files Created

```
crates/husako-cli/src/main.rs          # clap CLI, exit code mapping
crates/husako-core/src/lib.rs          # render() pipeline
crates/husako-compile-oxc/src/lib.rs   # compile() with oxc
crates/husako-runtime-qjs/src/lib.rs   # execute() with QuickJS
crates/husako-yaml/src/lib.rs          # emit_yaml()
crates/husako-sdk/src/lib.rs           # JS source constants
crates/husako-sdk/src/js/husako.js     # build() implementation
Cargo.toml                             # workspace root
```

## Key Dependencies

- `oxc_allocator`, `oxc_parser`, `oxc_transformer`, `oxc_codegen` — TS compilation
- `rquickjs` — QuickJS embedding
- `serde_json` — intermediate representation
- `serde_yaml_ng` — YAML output
- `clap` — CLI parsing
- `thiserror` — error types

## Tests

- `husako render examples/basic.ts` -> exit 0, YAML output with apiVersion, kind, metadata
- Missing `build()` call -> exit 7
- `build()` called twice -> exit 7
- Compile error (syntax) -> exit 3
- Strict JSON: `undefined` -> exit 7
- Strict JSON: function -> exit 7

## Acceptance Criteria

- [x] `husako render examples/basic.ts` produces valid YAML
- [x] Missing build -> exit 7 with "build() was not called"
- [x] Double build -> exit 7 with "called 2 times"
- [x] Compile error -> exit 3
- [x] Strict JSON violations -> exit 7
