# Benchmark Summary

> Generated: 2026-02-26 19:45:14
> Commit: 782a007-dirty
> Version: husako v0.1.0
> Platform: x86_64-linux
> CPU: Intel(R) Xeon(R) Platinum 8370C CPU @ 2.80GHz (4 cores)
> Memory: 16 GiB
> Runner: GitHub Actions

| Group | Benchmark | Mean | ± Std Dev |
|-------|-----------|------|-----------|
| compile | large | 28.461 µs | ± 143.00 ns |
| compile | medium | 15.966 µs | ± 91.87 ns |
| compile | small | 10.256 µs | ± 231.13 ns |
| emit_yaml | 1 | 21.308 µs | ± 391.97 ns |
| emit_yaml | 10 | 212.541 µs | ± 3.132 µs |
| emit_yaml | 50 | 1.068 ms | ± 23.480 µs |
| execute | builtin_medium | 1.167 ms | ± 19.275 µs |
| execute | builtin_small | 1.034 ms | ± 22.286 µs |
| execute | k8s_medium | 2.063 ms | ± 8.104 µs |
| execute | k8s_small | 1.949 ms | ± 7.194 µs |
| generate | core_v1 | 4.667 ms | ± 36.844 µs |
| generate | full_k8s | 9.686 ms | ± 246.033 µs |
| generate | full_k8s_crds | 11.744 ms | ± 211.733 µs |
| render | builtin_small | 1.067 ms | ± 28.833 µs |
| render | k8s_medium | 2.185 ms | ± 36.463 µs |
| render | k8s_small | 2.008 ms | ± 10.652 µs |
