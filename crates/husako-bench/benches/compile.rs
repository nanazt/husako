use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use husako_bench::{LARGE_TS, MEDIUM_TS, SMALL_TS};
use husako_compile_oxc::compile;

fn bench_compile(c: &mut Criterion) {
    let mut group = c.benchmark_group("compile");
    for (id, src) in [
        ("small", SMALL_TS),
        ("medium", MEDIUM_TS),
        ("large", LARGE_TS),
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(id), src, |b, src| {
            b.iter(|| compile(src, "bench.ts").unwrap())
        });
    }
    group.finish();
}

criterion_group!(benches, bench_compile);
criterion_main!(benches);
