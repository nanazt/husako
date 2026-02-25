# Benchmark Summary

> Generated: 2026-02-25 14:31:11
> Commit: 836c2a5-dirty
> Version: husako v0.1.0
> Platform: x86_64-linux
> CPU: AMD EPYC 7763 64-Core Processor (4 cores)
> Memory: 16 GiB
> Runner: GitHub Actions

| Group | Benchmark | Mean | ± Std Dev |
|-------|-----------|------|-----------|
| compile | large | 29.708 µs | ± 624.26 ns |
| compile | medium | 16.787 µs | ± 172.57 ns |
| compile | small | 10.654 µs | ± 190.47 ns |
| emit_yaml | 1 | 22.816 µs | ± 252.36 ns |
| emit_yaml | 10 | 226.788 µs | ± 918.76 ns |
| emit_yaml | 50 | 1.147 ms | ± 10.174 µs |
| execute | builtin_medium | 1.197 ms | ± 9.720 µs |
| execute | builtin_small | 1.075 ms | ± 28.155 µs |
| execute | k8s_medium | 2.164 ms | ± 7.110 µs |
| execute | k8s_small | 2.133 ms | ± 47.922 µs |
| generate | core_v1 | 5.290 ms | ± 79.950 µs |
| generate | full_k8s | 10.881 ms | ± 596.933 µs |
| generate | full_k8s_crds | 11.993 ms | ± 818.517 µs |
| render | builtin_small | 1.133 ms | ± 32.918 µs |
| render | k8s_medium | 2.284 ms | ± 19.691 µs |
| render | k8s_small | 2.120 ms | ± 16.588 µs |
