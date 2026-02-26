# Plan: husako add --cluster also writes [clusters.*] to husako.toml

## Context

`husako add --cluster dev` currently reads the server URL from kubeconfig and
*displays* it, but never persists it. The user must manually add `[clusters.dev]`
to husako.toml, or `husako generate` will fail later.

This inconsistency is resolved by automatically writing `[cluster]` / `[clusters.*]`
to husako.toml when the URL is sourced from kubeconfig (= the section is not yet
present in husako.toml). If the section already exists, it is left unchanged
(non-destructive).

## Target UX

### URL from kubeconfig, not yet in husako.toml

```
  Cluster: dev  https://dev:6443
  → will add [clusters.dev] to husako.toml
warning: Adding a cluster resource will fetch ALL CRDs from the cluster, which may be a large set.
Continue? (y/n):

✔ Added [clusters.dev] to husako.toml
✔ Added dev-crds to [resources]
  cluster  dev
```

### URL already in husako.toml (existing behavior unchanged)

```
  Cluster: dev  https://dev:6443
warning: ...
Continue? (y/n):

✔ Added dev-crds to [resources]
  cluster  dev
```

- `→` hint line → `style::arrow_mark()` (cyan)
- Section name in success messages → `style::bold()`

---

## Changes

### 1. `crates/husako-config/src/edit.rs`

Add `pub fn add_cluster_config(doc, cluster_name, server)`:

```rust
/// Write a cluster connection entry to husako.toml.
///
/// `cluster_name = None`       → `[cluster]` section
/// `cluster_name = Some(name)` → `[clusters.name]` section
///
/// Does nothing if the section already exists (non-destructive).
pub fn add_cluster_config(
    doc: &mut DocumentMut,
    cluster_name: Option<&str>,
    server: &str,
) {
    match cluster_name {
        None => {
            if doc.get("cluster").is_none() {
                let mut t = Table::new();
                t.insert("server", value(server));
                doc["cluster"] = Item::Table(t);
            }
        }
        Some(name) => {
            // Ensure [clusters] outer table exists (implicit — no [clusters] header)
            if doc.get("clusters").is_none() {
                let mut outer = Table::new();
                outer.set_implicit(true);
                doc["clusters"] = Item::Table(outer);
            }
            // Add [clusters.name] subtable only if absent
            if doc["clusters"].as_table().map_or(true, |t| !t.contains_key(name)) {
                let mut inner = Table::new();
                inner.insert("server", value(server));
                doc["clusters"][name] = Item::Table(inner);
            }
        }
    }
}
```

`toml_edit::Table::set_implicit(true)` prevents a standalone `[clusters]` header,
yielding the `[clusters.dev]` format.

Add 3 unit tests to the existing `#[cfg(test)]` block:
- `add_cluster_config_single` — writes `[cluster]` section
- `add_cluster_config_named` — writes `[clusters.dev]` section
- `add_cluster_config_no_overwrite` — does not modify an already-present section

### 2. `crates/husako-cli/src/main.rs`

#### a) Add `ClusterConfigToAdd` struct and extend `AddResult::Resource`

```rust
struct ClusterConfigToAdd {
    cluster_name: Option<String>, // None = [cluster], Some("dev") = [clusters.dev]
    server: String,
}

enum AddResult {
    Resource {
        name: String,
        source: husako_config::SchemaSource,
        cluster_config: Option<ClusterConfigToAdd>, // ← new field
    },
    Chart { name: String, source: husako_config::ChartSource },
}
```

#### b) `resolve_add_target()` — cluster branch

Split the URL lookup into two stages to track the source:

```rust
// Stage 1: husako.toml
let config_server = husako_config::load(project_root)
    .ok().flatten()
    .and_then(|cfg| {
        if cluster_val.is_empty() { cfg.cluster.map(|c| c.server) }
        else { cfg.clusters.get(&cluster_val).map(|c| c.server.clone()) }
    });

// Stage 2: kubeconfig fallback (only if husako.toml had nothing)
let kube_server = if config_server.is_none() {
    let ctx = if cluster_val.is_empty() { None } else { Some(cluster_val.as_str()) };
    husako_openapi::kubeconfig::server_for_context(ctx)
} else {
    None
};

// Fail if neither source has the URL
let server_url = match config_server.as_deref().or(kube_server.as_deref()) {
    Some(url) => url.to_string(),
    None => return Err(/* not configured error, same as before */),
};

// cluster_config is Some only when URL came from kubeconfig
let cluster_config = kube_server.map(|s| ClusterConfigToAdd {
    cluster_name: if cluster_val.is_empty() { None } else { Some(cluster_val.clone()) },
    server: s,
});

// Print cluster identity + "will add" hint (both unconditional)
eprintln!("  Cluster: {}  {}", style::dep_name(display_name), style::dim(&server_url));
if cluster_config.is_some() {
    let section = if cluster_val.is_empty() { "[cluster]".to_string() }
                  else { format!("[clusters.{}]", cluster_val) };
    eprintln!("  {} will add {} to husako.toml", style::arrow_mark(), style::bold(&section));
}

// ... confirmation prompt (unchanged) ...

return Ok(Some(AddResult::Resource {
    name: dep_name,
    source: SchemaSource::Cluster { cluster: cluster_name },
    cluster_config,
}));
```

