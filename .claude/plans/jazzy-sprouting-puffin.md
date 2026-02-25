# Plan: Clarify Bench Scope — All 5 Benchmarks

## Context

The `generate` bench shows 4–8 ms but `husako gen` feels much slower.
This prompted a review of all 5 bench files to find similar scope gaps.

## Scope Analysis: What Each Bench Does and Doesn't Cover

### `compile.rs` — No gap
Measures `husako_compile_oxc::compile()` with inline TS strings.
Real usage also uses inline-in-memory source after file read (< 1 µs file I/O). **No comment needed.**

### `execute.rs` — Already documented
Pre-compiles TS → JS outside `b.iter()` to isolate QuickJS; comment already says so.
No undocumented gap.

### `render.rs` — Minor gap (schema validation skip)
`schema_store: None` is already noted inline. Excludes source file read (< 1 µs, negligible) and YAML write to stdout (also < 1 µs). **No change needed.**

### `emit.rs` — No gap
Measures JSON → YAML serialization only; `husako render` doesn't write to disk in the bench hot path. No notable exclusion.

### `generate.rs` — **Significant undocumented gap**
Calls `husako_dts::generate()` with pre-loaded JSON already in memory.

`husako gen` end-to-end:

| Step | In bench? | Typical cost |
|------|-----------|-------------|
| Network fetch (git clone / GitHub release) | No | seconds (cold) |
| Disk read from `.husako/cache/` | No | 20–80 ms |
| CRD YAML → OpenAPI JSON | No | a few ms |
| `husako_dts::generate()` (codegen) | **Yes** | 4–8 ms |
| Write `.d.ts`/`.js`/_schema.json to `.husako/types/` | No | 10–50 ms (100–300 files) |

## Files to Modify

| Path | Action |
|------|--------|
| `crates/husako-bench/benches/generate.rs` | Add scope comment to `bench_generate` |
| `.worktrees/docs-site/docs/advanced/benchmarks.md` | Add "Scope" note to generate row in the table |

## Changes

### `benches/generate.rs`

Add at the top of `fn bench_generate`:

```rust
// Measures: husako_dts::generate() — OpenAPI JSON → .d.ts + .js codegen only.
//
// NOT measured:
//   - Network fetch (git clone / GitHub release download)
//   - Disk read from .husako/cache/
//   - CRD YAML → OpenAPI JSON conversion
//   - Writing generated files to .husako/types/
//
// Specs are pre-loaded into memory before the benchmark loop so that only
// the codegen step is timed. This isolates algorithmic performance from I/O.
//
// Real `husako gen` is substantially slower:
//   cold run  — dominated by network (seconds)
//   warm run  — ~30–130 ms extra (cache read + CRD parse + file writes)
```

### `benchmarks.md` table

Change the `generate.rs` description:

```
| `generate.rs` | `husako_dts::generate` | OpenAPI → `.d.ts` + `.js` codegen (pre-loaded JSON; excludes network, cache read, CRD parsing, file write — see note below) |
```

Add a note below the table:

```markdown
> **Note on `generate` scope**: The benchmark only measures the pure codegen step.
> `husako gen` also fetches/reads specs and writes generated files, making warm runs
> ~30–130 ms slower than the benchmark suggests and cold runs (first fetch) much
> slower still.
```

## Verification

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
