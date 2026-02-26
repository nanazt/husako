# Benchmarks

husako includes a [Criterion](https://bheisler.github.io/criterion.rs/book/) benchmark suite in `crates/husako-bench/`. It covers the five most performance-sensitive phases of the pipeline.

## What Each Benchmark Measures

| File | Function | What it measures |
|------|----------|-----------------|
| `compile.rs` | `husako_compile_oxc::compile` | oxc TypeScript → JavaScript compilation |
| `execute.rs` | `husako_runtime_qjs::execute` | QuickJS runtime execution (per fresh runtime; compile step excluded by design) |
| `render.rs` | `husako_core::render` | Full pipeline: compile + execute + emit (validation skipped — no cluster needed) |
| `generate.rs` | `husako_dts::generate` | OpenAPI → `.d.ts` + `.js` codegen only (see scope note below) |
| `emit.rs` | `husako_yaml::emit_yaml` | JSON → YAML serialization |

Each benchmark has multiple input sizes (`small`, `medium`, `large` or document counts `1`, `10`, `50`) so you can measure both throughput and scaling.

> **Note on `generate` scope**: The 4–8 ms shown by this benchmark is only the pure codegen step.
> `husako gen` also fetches specs from the network or cache and writes the generated files to disk, making
> warm runs (cached) roughly 30–130 ms slower than the benchmark suggests. Cold runs (first fetch) are
> much slower still, dominated by network I/O. Steps not covered: network fetch, `.husako/cache/` disk
> read, CRD YAML → OpenAPI conversion, writing `.d.ts`/`.js`/`_schema.json` to `.husako/types/`.

The `execute` and `render` benchmarks also have `k8s/*` variants that import real Kubernetes types, requiring the types to be generated first (see Prerequisites below).

## Prerequisites

All benchmarks except the `k8s/*` variants work without any setup.

For `execute/k8s/*` and `render/k8s/*`:

```bash
# Build the husako binary
cargo build --release --bin husako

# Generate k8s + CRD types in the bench fixture directory
cd crates/husako-bench/fixtures
../../../target/release/husako gen
cd ../../..
```

This downloads OpenAPI specs (k8s 1.35.1, cert-manager, FluxCD, CloudNativePG) and writes generated types to `crates/husako-bench/fixtures/.husako/types/`. Downloads are cached in `.husako/cache/` on subsequent runs.

## Running Benchmarks

```bash
# Quick sanity check — all benchmarks compile and run once (no statistics, fast)
cargo bench -p husako-bench -- --test

# Full Criterion run with statistics and HTML reports
cargo bench -p husako-bench

# Single benchmark group
cargo bench -p husako-bench --bench compile
cargo bench -p husako-bench --bench generate
```

After a full run, HTML reports appear at `target/criterion/`. Open the index to compare runs:

```bash
open target/criterion/compile/report/index.html
```

## Interpreting Criterion Output

A typical output line:

```
compile/small   time:   [3.1234 µs 3.1456 µs 3.1701 µs]
```

The three values are the lower bound, point estimate, and upper bound of a confidence interval for the per-iteration time. If you run twice and Criterion detects a statistically significant change, it prints:

```
Performance has regressed.  [+5.12% +5.34% +5.58%]
```

The first run stores a baseline in `target/criterion/`. Subsequent runs compare against it.

## Generating a Report

After running benchmarks, generate a shareable markdown report:

```bash
cargo run -p husako-bench --bin report
```

This reads all `estimates.json` files from `target/criterion/` and writes two files:

- **`target/criterion/bench-summary.md`** — compact table, one row per benchmark
- **`target/criterion/bench-report.md`** — detailed view with 95% CI bounds and slope

Both files include a header with the date and time, git commit (and `dirty` flag if there are uncommitted changes), husako version, and platform/architecture.

To write to a different directory:

```bash
cargo run -p husako-bench --bin report -- --output-dir ./reports
```

## CI Integration

Benchmarks run automatically on every push to `master` and on every `v*` release tag
(see `.github/workflows/bench.yml`). Each run:

1. Runs the full Criterion suite and uploads results as a GitHub Actions artifact named `criterion`
2. Runs the `report` binary and commits the markdown to the `bench-results` branch

**`bench-results` branch layout:**

- `latest/` — overwritten on every run (master push or tag)
- `releases/<version>/` — created once per release tag, permanent

To inspect results without cloning:

```bash
git fetch origin bench-results
git show origin/bench-results:latest/bench-summary.md
# or for a specific release:
git show origin/bench-results:releases/v0.3.0/bench-summary.md
```

> **Note on CI bench numbers**: GitHub Actions runners are shared VMs. Results will vary
> ±10–15% run-to-run due to CPU scheduling and noisy neighbors. CI bench results are
> useful for catching large regressions (>20%) and tracking order-of-magnitude
> performance per release — not for precise micro-benchmark comparison. The `CPU` and
> `Runner` fields in the report header record what hardware was used; this helps explain
> shifts when GitHub updates their runner infrastructure. For precise numbers, run
> benchmarks locally on a dedicated machine.

PRs do not run full benchmarks. Use `cargo bench -p husako-bench -- --test` locally to verify your changes compile and execute without errors.
