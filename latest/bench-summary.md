# Benchmark Summary

> Generated: 2026-02-25 13:53:09
> Commit: 385f163-dirty
> Version: husako v0.1.0
> Platform: x86_64-linux
> CPU: AMD EPYC 7763 64-Core Processor (4 cores)
> Memory: 16 GiB
> Runner: GitHub Actions

| Group | Benchmark | Mean | ± Std Dev |
|-------|-----------|------|-----------|
| compile | large | 30.684 µs | ± 1.112 µs |
| compile | medium | 16.952 µs | ± 145.58 ns |
| compile | small | 10.671 µs | ± 145.58 ns |
| emit_yaml | 1 | 22.011 µs | ± 186.74 ns |
| emit_yaml | 10 | 220.954 µs | ± 3.381 µs |
| emit_yaml | 50 | 1.109 ms | ± 13.168 µs |
| execute | builtin_medium | 1.203 ms | ± 21.803 µs |
| execute | builtin_small | 1.075 ms | ± 30.289 µs |
| execute | k8s_medium | 2.287 ms | ± 47.472 µs |
| execute | k8s_small | 2.130 ms | ± 56.975 µs |
| generate | core_v1 | 5.434 ms | ± 173.193 µs |
| generate | full_k8s | 12.335 ms | ± 654.182 µs |
| generate | full_k8s_crds | 13.330 ms | ± 941.082 µs |
| render | builtin_small | 1.126 ms | ± 29.051 µs |
| render | k8s_medium | 2.269 ms | ± 15.731 µs |
| render | k8s_small | 2.102 ms | ± 16.791 µs |
