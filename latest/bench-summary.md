# Benchmark Summary

> Generated: 2026-02-26 20:35:27
> Commit: b3a3b1f-dirty
> Version: husako v0.1.0
> Platform: x86_64-linux
> CPU: AMD EPYC 7763 64-Core Processor (4 cores)
> Memory: 16 GiB
> Runner: GitHub Actions

| Group | Benchmark | Mean | ± Std Dev |
|-------|-----------|------|-----------|
| compile | large | 30.514 µs | ± 414.10 ns |
| compile | medium | 16.906 µs | ± 183.82 ns |
| compile | small | 10.791 µs | ± 303.55 ns |
| emit_yaml | 1 | 23.166 µs | ± 179.68 ns |
| emit_yaml | 10 | 233.551 µs | ± 3.073 µs |
| emit_yaml | 50 | 1.173 ms | ± 25.751 µs |
| execute | builtin_medium | 1.194 ms | ± 23.505 µs |
| execute | builtin_small | 1.065 ms | ± 28.402 µs |
| execute | k8s_medium | 2.273 ms | ± 69.775 µs |
| execute | k8s_small | 2.171 ms | ± 29.748 µs |
| generate | core_v1 | 5.298 ms | ± 90.988 µs |
| generate | full_k8s | 11.593 ms | ± 698.466 µs |
| generate | full_k8s_crds | 14.553 ms | ± 708.067 µs |
| render | builtin_small | 1.144 ms | ± 17.925 µs |
| render | k8s_medium | 2.422 ms | ± 84.328 µs |
| render | k8s_small | 2.102 ms | ± 5.886 µs |
