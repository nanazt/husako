# Benchmark Summary

> Generated: 2026-02-27 04:56:20
> Commit: 0416e85-dirty
> Version: husako v0.1.0
> Platform: x86_64-linux
> CPU: AMD EPYC 7763 64-Core Processor (4 cores)
> Memory: 16 GiB
> Runner: GitHub Actions

| Group | Benchmark | Mean | ± Std Dev |
|-------|-----------|------|-----------|
| compile | large | 29.564 µs | ± 1.490 µs |
| compile | medium | 16.274 µs | ± 470.83 ns |
| compile | small | 10.338 µs | ± 285.47 ns |
| emit_yaml | 1 | 22.150 µs | ± 614.18 ns |
| emit_yaml | 10 | 223.760 µs | ± 8.908 µs |
| emit_yaml | 50 | 1.107 ms | ± 40.558 µs |
| execute | builtin_medium | 1.191 ms | ± 40.847 µs |
| execute | builtin_small | 1.044 ms | ± 28.964 µs |
| execute | k8s_medium | 2.229 ms | ± 84.056 µs |
| execute | k8s_small | 2.025 ms | ± 71.157 µs |
| generate | core_v1 | 4.946 ms | ± 131.692 µs |
| generate | full_k8s | 10.940 ms | ± 436.079 µs |
| generate | full_k8s_crds | 12.503 ms | ± 519.332 µs |
| render | builtin_small | 1.086 ms | ± 34.371 µs |
| render | k8s_medium | 2.234 ms | ± 63.045 µs |
| render | k8s_small | 2.123 ms | ± 88.651 µs |
