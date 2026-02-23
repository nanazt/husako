# Contributing to husako

## Development workflow

1. **Create a branch** from `master` and push it
2. **Open a PR** to `master` — this triggers quality gates automatically
3. **Quality gates** (`lint` and `test`) must pass before merge
4. **Squash merge** — the only merge strategy allowed, keeps `master` linear
5. **Release PR** — release-plz creates a PR with version bumps and changelog when changes land on `master`
6. **Release** — merging the release PR publishes crates, creates a git tag, and triggers binary distribution

## Building

```bash
cargo build                       # debug build
cargo build --release             # release build (optimized for size)
```

## Testing

```bash
cargo test --workspace --all-features
```

## Linting

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Workflows

Workflows are authored in TypeScript using [gaji](https://github.com/dodok8/gaji) and compiled to YAML.

```bash
gaji dev           # generate types
gaji build         # compile workflows to .github/workflows/
```

Source files live in `workflows/`, output goes to `.github/workflows/`. Do not edit the YAML files directly.

## CI/CD pipeline

| Workflow | Trigger | Purpose |
| --- | --- | --- |
| `check.yml` | PRs and pushes to `master` | Format check, clippy, tests |
| `version.yml` | Push to `master` | release-plz creates release PR and publishes crates |
| `distribute.yml` | `v*` tag | Cross-platform binary builds, GitHub release assets, npm publish |
| `audit.yml` | Weekly (Monday) | `cargo audit` security scan |
| `sync-workflows.yml` | Push to `master` changing `workflows/**` | Regenerates workflow YAML |
