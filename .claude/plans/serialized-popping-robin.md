# Plan: CI/CD for husako

## Context

husako has no CI/CD. The user wants to author GitHub Actions workflows using [gaji](https://gaji.gaebalgom.work) (type-safe GitHub Actions in TypeScript). Releases automated with release-plz. Binary distribution via npm. Crate publishing via crates.io.

## Architecture

```
Push to master → [check] quality gates
                → [version] release-plz creates release PR
Release PR merge → [version] release-plz bumps versions, creates tag + GitHub release, publishes crates
Tag v* created → [distribute] builds cross-platform binaries, uploads to GitHub release, publishes npm
Weekly → [audit] cargo audit
workflows/** changed → [sync-workflows] regenerate YAML
```

**release-plz** handles: changelog, version bumps, git tags, GitHub releases, crates.io publishing (`husako` CLI crate).
**distribute** handles: cross-platform binary builds, GitHub release assets, npm publishing.

## File Structure

```
husako/
├── gaji.config.ts
├── release-plz.toml
├── workflows/
│   ├── check.ts
│   ├── version.ts
│   ├── distribute.ts
│   ├── audit.ts
│   └── sync-workflows.ts
├── npm/
│   ├── husako/
│   │   ├── package.json
│   │   ├── bin/husako.js
│   │   └── scripts/postinstall.js
│   ├── platform-linux-x64/package.json
│   ├── platform-linux-arm64/package.json
│   ├── platform-darwin-x64/package.json
│   ├── platform-darwin-arm64/package.json
│   └── platform-win32-x64/package.json
├── scripts/
│   └── sync-versions.sh
├── generated/                    # gitignored
└── .github/workflows/            # gaji output (committed)
```

## Task 1: Initialize gaji

```bash
gaji init
gaji add actions/checkout@v5
gaji add dtolnay/rust-toolchain@stable
gaji add Swatinem/rust-cache@v2
gaji add release-plz/action@v0.5
gaji add actions/upload-artifact@v4
gaji add actions/download-artifact@v4
gaji add softprops/action-gh-release@v2
gaji add actions/setup-node@v4
```

`gaji.config.ts` — use defaults.

## Task 2: `release-plz.toml`

Configured from release-plz docs (https://release-plz.dev/docs/config):

```toml
[workspace]
# All crates publish to crates.io (required for `cargo install husako`)
publish = true
publish_allow_dirty = true
semver_check = true
# Library crates don't get individual tags or GitHub releases
git_release_enable = false
git_tag_enable = false

# Only the CLI crate creates tags + GitHub releases
[[package]]
name = "husako"
git_release_enable = true
git_tag_enable = true
git_tag_name = "v{{ version }}"
git_release_name = "husako v{{ version }}"
```

Key design:
- All 10 crates publish to crates.io (necessary for `cargo install husako` — deps must exist on registry)
- Only `husako` gets git tags + GitHub releases + changelog
- Library crates are published silently as implementation details
- All crates share `version.workspace = true`, bumped together

**Prerequisite**: Add `version.workspace = true` to all path dependency declarations for crates.io compatibility:
```toml
husako-core = { path = "../husako-core", version.workspace = true }
```

## Task 3: `workflows/check.ts` — Quality Gates

**Trigger**: PRs to `master` + push to `master`

**Jobs:**
- **lint**: checkout → rust-toolchain (clippy, rustfmt) → rust-cache → fmt check → clippy
- **test**: checkout → rust-toolchain → rust-cache → `cargo test --workspace --all-features`

Workspace-aware flags: `--workspace`, `--all-targets`, `--all-features`.

## Task 4: `workflows/version.ts` — Release Automation

**Trigger**: push to `master`

Based on release-plz GitHub Action quickstart (https://release-plz.dev/docs/github/quickstart):

**Jobs:**
- **release**: `release-plz/action@v0.5` with `command: release`
  - Permissions: `contents: write`, `id-token: write` (OIDC for all 10 crates on crates.io)
  - When release PR merges: publishes all crates, creates tag + GitHub release for `husako` only
  - `GITHUB_TOKEN: ${{ secrets.PAT }}`
- **release-pr**: `release-plz/action@v0.5` with `command: release-pr`
  - Permissions: `contents: write`, `pull-requests: write`
  - Creates/updates PR with version bumps + changelog
  - `GITHUB_TOKEN: ${{ secrets.PAT }}`

Concurrency: `group: release-plz-${{ github.ref }}`, `cancel-in-progress: false`

Both jobs: `fetch-depth: 0` (full history for changelog), `persist-credentials: false`.

## Task 5: `workflows/distribute.ts` — Binary Distribution

**Trigger**: `v*` tags (created by release-plz)

### Job: `build` (matrix: 5 targets)

| Platform | Runner | Rust Target |
|----------|--------|-------------|
| linux-x64 | ubuntu-latest | x86_64-unknown-linux-gnu |
| linux-arm64 | ubuntu-latest | aarch64-unknown-linux-gnu |
| darwin-x64 | macos-latest | x86_64-apple-darwin |
| darwin-arm64 | macos-latest | aarch64-apple-darwin |
| win32-x64 | windows-latest | x86_64-pc-windows-msvc |

Steps:
1. Checkout
2. Rust toolchain with target
3. (aarch64-linux) `sudo apt-get install -y gcc-aarch64-linux-gnu`, env: `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc`, `CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc`
4. `cargo build --release --target ${{ matrix.target.rust_target }}`
5. Archive: tar.gz (unix) / zip (windows)
6. Upload binary + npm artifacts

### Job: `github-release` (needs: build)

Download artifacts → SHA256 checksums → `softprops/action-gh-release` (appends to existing release created by release-plz).

### Job: `publish-npm` (needs: build)

Download npm artifacts → `scripts/sync-versions.sh` → publish 5 platform packages + main package.
`--provenance --access public`. Requires `id-token: write`.

## Task 6: `workflows/audit.ts` — Security Audit

**Trigger**: weekly (Monday 0:00 UTC)

checkout → `cargo install cargo-audit` → `cargo audit`

## Task 7: `workflows/sync-workflows.ts` — Workflow Regeneration

**Trigger**: push to `master` with changes in `workflows/**`

checkout (PAT for push) → `npm install -g gaji` → `gaji dev` → `gaji build` → commit + push if changed.

## Task 8: npm Package Structure

**`npm/husako/package.json`**:
```json
{
  "name": "husako",
  "version": "0.1.0",
  "description": "Type-safe Kubernetes resource authoring in TypeScript",
  "license": "MIT",
  "repository": { "type": "git", "url": "https://github.com/nanazt/husako" },
  "bin": { "husako": "bin/husako.js" },
  "scripts": { "postinstall": "node scripts/postinstall.js" },
  "optionalDependencies": {
    "@husako/linux-x64": "0.1.0",
    "@husako/linux-arm64": "0.1.0",
    "@husako/darwin-x64": "0.1.0",
    "@husako/darwin-arm64": "0.1.0",
    "@husako/win32-x64": "0.1.0"
  }
}
```

**`npm/husako/bin/husako.js`**: Platform detection → resolve `@husako/{platform}` binary → spawn with forwarded args.

**`npm/husako/scripts/postinstall.js`**: Validate platform binary availability.

**`npm/platform-*/package.json`**: Each with `name`, `version`, `os`, `cpu`, `files: ["bin/"]`.

## Task 9: `scripts/sync-versions.sh`

Reads version from `GITHUB_REF` tag or workspace `Cargo.toml`. Updates all npm `package.json` versions. Copies `README.md`.

## Task 10: Build + verify

```bash
gaji dev && gaji build
ls .github/workflows/        # 5 yml files
cargo test --workspace --all-features
```

## Task 11: GitHub Repository Settings

Configure via `gh` CLI after workflows are committed.

### Branch protection for `master`

```bash
gh api repos/nanazt/husako/branches/master/protection -X PUT --input - <<'EOF'
{
  "required_status_checks": {
    "strict": true,
    "contexts": ["lint", "test"]
  },
  "enforce_admins": false,
  "required_pull_request_reviews": null,
  "restrictions": null
}
EOF
```

- **Required checks**: `lint` and `test` (from `check.yml`) must pass before merge
- **Require up-to-date**: Branch must be current with `master` (`strict: true`)
- **No required reviews**: Solo development, can self-merge

### Repository merge settings

```bash
gh repo edit nanazt/husako \
  --enable-squash-merge \
  --disable-merge-commit \
  --disable-rebase-merge \
  --enable-auto-merge
```

- **Squash merge only**: Keeps `master` history clean and linear
- **Auto-merge enabled**: Release PRs (from release-plz) can auto-merge when checks pass

### GitHub Actions permissions

```bash
gh api repos/nanazt/husako/actions/permissions/workflow -X PUT \
  -f default_workflow_permissions=write \
  -F can_approve_pull_request_reviews=true
```

## Task 12: GitHub PAT → repo secret

release-plz needs a PAT (fine-grained) because the default `GITHUB_TOKEN` can't trigger other workflows (tag creation won't trigger distribute).

1. Ask user to create a fine-grained PAT at https://github.com/settings/tokens?type=beta
   - Token name: `husako-release-plz`
   - Repository: `nanazt/husako` only
   - Permissions: Contents (R/W), Pull requests (R/W), Metadata (read)
2. Run `gh secret set PAT --repo nanazt/husako` with the provided token

## Task 13: Initial crates.io publish

User provides a crates.io API token. Publish all 10 crates in dependency order (one-time, so OIDC can be configured).

Prerequisites:
- Add `version.workspace = true` to all path dependency declarations in each crate's `Cargo.toml`

```bash
# Dependency order (leaf → root)
CARGO_REGISTRY_TOKEN=<token> cargo publish -p husako-sdk
CARGO_REGISTRY_TOKEN=<token> cargo publish -p husako-yaml
CARGO_REGISTRY_TOKEN=<token> cargo publish -p husako-config
CARGO_REGISTRY_TOKEN=<token> cargo publish -p husako-compile-oxc
CARGO_REGISTRY_TOKEN=<token> cargo publish -p husako-openapi
CARGO_REGISTRY_TOKEN=<token> cargo publish -p husako-helm
CARGO_REGISTRY_TOKEN=<token> cargo publish -p husako-dts
CARGO_REGISTRY_TOKEN=<token> cargo publish -p husako-runtime-qjs
CARGO_REGISTRY_TOKEN=<token> cargo publish -p husako-core
CARGO_REGISTRY_TOKEN=<token> cargo publish -p husako
```

After publish: configure OIDC trusted publishing for all 10 crates at `https://crates.io/crates/{name}/settings` (owner: `nanazt`, repo: `husako`, workflow: `version.yml`). API token can then be revoked.

## Task 14: Documentation

### README.md — Installation section

Add installation methods:
```
cargo install husako
npm install -g husako
# or download from GitHub Releases
```

### CONTRIBUTING.md — Development workflow

Document the CI/CD-driven workflow:

1. **Development**: Create a branch → push → PR to `master`
2. **Quality gates**: `lint` (fmt + clippy) and `test` run automatically on PRs
3. **Merge**: Squash merge only, branch must be up-to-date with `master`
4. **Release PR**: release-plz automatically creates/updates a release PR with version bumps + changelog
5. **Release**: Merging the release PR triggers: crates.io publish → git tag → GitHub release
6. **Distribution**: Tag creation triggers: cross-platform builds → GitHub release assets → npm publish

### CLAUDE.md — CI/CD section

Add a brief section documenting the workflow files and their purpose, so future sessions have context.

## Manual Prerequisites

### npm Organization + Token

husako uses scoped packages (`@husako/*`) for platform-specific binaries.

1. Create org at https://www.npmjs.com/org/create → org name: `husako`
2. Generate a Granular Access Token (Packages: R/W, Organizations: `@husako`)
3. `gh secret set NPM_TOKEN --repo nanazt/husako` (I'll run this if you provide the token)
