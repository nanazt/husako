# Benchmark Report

> Generated: 2026-02-27 04:56:20
> Commit: 0416e85-dirty
> Version: husako v0.1.0
> Platform: x86_64-linux
> CPU: AMD EPYC 7763 64-Core Processor (4 cores)
> Memory: 16 GiB
> Runner: GitHub Actions

## compile

### large
- **Mean**: 29.564 µs (29.293 µs – 29.869 µs, 95% CI)
- **Std Dev**: 1.490 µs
- **Slope**: 29.857 µs

### medium
- **Mean**: 16.274 µs (16.181 µs – 16.364 µs, 95% CI)
- **Std Dev**: 470.83 ns
- **Slope**: 16.504 µs

### small
- **Mean**: 10.338 µs (10.284 µs – 10.397 µs, 95% CI)
- **Std Dev**: 285.47 ns
- **Slope**: 10.276 µs

## emit_yaml

### 1
- **Mean**: 22.150 µs (22.029 µs – 22.269 µs, 95% CI)
- **Std Dev**: 614.18 ns
- **Slope**: 22.219 µs

### 10
- **Mean**: 223.760 µs (222.150 µs – 225.637 µs, 95% CI)
- **Std Dev**: 8.908 µs
- **Slope**: 225.361 µs

### 50
- **Mean**: 1.107 ms (1.099 ms – 1.115 ms, 95% CI)
- **Std Dev**: 40.558 µs
- **Slope**: 1.103 ms

## execute

### builtin_medium
- **Mean**: 1.191 ms (1.183 ms – 1.199 ms, 95% CI)
- **Std Dev**: 40.847 µs
- **Slope**: 1.177 ms

### builtin_small
- **Mean**: 1.044 ms (1.038 ms – 1.049 ms, 95% CI)
- **Std Dev**: 28.964 µs
- **Slope**: 1.056 ms

### k8s_medium
- **Mean**: 2.229 ms (2.212 ms – 2.245 ms, 95% CI)
- **Std Dev**: 84.056 µs

### k8s_small
- **Mean**: 2.025 ms (2.011 ms – 2.039 ms, 95% CI)
- **Std Dev**: 71.157 µs

## generate

### core_v1
- **Mean**: 4.946 ms (4.920 ms – 4.971 ms, 95% CI)
- **Std Dev**: 131.692 µs

### full_k8s
- **Mean**: 10.940 ms (10.854 ms – 11.023 ms, 95% CI)
- **Std Dev**: 436.079 µs

### full_k8s_crds
- **Mean**: 12.503 ms (12.400 ms – 12.604 ms, 95% CI)
- **Std Dev**: 519.332 µs

## render

### builtin_small
- **Mean**: 1.086 ms (1.079 ms – 1.093 ms, 95% CI)
- **Std Dev**: 34.371 µs
- **Slope**: 1.082 ms

### k8s_medium
- **Mean**: 2.234 ms (2.222 ms – 2.246 ms, 95% CI)
- **Std Dev**: 63.045 µs

### k8s_small
- **Mean**: 2.123 ms (2.105 ms – 2.140 ms, 95% CI)
- **Std Dev**: 88.651 µs
