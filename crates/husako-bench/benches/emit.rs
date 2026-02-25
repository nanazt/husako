use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use husako_core::emit_yaml;
use serde_json::{Value, json};

fn make_deployment(n: u32) -> Value {
    json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": {
            "name": format!("bench-deploy-{n}"),
            "namespace": "default",
            "labels": { "app": format!("bench-{n}"), "env": "bench" }
        },
        "spec": {
            "replicas": 2,
            "selector": { "matchLabels": { "app": format!("bench-{n}") } },
            "template": {
                "metadata": { "labels": { "app": format!("bench-{n}") } },
                "spec": {
                    "containers": [{
                        "name": "app",
                        "image": format!("nginx:{n}"),
                        "resources": {
                            "requests": { "cpu": "100m", "memory": "128Mi" },
                            "limits": { "cpu": "500m", "memory": "256Mi" }
                        }
                    }]
                }
            }
        }
    })
}

fn bench_emit(c: &mut Criterion) {
    let mut group = c.benchmark_group("emit_yaml");

    for n in [1u32, 10, 50] {
        let docs = Value::Array((0..n).map(make_deployment).collect());
        group.bench_with_input(BenchmarkId::from_parameter(n), &docs, |b, docs| {
            b.iter(|| emit_yaml(docs).unwrap())
        });
    }

    group.finish();
}

criterion_group!(benches, bench_emit);
criterion_main!(benches);
