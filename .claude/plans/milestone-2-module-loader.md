# Milestone 2: Module Loader + Project Imports

**Status**: Completed
**Commit**: `3c7d7fe`

## Goal

Enable real multi-file TypeScript projects by adding relative import resolution with extension/index inference and project root boundary enforcement.

## Deliverables

- Relative import resolution (`./`, `../`) with extension inference (`.ts`, `.js`)
- Index file inference (`./lib` -> `./lib/index.ts`)
- Project root boundary (resolved imports must stay within project root)
- `--allow-outside-root` escape hatch
- Builtin module recognition: `"husako"`, `"k8s/<group>/<version>"`
- Per-module compilation (each imported `.ts` file gets compiled to JS)

## Architecture Decisions

### Import Resolution Algorithm

```
1. If specifier is "husako" or "k8s/*" -> builtin module (JS from husako-sdk)
2. If specifier starts with "./" or "../" -> relative import:
   a. Resolve against importer's directory
   b. Try: exact path, path.ts, path.js, path/index.ts, path/index.js
   c. Canonicalize resolved path
   d. Check: resolved path must be within project_root (unless --allow-outside-root)
   e. Read source, compile TS->JS if .ts, return JS
3. Otherwise -> error (bare specifiers not supported)
```

### Module Loader in QuickJS

The `rquickjs` module loader is configured with:
- A **resolver** that maps specifiers to canonical paths
- A **loader** that reads + compiles the source

Both are implemented in Rust, registered with the QuickJS runtime before eval.

### Project Root

- Defaults to `cwd`
- Used as the boundary for import resolution
- Passed through `ExecuteOptions` to the runtime

## Files Created/Modified

```
crates/husako-runtime-qjs/src/lib.rs   # Module loader + resolver
crates/husako-cli/src/main.rs          # --allow-outside-root flag
crates/husako-core/src/lib.rs          # ExecuteOptions with project_root
examples/project/                      # Multi-file example project
  env/dev.ts                           # Entry that imports shared modules
  shared/base.ts                       # Shared metadata
  components/nginx.ts                  # Component module
```

## Tests

### Integration Tests

- `examples/project/env/dev.ts` renders with cross-file imports -> exit 0
- Outside-root import without flag -> exit 4 ("outside project root")
- Outside-root import with `--allow-outside-root` -> exit 0
- `./lib` resolves to `lib.ts` (extension inference)
- `./lib` resolves to `lib/index.ts` (index inference)

### Snapshot Tests

- `render_basic_yaml_snapshot` — basic single-file output
- `render_project_snapshot` — multi-file project output

## Acceptance Criteria

- [x] Multi-file project with relative imports renders correctly
- [x] Extension inference (.ts) works
- [x] Index inference (dir/index.ts) works
- [x] Outside-root import rejected by default (exit 4)
- [x] `--allow-outside-root` bypasses boundary check
