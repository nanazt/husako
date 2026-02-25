# Benchmark Report

> Generated: 2026-02-25 10:15:48
> Commit: 589496e-dirty
> Version: husako v0.1.0
> Platform: x86_64-linux
> CPU: AMD EPYC 7763 64-Core Processor (4 cores)
> Memory: 16 GiB
> Runner: GitHub Actions

## compile

### large
- **Mean**: 30.150 µs (30.053 µs – 30.276 µs, 95% CI)
- **Std Dev**: 577.70 ns
- **Slope**: 30.050 µs

### medium
- **Mean**: 17.009 µs (16.905 µs – 17.170 µs, 95% CI)
- **Std Dev**: 712.55 ns
- **Slope**: 16.920 µs

### small
- **Mean**: 10.599 µs (10.583 µs – 10.620 µs, 95% CI)
- **Std Dev**: 95.53 ns
- **Slope**: 10.575 µs

## emit_yaml

### 1
- **Mean**: 22.264 µs (22.200 µs – 22.353 µs, 95% CI)
- **Std Dev**: 402.90 ns
- **Slope**: 22.280 µs

### 10
- **Mean**: 223.926 µs (223.535 µs – 224.426 µs, 95% CI)
- **Std Dev**: 2.284 µs
- **Slope**: 224.013 µs

### 50
- **Mean**: 1.130 ms (1.125 ms – 1.136 ms, 95% CI)
- **Std Dev**: 27.731 µs
- **Slope**: 1.129 ms

## execute

### builtin_medium
- **Mean**: 1.139 ms (1.130 ms – 1.150 ms, 95% CI)
- **Std Dev**: 50.981 µs
- **Slope**: 1.140 ms

### builtin_small
- **Mean**: 993.996 µs (992.421 µs – 996.173 µs, 95% CI)
- **Std Dev**: 9.881 µs
- **Slope**: 994.057 µs

### k8s_medium
- **Mean**: 2.117 ms (2.114 ms – 2.120 ms, 95% CI)
- **Std Dev**: 13.648 µs

### k8s_small
- **Mean**: 1.991 ms (1.987 ms – 1.997 ms, 95% CI)
- **Std Dev**: 28.389 µs

## generate

### core_v1
- **Mean**: 5.508 ms (5.478 ms – 5.540 ms, 95% CI)
- **Std Dev**: 158.506 µs

### full_k8s
- **Mean**: 12.445 ms (12.298 ms – 12.605 ms, 95% CI)
- **Std Dev**: 789.713 µs

### full_k8s_crds
- **Mean**: 14.331 ms (14.172 ms – 14.486 ms, 95% CI)
- **Std Dev**: 803.425 µs

## render

### builtin_small
- **Mean**: 1.067 ms (1.064 ms – 1.073 ms, 95% CI)
- **Std Dev**: 27.658 µs
- **Slope**: 1.065 ms

### k8s_medium
- **Mean**: 2.230 ms (2.225 ms – 2.237 ms, 95% CI)
- **Std Dev**: 31.641 µs

### k8s_small
- **Mean**: 2.046 ms (2.045 ms – 2.047 ms, 95% CI)
- **Std Dev**: 5.690 µs