#### c) `Commands::Add` handler — write cluster config, then resource dep

```rust
Ok(Some(result)) => {
    // 1. Write [cluster] / [clusters.*] if sourced from kubeconfig
    if let AddResult::Resource { cluster_config: Some(ref cc), .. } = result {
        let (mut doc, doc_path) = match husako_config::edit::load_document(&project_root) {
            Ok(d) => d,
            Err(e) => { eprintln!("{} {e}", style::error_prefix()); return ExitCode::from(2u8); }
        };
        husako_config::edit::add_cluster_config(&mut doc, cc.cluster_name.as_deref(), &cc.server);
        if let Err(e) = husako_config::edit::save_document(&doc, &doc_path) {
            eprintln!("{} {e}", style::error_prefix());
            return ExitCode::from(2u8);
        }
        let section = match &cc.cluster_name {
            None => "[cluster]".to_string(),
            Some(n) => format!("[clusters.{}]", n),
        };
        eprintln!("{} Added {} to husako.toml", style::check_mark(), style::bold(&section));
    }

    // 2. Add resource dep (existing flow)
    let target = match &result { ... };
    match husako_core::add_dependency(&project_root, &target) { ... }
}
```

**All `AddResult::Resource` sites to update:**

| Location | Change |
|----------|--------|
| Line ~582: `{ name, source }` in target-build match | Add `..` → `{ name, source, .. }` |
| Line ~1365: Release variant construction | Add `cluster_config: None` |
| Lines ~1512, ~1549: Git Resource variants | Add `cluster_config: None` |
| Line ~1596: LocalPath Resource variant | Add `cluster_config: None` |
| Line ~1442: Cluster variant | Set `cluster_config` to computed value |

`format_source_detail()` already uses `..` on all arms — no change needed.

`load_document` returns `(DocumentMut, PathBuf)` — use the returned `PathBuf`
for `save_document` (do not reconstruct the path manually).

### 3. `crates/husako-cli/tests/integration.rs`

Existing 3 cluster tests remain valid — the two husako.toml-based tests
(`add_cluster_shows_server_url_from_config`, `add_cluster_named_shows_server_url`)
will NOT show the "will add" message since the URL comes from husako.toml.

Add kubeconfig fixture constants and 2 new tests:

```rust
const KUBECONFIG_WITH_CURRENT_CONTEXT: &str = r#"
apiVersion: v1
kind: Config
current-context: my-ctx
clusters:
  - name: my-cluster
    cluster:
      server: https://k8s.local:6443
contexts:
  - name: my-ctx
    context:
      cluster: my-cluster
      user: my-user
users:
  - name: my-user
    user:
      token: tok
"#;

const KUBECONFIG_WITH_DEV_CONTEXT: &str = r#"
apiVersion: v1
kind: Config
current-context: dev
clusters:
  - name: dev-cluster
    cluster:
      server: https://dev:6443
contexts:
  - name: dev
    context:
      cluster: dev-cluster
      user: dev-user
users:
  - name: dev-user
    user:
      token: tok
"#;

#[test]
fn add_cluster_writes_cluster_config_from_kubeconfig() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("husako.toml"), "[resources]\n").unwrap();
    let kube_dir = dir.path().join(".kube");
    std::fs::create_dir_all(&kube_dir).unwrap();
    std::fs::write(kube_dir.join("config"), KUBECONFIG_WITH_CURRENT_CONTEXT).unwrap();

    husako_at(root)
        .args(["add", "--cluster", "--yes"])
        .env("HOME", dir.path())
        .assert()
        .success()
        .stderr(predicates::str::contains("will add [cluster]"))
        .stderr(predicates::str::contains("Added [cluster] to husako.toml"));

    let content = std::fs::read_to_string(root.join("husako.toml")).unwrap();
    assert!(content.contains("[cluster]"));
    assert!(content.contains("https://k8s.local:6443"));
}

#[test]
fn add_cluster_named_writes_clusters_section_from_kubeconfig() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("husako.toml"), "[resources]\n").unwrap();
    let kube_dir = dir.path().join(".kube");
    std::fs::create_dir_all(&kube_dir).unwrap();
    std::fs::write(kube_dir.join("config"), KUBECONFIG_WITH_DEV_CONTEXT).unwrap();

    husako_at(root)
        .args(["add", "--cluster", "dev", "--yes"])
        .env("HOME", dir.path())
        .assert()
        .success()
        .stderr(predicates::str::contains("will add [clusters.dev]"))
        .stderr(predicates::str::contains("Added [clusters.dev] to husako.toml"));

    let content = std::fs::read_to_string(root.join("husako.toml")).unwrap();
    assert!(content.contains("https://dev:6443"));
}
```

### 4. `.worktrees/docs-site/docs/reference/cli.md`

Update the `--cluster` flag description:

```
| `--cluster [name]` | Add a live-cluster schema source; resolves server URL from `husako.toml` or kubeconfig, and writes `[cluster]`/`[clusters.*]` to `husako.toml` if not already present (fails if URL found nowhere) |
```

---

## Verification

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```
