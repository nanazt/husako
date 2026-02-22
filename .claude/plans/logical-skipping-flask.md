# Plan: `husako.toml` Configuration File

## Context

Currently husako has no project-level config file. Schema sources are specified via CLI flags on every `husako init` invocation (`--api-server`, `--spec-dir`), and `husako render` requires the full file path to an entry file. This makes workflows verbose and non-reproducible.

This plan adds `husako.toml` — a project config file (like Cargo.toml) that:
1. **Entry aliases**: `husako render dev-1` instead of `husako render env/dev-1.ts`
2. **Schema dependencies**: Declarative schema sources with version pinning, eliminating repeated CLI flags

## Design Decisions

- `husako.toml` is created by `husako new` only (not `husako init`)
- Entry aliases are explicit mappings (not pattern-based)
- **Unified source model**: All schema entries require explicit `source` key (no magic defaults)
- 4 source types: `release`, `cluster`, `git`, `file`
- **Cluster config**: `[cluster]` for single, `[clusters.*]` for multiple — separated from `[schemas]`
- **Kubeconfig auto-detection**: Credentials from `~/.kube/` files (no subdirectories), matched by server URL
- CRD sources parse Kubernetes CRD YAML manifests (`spec.versions[].schema.openAPIV3Schema`)
- `husako.toml` is the source of truth for `husako init`; CLI flags become overrides
- **No lock file**: Pinned sources (release+version, git+tag) are already deterministic. Mutable sources (cluster) are intentionally mutable.

## TOML Format

```toml
[entries]
dev-1 = "env/dev-1.ts"
staging = "env/staging.ts"

# Single cluster (optional)
[cluster]
server = "https://10.0.0.1:6443"

# OR multiple named clusters
# [clusters.dev]
# server = "https://10.0.0.1:6443"
# [clusters.prod]
# server = "https://prod.k8s.example.com:6443"

[schemas]
kubernetes = { source = "release", version = "1.35" }
cluster-crds = { source = "cluster" }                     # uses [cluster] above
# dev-crds = { source = "cluster", cluster = "dev" }      # uses [clusters.dev]
cert-manager = { source = "git", repo = "https://github.com/cert-manager/cert-manager", tag = "v1.17.2", path = "deploy/crds" }
my-crd = { source = "file", path = "./crds/my-crd.yaml" }
```

## Source Types

| Source | Description | Required Fields | Schema Format |
|--------|-------------|-----------------|---------------|
| `release` | K8s GitHub releases (kubernetes/kubernetes) | `version` | OpenAPI v3 JSON |
| `cluster` | Live K8s API server `/openapi/v3` | none (uses `[cluster]`) or `cluster` name | OpenAPI v3 JSON |
| `git` | Clone git repo, extract CRDs | `repo`, `tag`, `path` | CRD YAML |
| `file` | Local CRD files | `path` | CRD YAML |

## Cluster Credential Resolution

1. Scan all files directly in `~/.kube/` (no subdirectory traversal)
2. Parse each as kubeconfig YAML (silently skip non-kubeconfig files)
3. Find `clusters[].cluster.server` matching the configured server URL
4. Resolve the context referencing that cluster → find the user entry
5. Extract bearer token or client certificate credentials
6. If no match found → error with clear message

## Milestones

### M13a: `husako-config` crate + entry aliases

Config parsing + `husako render` alias support + `husako.toml` in templates.

### M13b: CRD YAML parsing + file/cluster sources

`husako init` reads `[schemas]`. `source = "cluster"` with kubeconfig auto-detection. `source = "file"` with CRD YAML parser.

### M13c: K8s GitHub releases + git source

`source = "release"` downloads from GitHub. `source = "git"` clones + extracts CRDs.

---

## M13a Implementation Plan

### 1. New crate: `husako-config`

**Create**: `crates/husako-config/Cargo.toml`, `crates/husako-config/src/lib.rs`

