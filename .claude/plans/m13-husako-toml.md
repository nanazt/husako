# Plan: `husako.toml` Configuration File (M13)

## Context

husako had no project-level config file. Schema sources were specified via CLI flags on every `husako init` invocation (`--api-server`, `--spec-dir`), and `husako render` required the full file path to an entry file. This made workflows verbose and non-reproducible.

This plan adds `husako.toml` — a project config file (like Cargo.toml) that provides:
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

## Schema Source Resolution Architecture

```
husako.toml [schemas]
    ↓
schema_source::resolve_all(config)
    ↓
┌──────────────┬──────────────┬───────────────┬──────────────┐
│ file         │ cluster      │ release       │ git          │
│ read CRD YAML│ kubeconfig   │ GitHub API    │ git clone    │
│ → crd.rs     │ → OpenApi    │ → download    │ → crd.rs     │
│   convert    │   Client     │   spec JSON   │   convert    │
└──────┬───────┴──────┬───────┴───────┬───────┴──────┬───────┘
       └──────────────┴───────────────┴──────────────┘
                              ↓
              HashMap<String, serde_json::Value>
                              ↓
                    husako_dts::generate()
```

**`init()` priority**:
1. `--skip-k8s` → skip all schema generation
2. CLI `--api-server`/`--spec-dir` → legacy mode (backward compat)
3. `husako.toml` `[schemas]` → config-driven mode via `schema_source::resolve_all()`
4. None of the above → skip k8s types

## Cluster Credential Resolution

1. Scan all files directly in `~/.kube/` (no subdirectory traversal)
2. Parse each as kubeconfig YAML (silently skip non-kubeconfig files)
3. Find `clusters[].cluster.server` matching the configured server URL
4. Resolve the context referencing that cluster → find the user entry
5. Extract bearer token (no client cert/exec for now)
6. If no match found → error with clear message

Production entry point `resolve_credentials(server_url)` uses `~/.kube/` as default dir; `resolve_credentials_from_dir(kube_dir, server_url)` is the testable variant.

---

## Implementation

