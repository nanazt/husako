# ChartSource::Oci — Direct OCI Registry Support

## Context

`husako generate` supports OCI registry charts only indirectly (when an HTTP registry's
`index.yaml` lists `oci://` archive URLs, or when ArtifactHub points to OCI). There is no
way to declare a direct OCI chart source in `husako.toml` or add one via `husako add`.

This adds `ChartSource::Oci { reference, version }` so users can declare:
```toml
[charts]
postgresql = { source = "oci", reference = "oci://ghcr.io/org/postgresql", version = "1.2.3" }
```

And use `husako add` interactively with OCI tag discovery.

## Design Decisions

- **Fields:** `reference` (full `oci://` URL) + `version` — chart name is derived from the
  last path component of the reference (e.g. `oci://ghcr.io/org/postgresql` → `postgresql`).
  No explicit `chart` field; users who need an override edit `husako.toml` directly.
- **Tag discovery:** `list_tags()` added to `husako-helm/src/oci.rs` (auth + `GET /v2/{repo}/tags/list`).
  Exposed as `pub` and called from `husako-core/src/version_check.rs`.
- **Version format:** Tags stored as returned by registry (e.g. `"1.2.3"` or `"v1.2.3"`).

## Files to Modify

| File | Change |
|------|--------|
| `crates/husako-config/src/lib.rs` | Add `Oci { reference: String, version: String }` variant to `ChartSource` |
| `crates/husako-config/src/edit.rs` | Add `Oci` arm in `chart_source_to_inline_table()` |
| `crates/husako-helm/Cargo.toml` | Add `semver.workspace = true` |
| `crates/husako-helm/src/oci.rs` | Add `pub fn list_tags()` and `pub(crate) fn chart_name_from_reference()` |
| `crates/husako-helm/src/lib.rs` | Add `ChartSource::Oci` dispatch; derive chart name from reference |
| `crates/husako-core/src/version_check.rs` | Add `discover_oci_tags()` and `discover_latest_oci()` |
| `crates/husako-core/src/lib.rs` | Add `Oci` arms in `chart_info()`, outdated match (~L935), update match (~L995) |
| `crates/husako-cli/src/main.rs` | Add `--reference` flag to `Add` command; add `"oci"` arm in `build_chart_target()` |
| `crates/husako-cli/src/interactive.rs` | Add `"oci"` before `"git"` in source list; add `prompt_oci_chart()` |
| `scripts/e2e.sh` | Add Scenario C: OCI chart source |
| `.worktrees/docs-site/docs/guide/helm.md` | Add `### oci` section |
| `.worktrees/docs-site/docs/guide/configuration.md` | Add `oci` to chart source table |
| `CLAUDE.md` | Update "4 types" → "5 types", add `oci` to chart source list |

## Implementation

### Step 1 — `husako-config/src/lib.rs`

Add after `Git` variant in `ChartSource`:
```rust
/// Fetch directly from an OCI registry.
/// `postgresql = { source = "oci", reference = "oci://ghcr.io/org/postgresql", version = "1.2.3" }`
#[serde(rename = "oci")]
Oci {
    reference: String,
    version: String,
},
```

Add unit test to `parse_charts_section` group:
```rust
#[test]
fn parse_oci_chart_source() {
    let toml = r#"[charts]
postgresql = { source = "oci", reference = "oci://ghcr.io/org/postgresql", version = "1.2.3" }
"#;
    let config: HusakoConfig = toml::from_str(toml).unwrap();
    assert!(matches!(
        config.charts["postgresql"],
        ChartSource::Oci { ref reference, ref version }
        if reference == "oci://ghcr.io/org/postgresql" && version == "1.2.3"
    ));
}
```

### Step 2 — `husako-config/src/edit.rs`

In `chart_source_to_inline_table()`, add arm:
```rust
ChartSource::Oci { reference, version } => {
    t.insert("source", "oci".into());
    t.insert("reference", reference.as_str().into());
    t.insert("version", version.as_str().into());
}
```