```rust
#[derive(Debug, Clone, Deserialize, Default)]
pub struct HusakoConfig {
    #[serde(default)]
    pub entries: HashMap<String, String>,
    #[serde(default)]
    pub cluster: Option<ClusterConfig>,
    #[serde(default)]
    pub clusters: HashMap<String, ClusterConfig>,
    #[serde(default)]
    pub schemas: HashMap<String, SchemaSource>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClusterConfig {
    pub server: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "source")]
pub enum SchemaSource {
    #[serde(rename = "release")]
    Release { version: String },
    #[serde(rename = "cluster")]
    Cluster {
        #[serde(default)]
        cluster: Option<String>,  // references [clusters.*] name
    },
    #[serde(rename = "git")]
    Git { repo: String, tag: String, path: String },
    #[serde(rename = "file")]
    File { path: String },
}
```

`load(project_root: &Path) -> Result<Option<HusakoConfig>, ConfigError>`:
- Returns `None` if `husako.toml` doesn't exist
- Returns `Err` if file exists but can't be parsed
- Validates: no absolute paths in entries, cluster references resolve, no both `[cluster]` and `[clusters]`

Error type: `ConfigError` with `Io`, `Parse`, `Validation` variants (thiserror).

### 2. Entry alias resolution in CLI

**Modify**: `crates/husako-cli/src/main.rs`

- Change `Render.file` from `PathBuf` to `String`
- Load config via `husako_config::load(&project_root)`
- Resolution: try as direct file path first → alias from config → error with available aliases
- Exit code `2` for bad alias

```
husako render env/dev-1.ts   → direct path (works as before)
husako render dev-1          → looks up [entries] "dev-1" → "env/dev-1.ts"
husako render unknown        → error with list of available aliases
```

### 3. Config error variant in core

**Modify**: `crates/husako-core/src/lib.rs`

- Add `HusakoError::Config(#[from] husako_config::ConfigError)` variant
- Exit code `2`

### 4. Templates include `husako.toml`

**Create** template TOML files in `crates/husako-sdk/src/templates/`:

`simple/husako.toml`:
```toml
[schemas]
kubernetes = { source = "release", version = "1.35" }
```

`project/husako.toml`:
```toml
[entries]
dev = "env/dev.ts"

[schemas]
kubernetes = { source = "release", version = "1.35" }
```

`multi-env/husako.toml`:
```toml
[entries]
dev = "dev/main.ts"
staging = "staging/main.ts"
release = "release/main.ts"

[schemas]
kubernetes = { source = "release", version = "1.35" }
```

**Modify**: `crates/husako-sdk/src/lib.rs` — add `TEMPLATE_*_CONFIG` constants
**Modify**: `crates/husako-core/src/lib.rs` `scaffold()` — write `husako.toml`
**Modify**: CLI "Next steps" — `husako init --skip-k8s` → `husako init`

### 5. Workspace changes

**Modify**: `Cargo.toml` — add `toml = "0.8"` to workspace deps, `husako-config` to members
**Modify**: `crates/husako-cli/Cargo.toml` — add `husako-config` dep

### Files to create
- `crates/husako-config/Cargo.toml`
- `crates/husako-config/src/lib.rs`
- `crates/husako-sdk/src/templates/simple/husako.toml`
- `crates/husako-sdk/src/templates/project/husako.toml`
- `crates/husako-sdk/src/templates/multi-env/husako.toml`

### Files to modify
- `Cargo.toml` (workspace deps + members)
- `crates/husako-cli/Cargo.toml` (add husako-config dep)
- `crates/husako-cli/src/main.rs` (load config, resolve aliases, update "Next steps")
- `crates/husako-core/Cargo.toml` (add husako-config dep)
- `crates/husako-core/src/lib.rs` (Config error variant, scaffold writes toml)
- `crates/husako-sdk/src/lib.rs` (template constants)

### Tests (~10)
- Config: parse valid TOML with all source types, empty file, missing file → None, invalid TOML → error, absolute path rejected
- Config: cluster reference validation (unknown cluster name, both [cluster] and [clusters])
- Alias: resolves correctly, file-not-found error, unknown alias lists alternatives
- Scaffold: each template creates husako.toml
- Backward compat: render still works with direct file path (no config)

## Verification

```bash
cargo build
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check

# Manual E2E
cargo run -- new /tmp/test-app --template project
cat /tmp/test-app/husako.toml
```