### Config Data Model (`husako-config`)

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
#[serde(tag = "source")]
pub enum SchemaSource {
    #[serde(rename = "release")]
    Release { version: String },
    #[serde(rename = "cluster")]
    Cluster { #[serde(default)] cluster: Option<String> },
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

### Entry Alias Resolution (CLI)

- `Render.file`: `String` (not `PathBuf`)
- Resolution: try as direct file path first → alias from `[entries]` → error with available aliases
- Exit code `2` for bad alias

### CRD YAML → OpenAPI JSON (`husako-openapi/src/crd.rs`)

**Function**: `pub fn crd_yaml_to_openapi(yaml: &str) → Result<Value, OpenApiError>`

Algorithm:
1. Parse multi-document YAML stream with `serde_yaml_ng`
2. For each CRD document, extract `spec.group`, `spec.names.kind`, `spec.versions[]`
3. For each version, get `schema.openAPIV3Schema`
4. Recursively extract nested `object` properties with `properties` into separate named schemas, replacing with `$ref`
5. Build full names: `io.<reversed-group>.<version>.<Kind>` (e.g., `io.cert-manager.v1.Certificate`)
6. Add `x-kubernetes-group-version-kind` and standard `apiVersion`/`kind`/`metadata` properties
7. Return `{"components": {"schemas": {...}}}` — same format as existing fixtures

Schema naming:
- Top-level: `io.cert-manager.v1.Certificate`
- `spec` property: `io.cert-manager.v1.CertificateSpec`
- Nested objects: PascalCase from property name (e.g., `issuerRef` → `io.cert-manager.v1.CertificateIssuerRef`)

### Source Handlers

**File** (`schema_source::resolve_file`): Resolve path relative to project root. Single file or directory of `.yaml`/`.yml` files → `crd::crd_yaml_to_openapi()` → group by GVK into discovery-keyed specs.

**Cluster** (`schema_source::resolve_cluster`): Look up server URL from config → `kubeconfig::resolve_credentials()` → `OpenApiClient::new()` → `fetch_all_specs()`.

**Release** (`release::fetch_release_specs`): Version to tag (`"1.35"` → `v1.35.0`). GitHub Contents API to list `api/openapi-spec/v3/` files. Download each spec. Map filename to discovery key (`apis__apps__v1_openapi.json` → `apis/apps/v1`). Tag-based deterministic cache under `cache_dir/release/{tag}/`.

**Git** (`schema_source::resolve_git`): Check cache at `cache_dir/git/{repo_hash}/{tag}/`. If miss: `git clone --depth 1 --branch {tag}` → read CRD YAML → convert → cache converted OpenAPI JSON. Uses `std::process::Command` (no `git2` crate).

### Schema Resolver Orchestrator

**Function**: `resolve_all(config, project_root, cache_dir) → Result<HashMap<String, Value>>`

```rust
for (name, source) in &config.schemas {
    match source {
        SchemaSource::File { path } => resolve_file(path, project_root),
        SchemaSource::Cluster { cluster } => resolve_cluster(config, cluster.as_deref(), cache_dir),
        SchemaSource::Release { version } => resolve_release(version, cache_dir),
        SchemaSource::Git { repo, tag, path } => resolve_git(repo, tag, path, cache_dir),
    }
}
// Later sources override for same discovery key
```

### Error Variants

`OpenApiError`: `Crd(String)`, `Kubeconfig(String)`, `Release(String)`

`HusakoError`: `Config(#[from] husako_config::ConfigError)` (exit code 2)

### Templates

Each `husako new` template includes `husako.toml` with appropriate `[entries]` and `[schemas]` sections.

---

## Files

### Created
- `crates/husako-config/Cargo.toml` + `src/lib.rs` — config parser
- `crates/husako-openapi/src/crd.rs` — CRD YAML → OpenAPI JSON
- `crates/husako-openapi/src/kubeconfig.rs` — kubeconfig credential resolution
- `crates/husako-openapi/src/release.rs` — GitHub release spec download + cache
- `crates/husako-core/src/schema_source.rs` — source handler orchestrator
- `crates/husako-sdk/src/templates/{simple,project,multi-env}/husako.toml`

### Modified
- `Cargo.toml` — workspace deps (`toml`, `serde_yaml_ng`) + members
- `crates/husako-cli/Cargo.toml` — add `husako-config` dep
- `crates/husako-cli/src/main.rs` — load config, resolve aliases, pass to `InitOptions`
- `crates/husako-core/Cargo.toml` — add `husako-config` + `tempfile` deps
- `crates/husako-core/src/lib.rs` — `schema_source` module, `InitOptions.config`, config-driven `init()`
- `crates/husako-openapi/Cargo.toml` — add `serde_yaml_ng`
- `crates/husako-openapi/src/lib.rs` — `pub mod crd, kubeconfig, release` + error variants
- `crates/husako-sdk/src/lib.rs` — template config constants

---

## Tests

| Module | Tests | Count |
|--------|-------|-------|
| `husako-config` | parse all source types, empty/missing file, invalid TOML, absolute path rejected, cluster validation | ~15 |
| `crd.rs` | simple CRD, nested extraction, array items, GVK, multi-version, multi-doc, non-CRD skip, metadata ref | ~8 |
| `kubeconfig.rs` | resolve standard, bearer token, no match, skip non-yaml, multiple files, URL normalization | ~6 |
| `release.rs` | version mapping, filename conversion, filter, cache round-trip, cache hit | ~6 |
| `schema_source.rs` | file single, file dir, file not found, derive key, hash deterministic, git cache round-trip, CRD dir reading | ~7 |
| CLI integration | alias resolution, scaffold creates husako.toml, backward compat | ~9 |

Total: 270 tests (28 new over M13a's 242).

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
