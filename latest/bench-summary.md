# Benchmark Summary

> Generated: 2026-02-27 06:59:43
> Commit: 26bece4-dirty
> Version: husako v0.1.0
> Platform: x86_64-linux
> CPU: Intel(R) Xeon(R) Platinum 8370C CPU @ 2.80GHz (4 cores)
> Memory: 16 GiB
> Runner: GitHub Actions

| Group | Benchmark | Mean | ± Std Dev |
|-------|-----------|------|-----------|
| compile | large | 28.574 µs | ± 472.48 ns |
| compile | medium | 15.978 µs | ± 147.85 ns |
| compile | small | 10.253 µs | ± 147.67 ns |
| emit_yaml | 1 | 21.059 µs | ± 406.36 ns |
| emit_yaml | 10 | 210.547 µs | ± 2.692 µs |
| emit_yaml | 50 | 1.056 ms | ± 29.118 µs |
| execute | builtin_medium | 1.162 ms | ± 13.773 µs |
| execute | builtin_small | 1.047 ms | ± 30.441 µs |
| execute | k8s_medium | 2.077 ms | ± 9.121 µs |
| execute | k8s_small | 1.947 ms | ± 23.489 µs |
| generate | core_v1 | 4.775 ms | ± 32.324 µs |
| generate | full_k8s | 10.075 ms | ± 346.900 µs |
| generate | full_k8s_crds | 11.981 ms | ± 487.172 µs |
| render | builtin_small | 1.088 ms | ± 21.730 µs |
| render | k8s_medium | 2.182 ms | ± 12.814 µs |
| render | k8s_small | 1.989 ms | ± 15.664 µs |
