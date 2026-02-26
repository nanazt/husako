# Benchmark Report

> Generated: 2026-02-26 19:38:59
> Commit: 2e24d7f-dirty
> Version: husako v0.1.0
> Platform: x86_64-linux
> CPU: AMD EPYC 7763 64-Core Processor (4 cores)
> Memory: 16 GiB
> Runner: GitHub Actions

## compile

### large
- **Mean**: 30.665 µs (30.592 µs – 30.766 µs, 95% CI)
- **Std Dev**: 456.34 ns
- **Slope**: 30.650 µs

### medium
- **Mean**: 17.098 µs (17.031 µs – 17.208 µs, 95% CI)
- **Std Dev**: 474.29 ns
- **Slope**: 17.093 µs

### small
- **Mean**: 10.578 µs (10.550 µs – 10.620 µs, 95% CI)
- **Std Dev**: 184.02 ns
- **Slope**: 10.560 µs

## emit_yaml

### 1
- **Mean**: 22.785 µs (22.749 µs – 22.835 µs, 95% CI)
- **Std Dev**: 223.44 ns
- **Slope**: 22.731 µs

### 10
- **Mean**: 227.465 µs (227.226 µs – 227.792 µs, 95% CI)
- **Std Dev**: 1.458 µs
- **Slope**: 227.377 µs

### 50
- **Mean**: 1.149 ms (1.148 ms – 1.151 ms, 95% CI)
- **Std Dev**: 9.117 µs
- **Slope**: 1.147 ms

## execute

### builtin_medium
- **Mean**: 1.191 ms (1.185 ms – 1.199 ms, 95% CI)
- **Std Dev**: 36.169 µs
- **Slope**: 1.203 ms

### builtin_small
- **Mean**: 1.084 ms (1.074 ms – 1.100 ms, 95% CI)
- **Std Dev**: 69.910 µs
- **Slope**: 1.076 ms

### k8s_medium
- **Mean**: 2.189 ms (2.179 ms – 2.201 ms, 95% CI)
- **Std Dev**: 57.150 µs

### k8s_small
- **Mean**: 2.048 ms (2.047 ms – 2.050 ms, 95% CI)
- **Std Dev**: 6.349 µs

## generate

### core_v1
- **Mean**: 5.127 ms (5.122 ms – 5.134 ms, 95% CI)
- **Std Dev**: 30.731 µs

### full_k8s
- **Mean**: 10.676 ms (10.532 ms – 10.829 ms, 95% CI)
- **Std Dev**: 760.886 µs

### full_k8s_crds
- **Mean**: 12.436 ms (12.324 ms – 12.544 ms, 95% CI)
- **Std Dev**: 566.199 µs

## render

### builtin_small
- **Mean**: 1.130 ms (1.124 ms – 1.137 ms, 95% CI)
- **Std Dev**: 35.082 µs
- **Slope**: 1.135 ms

### k8s_medium
- **Mean**: 2.264 ms (2.261 ms – 2.267 ms, 95% CI)
- **Std Dev**: 17.679 µs

### k8s_small
- **Mean**: 2.095 ms (2.092 ms – 2.099 ms, 95% CI)
- **Std Dev**: 19.163 µs
