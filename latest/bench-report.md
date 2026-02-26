# Benchmark Report

> Generated: 2026-02-26 20:35:27
> Commit: b3a3b1f-dirty
> Version: husako v0.1.0
> Platform: x86_64-linux
> CPU: AMD EPYC 7763 64-Core Processor (4 cores)
> Memory: 16 GiB
> Runner: GitHub Actions

## compile

### large
- **Mean**: 30.514 µs (30.445 µs – 30.605 µs, 95% CI)
- **Std Dev**: 414.10 ns
- **Slope**: 30.467 µs

### medium
- **Mean**: 16.906 µs (16.878 µs – 16.948 µs, 95% CI)
- **Std Dev**: 183.82 ns
- **Slope**: 16.897 µs

### small
- **Mean**: 10.791 µs (10.737 µs – 10.854 µs, 95% CI)
- **Std Dev**: 303.55 ns
- **Slope**: 10.762 µs

## emit_yaml

### 1
- **Mean**: 23.166 µs (23.137 µs – 23.206 µs, 95% CI)
- **Std Dev**: 179.68 ns
- **Slope**: 23.135 µs

### 10
- **Mean**: 233.551 µs (233.090 µs – 234.252 µs, 95% CI)
- **Std Dev**: 3.073 µs
- **Slope**: 233.205 µs

### 50
- **Mean**: 1.173 ms (1.169 ms – 1.179 ms, 95% CI)
- **Std Dev**: 25.751 µs
- **Slope**: 1.174 ms

## execute

### builtin_medium
- **Mean**: 1.194 ms (1.189 ms – 1.198 ms, 95% CI)
- **Std Dev**: 23.505 µs
- **Slope**: 1.206 ms

### builtin_small
- **Mean**: 1.065 ms (1.061 ms – 1.072 ms, 95% CI)
- **Std Dev**: 28.402 µs
- **Slope**: 1.058 ms

### k8s_medium
- **Mean**: 2.273 ms (2.259 ms – 2.286 ms, 95% CI)
- **Std Dev**: 69.775 µs

### k8s_small
- **Mean**: 2.171 ms (2.165 ms – 2.176 ms, 95% CI)
- **Std Dev**: 29.748 µs

## generate

### core_v1
- **Mean**: 5.298 ms (5.281 ms – 5.317 ms, 95% CI)
- **Std Dev**: 90.988 µs

### full_k8s
- **Mean**: 11.593 ms (11.457 ms – 11.730 ms, 95% CI)
- **Std Dev**: 698.466 µs

### full_k8s_crds
- **Mean**: 14.553 ms (14.416 ms – 14.693 ms, 95% CI)
- **Std Dev**: 708.067 µs

## render

### builtin_small
- **Mean**: 1.144 ms (1.141 ms – 1.148 ms, 95% CI)
- **Std Dev**: 17.925 µs
- **Slope**: 1.147 ms

### k8s_medium
- **Mean**: 2.422 ms (2.405 ms – 2.438 ms, 95% CI)
- **Std Dev**: 84.328 µs

### k8s_small
- **Mean**: 2.102 ms (2.101 ms – 2.103 ms, 95% CI)
- **Std Dev**: 5.886 µs
