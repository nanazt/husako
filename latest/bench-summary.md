# Benchmark Summary

> Generated: 2026-02-26 19:38:59
> Commit: 2e24d7f-dirty
> Version: husako v0.1.0
> Platform: x86_64-linux
> CPU: AMD EPYC 7763 64-Core Processor (4 cores)
> Memory: 16 GiB
> Runner: GitHub Actions

| Group | Benchmark | Mean | ± Std Dev |
|-------|-----------|------|-----------|
| compile | large | 30.665 µs | ± 456.34 ns |
| compile | medium | 17.098 µs | ± 474.29 ns |
| compile | small | 10.578 µs | ± 184.02 ns |
| emit_yaml | 1 | 22.785 µs | ± 223.44 ns |
| emit_yaml | 10 | 227.465 µs | ± 1.458 µs |
| emit_yaml | 50 | 1.149 ms | ± 9.117 µs |
| execute | builtin_medium | 1.191 ms | ± 36.169 µs |
| execute | builtin_small | 1.084 ms | ± 69.910 µs |
| execute | k8s_medium | 2.189 ms | ± 57.150 µs |
| execute | k8s_small | 2.048 ms | ± 6.349 µs |
| generate | core_v1 | 5.127 ms | ± 30.731 µs |
| generate | full_k8s | 10.676 ms | ± 760.886 µs |
| generate | full_k8s_crds | 12.436 ms | ± 566.199 µs |
| render | builtin_small | 1.130 ms | ± 35.082 µs |
| render | k8s_medium | 2.264 ms | ± 17.679 µs |
| render | k8s_small | 2.095 ms | ± 19.163 µs |
