# Benchmark Report

> Generated: 2026-02-25 14:31:11
> Commit: 836c2a5-dirty
> Version: husako v0.1.0
> Platform: x86_64-linux
> CPU: AMD EPYC 7763 64-Core Processor (4 cores)
> Memory: 16 GiB
> Runner: GitHub Actions

## compile

### large
- **Mean**: 29.708 µs (29.626 µs – 29.846 µs, 95% CI)
- **Std Dev**: 624.26 ns
- **Slope**: 29.675 µs

### medium
- **Mean**: 16.787 µs (16.765 µs – 16.826 µs, 95% CI)
- **Std Dev**: 172.57 ns
- **Slope**: 16.778 µs

### small
- **Mean**: 10.654 µs (10.625 µs – 10.697 µs, 95% CI)
- **Std Dev**: 190.47 ns
- **Slope**: 10.629 µs

## emit_yaml

### 1
- **Mean**: 22.816 µs (22.773 µs – 22.870 µs, 95% CI)
- **Std Dev**: 252.36 ns
- **Slope**: 22.819 µs

### 10
- **Mean**: 226.788 µs (226.628 µs – 226.985 µs, 95% CI)
- **Std Dev**: 918.76 ns
- **Slope**: 226.546 µs

### 50
- **Mean**: 1.147 ms (1.145 ms – 1.149 ms, 95% CI)
- **Std Dev**: 10.174 µs
- **Slope**: 1.147 ms

## execute

### builtin_medium
- **Mean**: 1.197 ms (1.195 ms – 1.198 ms, 95% CI)
- **Std Dev**: 9.720 µs
- **Slope**: 1.202 ms

### builtin_small
- **Mean**: 1.075 ms (1.070 ms – 1.081 ms, 95% CI)
- **Std Dev**: 28.155 µs
- **Slope**: 1.069 ms

### k8s_medium
- **Mean**: 2.164 ms (2.163 ms – 2.166 ms, 95% CI)
- **Std Dev**: 7.110 µs

### k8s_small
- **Mean**: 2.133 ms (2.123 ms – 2.142 ms, 95% CI)
- **Std Dev**: 47.922 µs

## generate

### core_v1
- **Mean**: 5.290 ms (5.277 ms – 5.308 ms, 95% CI)
- **Std Dev**: 79.950 µs

### full_k8s
- **Mean**: 10.881 ms (10.764 ms – 10.997 ms, 95% CI)
- **Std Dev**: 596.933 µs

### full_k8s_crds
- **Mean**: 11.993 ms (11.837 ms – 12.154 ms, 95% CI)
- **Std Dev**: 818.517 µs

## render

### builtin_small
- **Mean**: 1.133 ms (1.127 ms – 1.140 ms, 95% CI)
- **Std Dev**: 32.918 µs
- **Slope**: 1.127 ms

### k8s_medium
- **Mean**: 2.284 ms (2.280 ms – 2.288 ms, 95% CI)
- **Std Dev**: 19.691 µs

### k8s_small
- **Mean**: 2.120 ms (2.117 ms – 2.123 ms, 95% CI)
- **Std Dev**: 16.588 µs