### Step 3 — `husako-helm/src/oci.rs`

Add after `resolve()`:

```rust
/// Helper: extract chart name from OCI reference (last path component, before any tag suffix).
/// `oci://ghcr.io/org/postgresql` → `"postgresql"`
/// `oci://ghcr.io/org/postgresql:1.2.3` → `"postgresql"`
pub(crate) fn chart_name_from_reference(reference: &str) -> &str {
    let without_scheme = reference.strip_prefix("oci://").unwrap_or(reference);
    let path_part = without_scheme.split('/').last().unwrap_or(without_scheme);
    path_part.split(':').next().unwrap_or(path_part)
}

/// Fetch available tags from an OCI registry for the given reference.
/// Uses the same anonymous bearer token auth flow as `resolve()`.
/// Returns stable semver tags sorted descending, up to `limit` starting at `offset`.
pub fn list_tags(
    reference: &str,
    limit: usize,
    offset: usize,
) -> Result<Vec<String>, HelmError> {
    let (host, repo, _) = parse_oci_reference(reference)?;
    let token = get_token(&host, &repo).ok();  // anonymous; ignore auth errors

    let url = format!("https://{host}/v2/{repo}/tags/list?n=200");
    let mut req = ureq::get(&url);
    if let Some(token) = &token {
        req = req.set("Authorization", &format!("Bearer {token}"));
    }

    let body: serde_json::Value = req.call()
        .map_err(|e| HelmError::Http(e.to_string()))?
        .into_json()
        .map_err(|e| HelmError::Http(e.to_string()))?;

    let mut tags: Vec<String> = body["tags"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .filter(|tag| {
            // Accept stable semver (with or without leading 'v')
            let stripped = tag.strip_prefix('v').unwrap_or(tag);
            semver::Version::parse(stripped)
                .map(|v| v.pre.is_empty())
                .unwrap_or(false)
        })
        .collect();

    tags.sort_by(|a, b| {
        let va = semver::Version::parse(a.strip_prefix('v').unwrap_or(a)).ok();
        let vb = semver::Version::parse(b.strip_prefix('v').unwrap_or(b)).ok();
        vb.cmp(&va)
    });

    Ok(tags.into_iter().skip(offset).take(limit).collect())
}
```

Add unit tests:
```rust
#[test]
fn chart_name_from_reference_basic() {
    assert_eq!(chart_name_from_reference("oci://ghcr.io/org/postgresql"), "postgresql");
}
#[test]
fn chart_name_from_reference_with_tag() {
    assert_eq!(chart_name_from_reference("oci://ghcr.io/org/postgresql:1.2.3"), "postgresql");
}
```

Note: `parse_oci_reference` and `get_token` are already in `oci.rs` — reuse directly.
Use `reqwest::blocking::Client` (same as the rest of `oci.rs`).
Add `semver.workspace = true` to `crates/husako-helm/Cargo.toml` (it's in workspace deps but not yet used in husako-helm).

The `list_tags()` function uses the same `reqwest::blocking::Client` pattern as `resolve()`:
```rust
pub fn list_tags(reference: &str, limit: usize, offset: usize) -> Result<Vec<String>, HelmError> {
    let (host, repo, _) = parse_oci_reference(reference)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| HelmError::Http(e.to_string()))?;

    let token = get_token(&client, &host, &repo).ok();
    let url = format!("https://{host}/v2/{repo}/tags/list?n=200");
    let mut builder = client.get(&url);
    if let Some(token) = &token {
        builder = builder.bearer_auth(token);
    }
    let body: serde_json::Value = builder.send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| HelmError::Http(e.to_string()))?
        .json()
        .map_err(|e| HelmError::Http(e.to_string()))?;
    // ... filter + sort semver tags ...
}
```
(Exact signatures of `get_token` and `parse_oci_reference` must be verified against current oci.rs before implementation.)

### Step 4 — `husako-helm/src/lib.rs`

In `resolve()`, add `ChartSource::Oci` arm:
```rust
ChartSource::Oci { reference, version } => {
    let chart = crate::oci::chart_name_from_reference(reference);
    crate::oci::resolve(name, reference, chart, version, cache_dir)
}
```

### Step 5 — `husako-core/src/version_check.rs`

Add two functions:
```rust
/// Fetch up to `limit` available OCI tags for `reference`, starting at `offset`.
pub fn discover_oci_tags(
    reference: &str,
    limit: usize,
    offset: usize,
) -> Result<Vec<String>, HusakoError> {
    husako_helm::oci::list_tags(reference, limit, offset)
        .map_err(|e| HusakoError::Helm(e.to_string()))
}

