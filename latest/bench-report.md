# Benchmark Report

> Generated: 2026-02-26 19:45:14
> Commit: 782a007-dirty
> Version: husako v0.1.0
> Platform: x86_64-linux
> CPU: Intel(R) Xeon(R) Platinum 8370C CPU @ 2.80GHz (4 cores)
> Memory: 16 GiB
> Runner: GitHub Actions

## compile

### large
- **Mean**: 28.461 µs (28.436 µs – 28.491 µs, 95% CI)
- **Std Dev**: 143.00 ns
- **Slope**: 28.491 µs

### medium
- **Mean**: 15.966 µs (15.950 µs – 15.986 µs, 95% CI)
- **Std Dev**: 91.87 ns
- **Slope**: 15.956 µs

### small
- **Mean**: 10.256 µs (10.217 µs – 10.306 µs, 95% CI)
- **Std Dev**: 231.13 ns
- **Slope**: 10.302 µs

## emit_yaml

### 1
- **Mean**: 21.308 µs (21.243 µs – 21.395 µs, 95% CI)
- **Std Dev**: 391.97 ns
- **Slope**: 21.268 µs

### 10
- **Mean**: 212.541 µs (212.152 µs – 213.222 µs, 95% CI)
- **Std Dev**: 3.132 µs
- **Slope**: 211.935 µs

### 50
- **Mean**: 1.068 ms (1.065 ms – 1.073 ms, 95% CI)
- **Std Dev**: 23.480 µs
- **Slope**: 1.063 ms

## execute

### builtin_medium
- **Mean**: 1.167 ms (1.164 ms – 1.171 ms, 95% CI)
- **Std Dev**: 19.275 µs
- **Slope**: 1.167 ms

### builtin_small
- **Mean**: 1.034 ms (1.031 ms – 1.039 ms, 95% CI)
- **Std Dev**: 22.286 µs
- **Slope**: 1.033 ms

### k8s_medium
- **Mean**: 2.063 ms (2.061 ms – 2.064 ms, 95% CI)
- **Std Dev**: 8.104 µs

### k8s_small
- **Mean**: 1.949 ms (1.947 ms – 1.950 ms, 95% CI)
- **Std Dev**: 7.194 µs
- **Slope**: 1.949 ms

## generate

### core_v1
- **Mean**: 4.667 ms (4.661 ms – 4.675 ms, 95% CI)
- **Std Dev**: 36.844 µs

### full_k8s
- **Mean**: 9.686 ms (9.639 ms – 9.735 ms, 95% CI)
- **Std Dev**: 246.033 µs

### full_k8s_crds
- **Mean**: 11.744 ms (11.701 ms – 11.784 ms, 95% CI)
- **Std Dev**: 211.733 µs

## render

### builtin_small
- **Mean**: 1.067 ms (1.062 ms – 1.073 ms, 95% CI)
- **Std Dev**: 28.833 µs
- **Slope**: 1.078 ms

### k8s_medium
- **Mean**: 2.185 ms (2.179 ms – 2.193 ms, 95% CI)
- **Std Dev**: 36.463 µs

### k8s_small
- **Mean**: 2.008 ms (2.006 ms – 2.010 ms, 95% CI)
- **Std Dev**: 10.652 µs
