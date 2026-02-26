# Benchmark Report

> Generated: 2026-02-26 20:27:44
> Commit: af76fe8-dirty
> Version: husako v0.1.0
> Platform: x86_64-linux
> CPU: AMD EPYC 7763 64-Core Processor (4 cores)
> Memory: 16 GiB
> Runner: GitHub Actions

## compile

### large
- **Mean**: 30.682 µs (30.625 µs – 30.764 µs, 95% CI)
- **Std Dev**: 363.66 ns
- **Slope**: 30.674 µs

### medium
- **Mean**: 17.150 µs (17.125 µs – 17.187 µs, 95% CI)
- **Std Dev**: 162.57 ns
- **Slope**: 17.095 µs

### small
- **Mean**: 10.815 µs (10.744 µs – 10.910 µs, 95% CI)
- **Std Dev**: 430.35 ns
- **Slope**: 10.899 µs

## emit_yaml

### 1
- **Mean**: 23.086 µs (23.060 µs – 23.122 µs, 95% CI)
- **Std Dev**: 163.13 ns
- **Slope**: 23.104 µs

### 10
- **Mean**: 231.374 µs (230.812 µs – 232.296 µs, 95% CI)
- **Std Dev**: 4.054 µs
- **Slope**: 230.945 µs

### 50
- **Mean**: 1.201 ms (1.195 ms – 1.208 ms, 95% CI)
- **Std Dev**: 33.613 µs
- **Slope**: 1.205 ms

## execute

### builtin_medium
- **Mean**: 1.211 ms (1.207 ms – 1.216 ms, 95% CI)
- **Std Dev**: 22.885 µs
- **Slope**: 1.203 ms

### builtin_small
- **Mean**: 1.062 ms (1.058 ms – 1.066 ms, 95% CI)
- **Std Dev**: 19.632 µs
- **Slope**: 1.064 ms

### k8s_medium
- **Mean**: 2.252 ms (2.238 ms – 2.265 ms, 95% CI)
- **Std Dev**: 67.610 µs

### k8s_small
- **Mean**: 2.101 ms (2.089 ms – 2.114 ms, 95% CI)
- **Std Dev**: 66.204 µs

## generate

### core_v1
- **Mean**: 5.313 ms (5.290 ms – 5.336 ms, 95% CI)
- **Std Dev**: 118.019 µs

### full_k8s
- **Mean**: 10.631 ms (10.524 ms – 10.739 ms, 95% CI)
- **Std Dev**: 547.849 µs

### full_k8s_crds
- **Mean**: 13.462 ms (13.308 ms – 13.614 ms, 95% CI)
- **Std Dev**: 784.523 µs

## render

### builtin_small
- **Mean**: 1.141 ms (1.135 ms – 1.149 ms, 95% CI)
- **Std Dev**: 35.940 µs
- **Slope**: 1.134 ms

### k8s_medium
- **Mean**: 2.337 ms (2.322 ms – 2.353 ms, 95% CI)
- **Std Dev**: 79.943 µs

### k8s_small
- **Mean**: 2.202 ms (2.188 ms – 2.216 ms, 95% CI)
- **Std Dev**: 73.276 µs
