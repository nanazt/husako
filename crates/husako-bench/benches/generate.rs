use std::collections::HashMap;
use std::path::Path;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use husako_bench::dts_fixtures_dir;
use husako_dts::{GenerateOptions, generate};

fn load_spec(dir: &Path, rel: &str) -> (String, serde_json::Value) {
    let content = std::fs::read_to_string(dir.join(rel))
        .unwrap_or_else(|e| panic!("failed to read {rel}: {e}"));
    let value: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("failed to parse {rel}: {e}"));
    // Discovery key: strip the vendor prefix and ".json" suffix.
    // e.g. "k8s/api/v1.json" → "api/v1"
    //      "crds/cert-manager/apis/cert-manager.io/v1.json" → "apis/cert-manager.io/v1"
    let key = rel
        .strip_prefix("k8s/")
        .or_else(|| {
            // "crds/<vendor>/apis/..." → "apis/..."
            let after_crds = rel.strip_prefix("crds/")?;
            after_crds.find('/').map(|i| &after_crds[i + 1..])
        })
        .unwrap_or(rel)
        .trim_end_matches(".json")
        .to_owned();
    (key, value)
}

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
fn bench_generate(c: &mut Criterion) {
    let dir = dts_fixtures_dir();

    // Load all specs once, before the benchmark loop.
    let core_v1_specs: HashMap<String, serde_json::Value> =
        [load_spec(&dir, "k8s/api/v1.json")].into_iter().collect();

    let full_k8s_specs: HashMap<String, serde_json::Value> = [
        load_spec(&dir, "k8s/api/v1.json"),
        load_spec(&dir, "k8s/apis/apps/v1.json"),
        load_spec(&dir, "k8s/apis/batch/v1.json"),
        load_spec(&dir, "k8s/apis/networking.k8s.io/v1.json"),
    ]
    .into_iter()
    .collect();

    let full_k8s_crds_specs: HashMap<String, serde_json::Value> = [
        load_spec(&dir, "k8s/api/v1.json"),
        load_spec(&dir, "k8s/apis/apps/v1.json"),
        load_spec(&dir, "k8s/apis/batch/v1.json"),
        load_spec(&dir, "k8s/apis/networking.k8s.io/v1.json"),
        load_spec(&dir, "crds/cert-manager/apis/cert-manager.io/v1.json"),
        load_spec(&dir, "crds/fluxcd/apis/source.toolkit.fluxcd.io/v1.json"),
        load_spec(&dir, "crds/fluxcd/apis/kustomize.toolkit.fluxcd.io/v1.json"),
        load_spec(&dir, "crds/fluxcd/apis/helm.toolkit.fluxcd.io/v2.json"),
        load_spec(&dir, "crds/cnpg/apis/postgresql.cnpg.io/v1.json"),
    ]
    .into_iter()
    .collect();

    let mut group = c.benchmark_group("generate");

    group.bench_with_input(
        BenchmarkId::from_parameter("core_v1"),
        &core_v1_specs,
        |b, specs| {
            b.iter(|| {
                generate(&GenerateOptions {
                    specs: specs.clone(),
                })
                .unwrap()
            })
        },
    );

    group.bench_with_input(
        BenchmarkId::from_parameter("full_k8s"),
        &full_k8s_specs,
        |b, specs| {
            b.iter(|| {
                generate(&GenerateOptions {
                    specs: specs.clone(),
                })
                .unwrap()
            })
        },
    );

    group.bench_with_input(
        BenchmarkId::from_parameter("full_k8s_crds"),
        &full_k8s_crds_specs,
        |b, specs| {
            b.iter(|| {
                generate(&GenerateOptions {
                    specs: specs.clone(),
                })
                .unwrap()
            })
        },
    );

    group.finish();
}

criterion_group!(benches, bench_generate);
criterion_main!(benches);