/// Return the latest stable OCI tag for `reference`, or None if unavailable.
pub fn discover_latest_oci(reference: &str) -> Result<Option<String>, HusakoError> {
    let tags = discover_oci_tags(reference, 1, 0)?;
    Ok(tags.into_iter().next())
}
```

### Step 6 — `husako-core/src/lib.rs`

**`chart_info()` (~L743)** — add `Oci` arm:
```rust
husako_config::ChartSource::Oci { reference, version } => DependencyInfo {
    name: name.to_string(),
    source_type: "oci".to_string(),
    identifier: reference.clone(),
    current_version: Some(version.clone()),
    latest_version: None,
},
```

**Outdated check match (~L935)** — add `Oci` arm:
```rust
husako_config::ChartSource::Oci { reference, version } => {
    match version_check::discover_latest_oci(reference) {
        Ok(Some(latest)) => { /* compare and record */ }
        Ok(None) => { /* no tags found, skip */ }
        Err(e) => { /* record error */ }
    }
}
```

**Update match (~L995)** — add `Oci` arm:
```rust
husako_config::ChartSource::Oci { ref version, reference } => {
    match version_check::discover_latest_oci(reference) {
        Ok(Some(latest)) => { /* update if newer */ }
        _ => {}
    }
}
```

### Step 7 — `husako-cli/src/interactive.rs`

**Source list** — add `"oci"` before `"git"`:
```rust
.items(["artifacthub", "registry", "oci", "git", "file"])
```

**Dispatch** — shift git/file indices and add oci arm:
```rust
2 => prompt_oci_chart()?,
3 => prompt_git_chart()?,   // was 2
4 => prompt_file_chart()?,  // was 3
```

**New function `prompt_oci_chart()`:**
```rust
fn prompt_oci_chart() -> Result<AddTarget, String> {
    let reference = text_input::run(
        "OCI reference",
        Some("oci://ghcr.io/org/chart-name"),
        validate_oci_reference,
    )?;

    let default_name = husako_helm::oci::chart_name_from_reference(&reference).to_string();
    let limit = 20;

    let fetch = |offset: usize| {
        husako_core::version_check::discover_oci_tags(&reference, limit, offset)
            .map_err(|e| e.to_string())
    };

    let (name, version) = name_version_select::run(&default_name, limit, fetch)
        .map_err(|e| e.to_string())?;

    Ok(AddTarget::Chart {
        name,
        source: ChartSource::Oci { reference, version },
    })
}
```

**Validation helper** (add alongside existing validators):
```rust
fn validate_oci_reference(s: &str) -> Result<(), String> {
    if s.starts_with("oci://") && s.len() > 6 {
        Ok(())
    } else {
        Err("Must start with oci://".to_string())
    }
}
```

Note: `husako_helm::oci::chart_name_from_reference` needs to be accessible from CLI crate.
Check `husako-cli/Cargo.toml` — if it depends on `husako-helm` already, use directly; otherwise
route through `husako-core` or expose from `husako-config`.

Actually `husako-cli` depends on `husako-core` and `husako-config`, not directly on `husako-helm`.
So expose `chart_name_from_reference` as a helper in `husako-core` or just duplicate the logic
inline in `prompt_oci_chart()` (it's a one-liner).

**Simpler approach:** inline the derivation in `prompt_oci_chart()`:
```rust
let default_name = reference
    .trim_end_matches('/')
    .rsplit('/')
    .next()
    .unwrap_or("chart")
    .split(':')
    .next()
    .unwrap_or("chart")
    .to_string();
