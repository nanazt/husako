use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use husako_bench::{K8S_MEDIUM_TS, K8S_SMALL_TS, SMALL_TS, bench_fixtures_dir};
use husako_core::{RenderOptions, render};

fn make_opts(fixtures: &std::path::Path) -> RenderOptions {
    RenderOptions {
        project_root: fixtures.to_path_buf(),
        allow_outside_root: false,
        schema_store: None, // skip validation — no live cluster needed
        timeout_ms: None,
        max_heap_mb: None,
        verbose: false,
    }
}

fn bench_render(c: &mut Criterion) {
    let fixtures = bench_fixtures_dir();
    let types_dir = fixtures.join(".husako/types");
    let opts = make_opts(&fixtures);

    let mut group = c.benchmark_group("render");

    // Full pipeline: compile + execute + emit. Builtin variant always works.
    group.bench_with_input(
        BenchmarkId::from_parameter("builtin/small"),
        SMALL_TS,
        |b, src| b.iter(|| render(src, "bench.ts", &opts).unwrap()),
    );

    // k8s variants require `husako gen` in fixtures directory.
    if types_dir.exists() {
        for (id, src) in [("k8s/small", K8S_SMALL_TS), ("k8s/medium", K8S_MEDIUM_TS)] {
            group.bench_with_input(BenchmarkId::from_parameter(id), src, |b, src| {
                b.iter(|| render(src, "bench.ts", &opts).unwrap())
            });
        }
    } else {
        eprintln!(
            "Skipping render/k8s/* benchmarks — types not generated.\n\
             Run: cd crates/husako-bench/fixtures && husako gen"
        );
    }

    group.finish();
}

criterion_group!(benches, bench_render);
criterion_main!(benches);
