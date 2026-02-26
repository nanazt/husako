# Benchmark Summary

> Generated: 2026-02-26 20:27:44
> Commit: af76fe8-dirty
> Version: husako v0.1.0
> Platform: x86_64-linux
> CPU: AMD EPYC 7763 64-Core Processor (4 cores)
> Memory: 16 GiB
> Runner: GitHub Actions

| Group | Benchmark | Mean | ± Std Dev |
|-------|-----------|------|-----------|
| compile | large | 30.682 µs | ± 363.66 ns |
| compile | medium | 17.150 µs | ± 162.57 ns |
| compile | small | 10.815 µs | ± 430.35 ns |
| emit_yaml | 1 | 23.086 µs | ± 163.13 ns |
| emit_yaml | 10 | 231.374 µs | ± 4.054 µs |
| emit_yaml | 50 | 1.201 ms | ± 33.613 µs |
| execute | builtin_medium | 1.211 ms | ± 22.885 µs |
| execute | builtin_small | 1.062 ms | ± 19.632 µs |
| execute | k8s_medium | 2.252 ms | ± 67.610 µs |
| execute | k8s_small | 2.101 ms | ± 66.204 µs |
| generate | core_v1 | 5.313 ms | ± 118.019 µs |
| generate | full_k8s | 10.631 ms | ± 547.849 µs |
| generate | full_k8s_crds | 13.462 ms | ± 784.523 µs |
| render | builtin_small | 1.141 ms | ± 35.940 µs |
| render | k8s_medium | 2.337 ms | ± 79.943 µs |
| render | k8s_small | 2.202 ms | ± 73.276 µs |