```

### Step 8 — `husako-cli/src/main.rs` (non-interactive flags)

**Add `--reference` arg** to `Add` command (after `--package`):
```rust
/// OCI reference (e.g. oci://ghcr.io/org/chart-name)
#[arg(long)]
reference: Option<String>,
```

Update the `source` help text: `"Source type (release, cluster, git, file, registry, artifacthub, oci)"`.

**Add `"oci"` arm in `build_chart_target()`**:
```rust
"oci" => {
    let reference = reference.ok_or("--reference is required for oci source")?;
    let version = version.ok_or("--version is required for oci source")?;
    husako_config::ChartSource::Oci { reference, version }
}
```

Pass `reference` through the existing call to `build_chart_target()` at the call site (~L585).

### Step 9 — `scripts/e2e.sh`

Add Scenario C after Scenario B. Use a fixed known version of a stable OCI chart:
```bash
# ── Scenario C: OCI chart source ────────────────────────────────────────────
run_scenario_c() {
  local dir; dir=$(mktemp -d)
  echo "── C: OCI chart source ──"
  cd "$dir"
  "$HUSAKO" new . --name test-oci

  "$HUSAKO" -y add postgresql --chart --source oci \
    --reference "oci://registry-1.docker.io/bitnamicharts/postgresql" \
    --version "16.4.0"

  "$HUSAKO" generate

  assert_file ".husako/types/helm/postgresql.d.ts"
  assert_file ".husako/types/helm/postgresql.js"
  assert_dts_exports ".husako/types/helm/postgresql.d.ts" "PostgresqlValues"

  pass "Scenario C: OCI chart"
  cd - > /dev/null
  rm -rf "$dir"
}
```

### Step 10 — Docs

**`docs/guide/helm.md`** — add `### oci` section after the `### registry` section:
```markdown
### oci

Fetches `values.schema.json` directly from an OCI registry using the
OCI Distribution API. Use this when you have a full `oci://` reference to the chart:

```toml
[charts]
postgresql = { source = "oci", reference = "oci://registry-1.docker.io/bitnamicharts/postgresql", version = "16.4.0" }
grafana    = { source = "oci", reference = "oci://ghcr.io/grafana/grafana", version = "8.8.2" }
```

The chart name for type generation is derived from the last path component of the
reference. Public registries (Docker Hub, GHCR) work with anonymous access. Private
registries requiring credentials are not yet supported.
```

**`docs/guide/configuration.md`** — add `oci` row to the chart sources table and `### oci` sub-section parallel to the existing `### registry`, `### artifacthub`, etc. sections.

### Step 11 — `CLAUDE.md`

Update line ~189:
```
- **Chart dependencies**: `[charts]` declares Helm chart sources with 5 types: `registry`, `artifacthub`, `git`, `file`, `oci`
```

## Verification

```bash
# Unit tests
cargo test -p husako-config parse_oci_chart_source
cargo test -p husako-helm chart_name_from_reference
cargo test --workspace --all-features

# Clippy
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Manual: husako add → oci → enter reference → pick version
cargo build --bin husako
echo '{}' > /tmp/test-husako.toml  # or use husako new in a temp dir
./target/debug/husako add  # select "oci", enter oci://ghcr.io/grafana/grafana

# Manual: husako generate with OCI source
# husako.toml: [charts] grafana = { source = "oci", reference = "oci://...", version = "..." }
bash scripts/e2e.sh  # if OCI chart added to e2e
```

## Branch

Create `feat/chart-source-oci` from `master`.
