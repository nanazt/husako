# Benchmark Summary

> Generated: 2026-02-25 10:15:48
> Commit: 589496e-dirty
> Version: husako v0.1.0
> Platform: x86_64-linux
> CPU: AMD EPYC 7763 64-Core Processor (4 cores)
> Memory: 16 GiB
> Runner: GitHub Actions

| Group | Benchmark | Mean | ± Std Dev |
|-------|-----------|------|-----------|
| compile | large | 30.150 µs | ± 577.70 ns |
| compile | medium | 17.009 µs | ± 712.55 ns |
| compile | small | 10.599 µs | ± 95.53 ns |
| emit_yaml | 1 | 22.264 µs | ± 402.90 ns |
| emit_yaml | 10 | 223.926 µs | ± 2.284 µs |
| emit_yaml | 50 | 1.130 ms | ± 27.731 µs |
| execute | builtin_medium | 1.139 ms | ± 50.981 µs |
| execute | builtin_small | 993.996 µs | ± 9.881 µs |
| execute | k8s_medium | 2.117 ms | ± 13.648 µs |
| execute | k8s_small | 1.991 ms | ± 28.389 µs |
| generate | core_v1 | 5.508 ms | ± 158.506 µs |
| generate | full_k8s | 12.445 ms | ± 789.713 µs |
| generate | full_k8s_crds | 14.331 ms | ± 803.425 µs |
| render | builtin_small | 1.067 ms | ± 27.658 µs |
| render | k8s_medium | 2.230 ms | ± 31.641 µs |
| render | k8s_small | 2.046 ms | ± 5.690 µs |
