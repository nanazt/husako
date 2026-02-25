# Benchmark Report

> Generated: 2026-02-25 13:53:09
> Commit: 385f163-dirty
> Version: husako v0.1.0
> Platform: x86_64-linux
> CPU: AMD EPYC 7763 64-Core Processor (4 cores)
> Memory: 16 GiB
> Runner: GitHub Actions

## compile

### large
- **Mean**: 30.684 µs (30.502 µs – 30.931 µs, 95% CI)
- **Std Dev**: 1.112 µs
- **Slope**: 30.739 µs

### medium
- **Mean**: 16.952 µs (16.926 µs – 16.983 µs, 95% CI)
- **Std Dev**: 145.58 ns
- **Slope**: 16.969 µs

### small
- **Mean**: 10.671 µs (10.648 µs – 10.704 µs, 95% CI)
- **Std Dev**: 145.58 ns
- **Slope**: 10.650 µs

## emit_yaml

### 1
- **Mean**: 22.011 µs (21.984 µs – 22.054 µs, 95% CI)
- **Std Dev**: 186.74 ns
- **Slope**: 21.986 µs

### 10
- **Mean**: 220.954 µs (220.550 µs – 221.683 µs, 95% CI)
- **Std Dev**: 3.381 µs
- **Slope**: 220.642 µs

### 50
- **Mean**: 1.109 ms (1.107 ms – 1.112 ms, 95% CI)
- **Std Dev**: 13.168 µs
- **Slope**: 1.108 ms

## execute

### builtin_medium
- **Mean**: 1.203 ms (1.199 ms – 1.207 ms, 95% CI)
- **Std Dev**: 21.803 µs
- **Slope**: 1.193 ms

### builtin_small
- **Mean**: 1.075 ms (1.070 ms – 1.082 ms, 95% CI)
- **Std Dev**: 30.289 µs
- **Slope**: 1.073 ms

### k8s_medium
- **Mean**: 2.287 ms (2.277 ms – 2.296 ms, 95% CI)
- **Std Dev**: 47.472 µs

### k8s_small
- **Mean**: 2.130 ms (2.119 ms – 2.141 ms, 95% CI)
- **Std Dev**: 56.975 µs

## generate

### core_v1
- **Mean**: 5.434 ms (5.401 ms – 5.469 ms, 95% CI)
- **Std Dev**: 173.193 µs

### full_k8s
- **Mean**: 12.335 ms (12.208 ms – 12.464 ms, 95% CI)
- **Std Dev**: 654.182 µs

### full_k8s_crds
- **Mean**: 13.330 ms (13.147 ms – 13.513 ms, 95% CI)
- **Std Dev**: 941.082 µs

## render

### builtin_small
- **Mean**: 1.126 ms (1.121 ms – 1.132 ms, 95% CI)
- **Std Dev**: 29.051 µs
- **Slope**: 1.118 ms

### k8s_medium
- **Mean**: 2.269 ms (2.266 ms – 2.272 ms, 95% CI)
- **Std Dev**: 15.731 µs

### k8s_small
- **Mean**: 2.102 ms (2.099 ms – 2.105 ms, 95% CI)
- **Std Dev**: 16.791 µs
