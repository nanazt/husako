# Benchmark Report

> Generated: 2026-02-27 06:59:43
> Commit: 26bece4-dirty
> Version: husako v0.1.0
> Platform: x86_64-linux
> CPU: Intel(R) Xeon(R) Platinum 8370C CPU @ 2.80GHz (4 cores)
> Memory: 16 GiB
> Runner: GitHub Actions

## compile

### large
- **Mean**: 28.574 µs (28.494 µs – 28.678 µs, 95% CI)
- **Std Dev**: 472.48 ns
- **Slope**: 28.552 µs

### medium
- **Mean**: 15.978 µs (15.957 µs – 16.012 µs, 95% CI)
- **Std Dev**: 147.85 ns
- **Slope**: 15.955 µs

### small
- **Mean**: 10.253 µs (10.228 µs – 10.285 µs, 95% CI)
- **Std Dev**: 147.67 ns
- **Slope**: 10.247 µs

## emit_yaml

### 1
- **Mean**: 21.059 µs (21.003 µs – 21.154 µs, 95% CI)
- **Std Dev**: 406.36 ns
- **Slope**: 21.031 µs

### 10
- **Mean**: 210.547 µs (210.106 µs – 211.144 µs, 95% CI)
- **Std Dev**: 2.692 µs
- **Slope**: 210.390 µs

### 50
- **Mean**: 1.056 ms (1.052 ms – 1.063 ms, 95% CI)
- **Std Dev**: 29.118 µs
- **Slope**: 1.052 ms

## execute

### builtin_medium
- **Mean**: 1.162 ms (1.159 ms – 1.165 ms, 95% CI)
- **Std Dev**: 13.773 µs
- **Slope**: 1.168 ms

### builtin_small
- **Mean**: 1.047 ms (1.042 ms – 1.054 ms, 95% CI)
- **Std Dev**: 30.441 µs
- **Slope**: 1.044 ms

### k8s_medium
- **Mean**: 2.077 ms (2.075 ms – 2.079 ms, 95% CI)
- **Std Dev**: 9.121 µs

### k8s_small
- **Mean**: 1.947 ms (1.944 ms – 1.952 ms, 95% CI)
- **Std Dev**: 23.489 µs
- **Slope**: 1.948 ms

## generate

### core_v1
- **Mean**: 4.775 ms (4.769 ms – 4.782 ms, 95% CI)
- **Std Dev**: 32.324 µs

### full_k8s
- **Mean**: 10.075 ms (10.007 ms – 10.143 ms, 95% CI)
- **Std Dev**: 346.900 µs

### full_k8s_crds
- **Mean**: 11.981 ms (11.886 ms – 12.077 ms, 95% CI)
- **Std Dev**: 487.172 µs

## render

### builtin_small
- **Mean**: 1.088 ms (1.084 ms – 1.093 ms, 95% CI)
- **Std Dev**: 21.730 µs
- **Slope**: 1.085 ms

### k8s_medium
- **Mean**: 2.182 ms (2.180 ms – 2.185 ms, 95% CI)
- **Std Dev**: 12.814 µs

### k8s_small
- **Mean**: 1.989 ms (1.986 ms – 1.992 ms, 95% CI)
- **Std Dev**: 15.664 µs
