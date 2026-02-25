use std::collections::HashMap;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use husako_bench::{K8S_MEDIUM_TS, K8S_SMALL_TS, MEDIUM_TS, SMALL_TS, bench_fixtures_dir};
use husako_compile_oxc::compile;
use husako_runtime_qjs::{ExecuteOptions, execute};

fn make_opts(
    fixtures: &std::path::Path,
    generated_types_dir: Option<std::path::PathBuf>,
) -> ExecuteOptions {
    ExecuteOptions {
        entry_path: fixtures.join("bench.ts"),
        project_root: fixtures.to_path_buf(),
        allow_outside_root: false,
        timeout_ms: None,
        max_heap_mb: None,
        generated_types_dir,
        plugin_modules: HashMap::new(),
    }
}

fn bench_execute(c: &mut Criterion) {
    let fixtures = bench_fixtures_dir();
    let types_dir = fixtures.join(".husako/types");
    let rt = tokio::runtime::Runtime::new().unwrap();

    // Pre-compile all TS → JS once, outside b.iter(), to isolate QuickJS execution cost.
    let small_js = compile(SMALL_TS, "bench.ts").expect("compile SMALL_TS");
    let medium_js = compile(MEDIUM_TS, "bench.ts").expect("compile MEDIUM_TS");
    let k8s_small_js = compile(K8S_SMALL_TS, "bench.ts").expect("compile K8S_SMALL_TS");
    let k8s_medium_js = compile(K8S_MEDIUM_TS, "bench.ts").expect("compile K8S_MEDIUM_TS");

    let mut group = c.benchmark_group("execute");

    // Builtin-only variants — always available.
    for (id, js) in [("builtin/small", &small_js), ("builtin/medium", &medium_js)] {
        let opts = make_opts(&fixtures, None);
        group.bench_with_input(BenchmarkId::from_parameter(id), js.as_str(), |b, js| {
            b.iter(|| rt.block_on(execute(js, &opts)).unwrap())
        });
    }

    // k8s variants — require `husako gen` in the fixtures directory.
    if types_dir.exists() {
        for (id, js) in [("k8s/small", &k8s_small_js), ("k8s/medium", &k8s_medium_js)] {
            let opts = make_opts(&fixtures, Some(types_dir.clone()));
            group.bench_with_input(BenchmarkId::from_parameter(id), js.as_str(), |b, js| {
                b.iter(|| rt.block_on(execute(js, &opts)).unwrap())
            });
        }
    } else {
        eprintln!(
            "Skipping execute/k8s/* benchmarks — types not generated.\n\
             Run: cd crates/husako-bench/fixtures && husako gen"
        );
    }

    group.finish();
}

criterion_group!(benches, bench_execute);
criterion_main!(benches);
