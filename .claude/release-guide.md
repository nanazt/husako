# Release Guide

## Pre-release Checklist

Before tagging a release:

1. **Run benchmarks** to capture a performance snapshot for the release notes:
   ```bash
   cargo bench -p husako-bench
   cargo run -p husako-bench --bin report
   cat target/criterion/bench-summary.md
   ```

2. **Check for regressions**: compare with the previous release's results at
   `github.com/nanazt/husako/tree/bench-results/releases/` or via:
   ```bash
   git show origin/bench-results:releases/<prev-version>/bench-summary.md
   ```

3. **Include the bench summary in the GitHub Release description**.
   Copy the table from `bench-summary.md` and paste it into the release notes under
   a `## Performance` heading.

## Release Flow

(See also CLAUDE.md "CI/CD" section)

1. Merge PR to `master`
2. Run benchmarks locally (step 1 above)
3. Trigger "Version" workflow manually → release-plz publishes crates + creates GitHub Release draft
4. Edit the GitHub Release description to add the bench summary table
5. Publish the release → `v*` tag push → `distribute.yml` runs (binaries + npm)
   → `bench.yml` runs (stores results to `bench-results/releases/v*/`)

## Bench Results Storage

CI stores bench result markdown files to the `bench-results` branch on every run:

```
bench-results/
  latest/               ← overwritten on every master push or tag
    bench-summary.md
    bench-report.md
  releases/
    v0.3.0/             ← created only on v* tag push, permanent
      bench-summary.md
      bench-report.md
```

To inspect results after CI runs:
```bash
# Latest (after any master push)
git fetch origin bench-results
git show origin/bench-results:latest/bench-summary.md

# Specific release
git show origin/bench-results:releases/v0.3.0/bench-summary.md
```
