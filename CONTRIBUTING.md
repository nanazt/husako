# Contributing to husako

## Reporting issues

Use the issue templates on GitHub:

- **Bug report** — unexpected behavior, wrong output, CLI errors
- **Feature request** — new commands, config options, integrations

Blank issues are also allowed for questions or discussions.

---

## Development setup

Install [Rust stable](https://rustup.rs/).

---

## Build

```bash
cargo build                # debug
cargo build --release      # release (optimized for size)
```

---

## Before opening a PR

Run these in order and fix any failures before pushing:

```bash
# 1. Format
cargo fmt --all

# 2. Lint (all warnings are errors)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 3. Tests
cargo test --workspace --all-features
```

For changes touching `husako-helm`, `husako-core`, `husako-dts`, `husako-runtime-qjs`, `husako-sdk`, or `husako-cli`, also run the local E2E suite (no network required):

```bash
cargo test -p husako --test e2e_g
```

---

## Pull request process

1. Create a branch from `master` and push it
2. Open a PR to `master` — CI runs automatically
3. All checks must pass before merge
4. **Squash merge only** — keeps `master` linear

---

## Editing CI workflows

Workflows are written in TypeScript and compiled to YAML using [gaji](https://github.com/dodok8/gaji). **Never edit `.github/workflows/*.yml` directly** — changes will be overwritten.

```bash
gaji dev     # generate types
gaji build   # compile workflows/**.ts → .github/workflows/
```

---

## CI/CD pipeline

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `check.yml` | PRs and pushes to `master` | Format check, clippy, tests |
| `version.yml` | Manual (`workflow_dispatch`) + `v*` tag | release-plz publishes changed crates and creates GitHub Release |
| `distribute.yml` | `v*` tag | Cross-platform binary builds, GitHub release assets, npm publish |
| `audit.yml` | Weekly (Monday) | `cargo audit` security scan |
| `sync-workflows.yml` | Push to `master` changing `workflows/**` | Regenerates workflow YAML |
