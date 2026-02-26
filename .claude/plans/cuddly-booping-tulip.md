# Plan: Comprehensive CLI integration tests — all untested cases

## Context

A full audit of `crates/husako-cli/tests/integration.rs` (90 tests) found:
- **Zero coverage**: `outdated`, `update`, `test`, `info`, `debug`, `clean`
- **Partial coverage**: `list/ls`, `remove/rm`, `plugin add`, `gen` (lock-based, chart sources)
- **Missing flags**: `--dry-run`, `--yes`, `-o`, `--no-incremental`, `--resources-only`, `--charts-only`

The plan adds ~50 new tests, grouped into two tracks:
- **Track A** — no network needed, no production code changes
- **Track B** — HTTP network, requires 3 small production code changes (env var injection)

**Key behavioral notes discovered during planning:**
- `husako clean` without `--all`, `--cache`, or `--types` enters interactive mode even with `--yes`. Always use `--all` for "remove everything" in tests.
- `husako outdated` has **no** `--resources-only`/`--charts-only` flags — those exist only on `husako update`.
- `husako add --release` returns immediately without any network call (version provided directly).
- ArtifactHub chart `add` calls `discover_latest_artifacthub` **before** the duplicate check → mockito always needed.
- `run_auto_generate` is called after successful `add` and `remove` → use `write_release_cache()` or ensure no remaining resources to avoid network.

---

## Track A: Tests requiring no production code changes

### `husako clean`

```
clean_removes_types_dir
  Setup: create .husako/types/ with a file
  Run:   husako clean --types --yes
  Check: exit 0; .husako/types/ gone; stderr "Removed .husako/types/"

clean_removes_cache_dir
  Setup: create .husako/cache/ with a file
  Run:   husako clean --cache --yes
  Check: exit 0; .husako/cache/ gone; stderr "Removed .husako/cache/"

clean_removes_all_by_default
  Setup: create .husako/types/ and .husako/cache/
  Run:   husako clean --all --yes
  Check: exit 0; both dirs gone
  NOTE: `husako clean --yes` (no flag) enters interactive mode; must use `--all`
```

### `husako init`

```
init_creates_project_files
  Setup: empty dir
  Run:   husako init
  Check: exit 0; husako.toml created in current dir; no new subdirectory created
  NOTE:  init is in-place; unlike `husako new <dir>` it does not create a child directory

init_with_project_template
  Setup: empty dir
  Run:   husako init --template project
  Check: exit 0; husako.toml + [entries] section present
```

### `husako list` / `husako ls`

```
list_resources_from_config
  Setup: husako.toml with [resources] k8s + [charts] metallb
  Run:   husako list
  Check: exit 0; output contains "k8s" and "metallb"

list_resources_only_flag
  Setup: husako.toml with [resources] k8s + [charts] metallb
  Run:   husako list --resources
  Check: exit 0; output contains "k8s"; does NOT contain "metallb"

list_charts_only_flag
  Setup: husako.toml with [resources] k8s + [charts] metallb
  Run:   husako list --charts
  Check: exit 0; output contains "metallb"; does NOT contain "k8s"

list_empty_no_config
  Setup: empty dir (no husako.toml)
  Run:   husako list
  Check: exit 0; no crash

ls_alias_works
  Run:   husako ls (same as list_resources_from_config setup)
  Check: same output as husako list
```

### `husako remove` / `husako rm`

```
remove_resource_from_config
  Setup: husako.toml with [resources] k8s = release/1.35
  Run:   husako remove k8s --yes
  Check: exit 0; husako.toml no longer contains "k8s"; stderr "Removed k8s from [resources]"

remove_chart_from_config
  Setup: husako.toml with [charts] metallb = artifacthub/...
  Run:   husako remove metallb --yes
  Check: exit 0; husako.toml no longer contains "metallb"

remove_nonexistent_errors
  Setup: husako.toml with k8s only
  Run:   husako remove nonexistent --yes
  Check: exit non-0; stderr contains "not found"

rm_alias_works
  Same as remove_resource_from_config using "rm" subcommand
```

### `husako info`

```
info_no_config
  Setup: empty dir
  Run:   husako info
  Check: exit 0; no crash; some output (e.g., "no husako.toml" or "no dependencies")

info_with_config
  Setup: husako.toml with [resources] k8s + [charts] metallb + [entries] dev = "dev.ts"
  Run:   husako info
  Check: exit 0; output contains "k8s", "metallb", "dev"

info_with_types_dir
  Setup: husako.toml + pre-seeded .husako/types/
  Run:   husako info
  Check: exit 0; output mentions types dir stats

info_dependency_detail
  Setup: husako.toml with [resources] k8s = {source=release, version=1.35}
  Run:   husako info k8s
  Check: exit 0; output contains "k8s" and "1.35" (dependency-specific view)

info_dependency_not_found_errors
  Setup: husako.toml with [resources] k8s only
  Run:   husako info nonexistent
  Check: exit non-0; stderr contains "not found"
```

### `husako debug`

```
debug_no_config
  Setup: empty dir
  Run:   husako debug
  Check: exit 0; no crash (some checks fail gracefully)

debug_with_types_present
  Setup: husako.toml + .husako/types/ directory
  Run:   husako debug
  Check: exit 0; output contains positive health check for types

debug_missing_types_flags_warning
  Setup: husako.toml but no .husako/types/
  Run:   husako debug
  Check: exit 0; stderr/stdout contains warning about missing types
```

### `husako test` (TypeScript test runner)

```
test_passing_tests_exit_0
  Setup: entry.test.ts with passing describe/test blocks
  Run:   husako test entry.test.ts
  Check: exit 0; output shows pass count

test_failing_tests_exit_nonzero
  Setup: entry.test.ts with a failing assert
  Run:   husako test entry.test.ts
  Check: exit non-0; output shows failure

test_no_test_file_errors
  Setup: empty dir
  Run:   husako test nonexistent.ts
  Check: exit non-0; stderr contains file not found error

test_auto_discovery_no_files
  Setup: empty dir (no *.test.ts or *.spec.ts files)
  Run:   husako test   (no args — auto-discovery mode)
  Check: exit 0; stderr contains "No test files found"
  NOTE: Different from test_no_test_file_errors (explicit missing file → non-0).
        Confirmed: run_tests returns empty results → CLI prints "No test files found" → EXIT 0.
```

### `husako gen` — lock-based incremental skip

```
gen_with_lock_skips_unchanged_resources
  Setup: husako.toml with [resources] k8s = release/1.35
         husako.lock reflecting current version
         .husako/types/k8s/ already populated
  Run:   husako gen
  Check: exit 0; "Already up to date." output; .husako/types/ unchanged

gen_no_incremental_ignores_lock
  Setup: husako.toml with [resources] k8s = release/1.35
         husako.lock reflecting current version (would normally skip)
         write_release_cache(root, "1.35") — cache hit avoids network in generate
         NOTE: do NOT pre-seed .husako/types/ (contrast with gen_with_lock_skips_unchanged_resources)
  Run:   husako gen --no-incremental
  Check: exit 0; .husako/types/ was created (proves generation ran, not skipped)
```

### `husako render` — missing flag coverage

```
render_short_o_flag_writes_file
  Setup: valid entry.ts
  Run:   husako render entry.ts -o out.yaml
  Check: exit 0; out.yaml created; no stdout output
```

### `husako check` — compile error messages

```
check_compile_error_shows_message
  Setup: entry.ts with syntax error
  Run:   husako check entry.ts
  Check: exit 3; stderr contains compile error details

check_runtime_error_shows_message
  Setup: entry.ts with throw new Error("boom")
  Run:   husako check entry.ts
  Check: exit 4; stderr contains "boom"
```

---

## Track B: Tests requiring env var URL injection

### Production code changes (3 files)

**1. `crates/husako-core/src/version_check.rs`**

Add two private helpers near the top (after constants):

```rust
fn github_api_base() -> String {
    std::env::var("HUSAKO_GITHUB_API_URL")
        .unwrap_or_else(|_| GITHUB_API_BASE.to_string())
}

fn artifacthub_base() -> String {
    std::env::var("HUSAKO_ARTIFACTHUB_URL")
        .unwrap_or_else(|_| ARTIFACTHUB_BASE.to_string())
}
```

Update 5 public functions (one-liner each) to call these helpers instead of the constants.
Each already delegates to a `_from(base_url)` variant; just swap the constant for the helper:
- `discover_recent_releases` — change `GITHUB_API_BASE` → `&github_api_base()`
- `discover_latest_release` — same
- `search_artifacthub` — change `ARTIFACTHUB_BASE` → `&artifacthub_base()`
- `discover_latest_artifacthub` — same
- `discover_artifacthub_versions` — same

**2. `crates/husako-openapi/src/release.rs`**

Extract internal `fetch_release_specs_from(version, cache_dir, on_progress, base_url)`.
Public `fetch_release_specs` reads env var:

```rust
pub async fn fetch_release_specs(version, cache_dir, on_progress) {
    let base = std::env::var("HUSAKO_GITHUB_API_URL")
        .unwrap_or_else(|_| "https://api.github.com".to_string());
    fetch_release_specs_from(version, cache_dir, on_progress, &base).await
}
```

**3. `crates/husako-helm/src/artifacthub.rs`**

`resolve()` already calls `resolve_from(..., base_url)`.
`ARTIFACTHUB_BASE` in this file is `"https://artifacthub.io/api/v1/packages/helm"` (includes path),
while `version_check.rs` uses `"https://artifacthub.io"` (host only) and appends the path itself.
To keep both using the same env var, append the path when overriding:

```rust
pub async fn resolve(...) {
    let base = std::env::var("HUSAKO_ARTIFACTHUB_URL")
        .map(|u| format!("{u}/api/v1/packages/helm"))
        .unwrap_or_else(|_| ARTIFACTHUB_BASE.to_string());
    resolve_from(..., &base).await
}
```

This means when `HUSAKO_ARTIFACTHUB_URL=http://mock-server`:
- `artifacthub::resolve()` constructs `http://mock-server/api/v1/packages/helm/{package}/{version}`
- `version_check` ArtifactHub fns construct `http://mock-server/api/v1/packages/helm/{package}`
Both mock paths are consistent under the same server.

---

### New test helper in `integration.rs`

```rust
/// Pre-seed .husako/cache/release/v{tag}/ with a minimal valid OpenAPI spec
/// so fetch_release_specs() returns from cache — no network needed in tests.
fn write_release_cache(root: &Path, version: &str) {
    // version "1.35" → tag "v1.35.0"
    // Creates: cache/release/v1.35.0/_manifest.json
    //          cache/release/v1.35.0/apis__apps__v1_openapi.json
    // Uses a minimal OpenAPI v3 spec with one Deployment GVK
    // (reuses the schema structure from rich_mock_spec() / write_mock_spec())
}
```

### Mockito pattern for integration tests

```rust
#[tokio::test]
async fn update_bumps_version_in_toml() {
    let mut server = mockito::Server::new_async().await;
    server.mock("GET", "/repos/kubernetes/kubernetes/tags")
        .match_query(mockito::Matcher::Any)
        .with_status(200)
        .with_body(r#"[{"name":"v1.35.0"}]"#)
        .create_async().await;

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // ... setup husako.toml ...

    husako_at(root)
        .args(["update"])
        .env("HUSAKO_GITHUB_API_URL", server.url())
        .assert()
        .success();
    // ... check husako.toml changed ...
}
```

The `env()` call passes the var to the husako subprocess via assert_cmd.

---

### `husako gen` — with configured sources (Track B)

```
gen_with_release_source_uses_cache
  Setup: husako.toml [resources] k8s = release/1.35; write_release_cache(root, "1.35")
  Run:   husako gen   (no env var needed — cache hit)
  Check: exit 0; .husako/types/k8s/ created; stderr contains "✔"

gen_with_release_source_fails_gracefully_on_network_error
  Setup: husako.toml [resources] k8s = release/1.35 (no cache)
         HUSAKO_GITHUB_API_URL → mockito returning 500
  Run:   husako gen
  Check: exit non-0; stderr contains error description

gen_with_chart_artifacthub_source
  Setup: husako.toml [charts] metallb = {source=artifacthub, package=..., version=0.15.3}
         HUSAKO_ARTIFACTHUB_URL → mockito returning package JSON with values_schema
  Run:   husako gen --skip-k8s
  Check: exit 0; .husako/types/helm/metallb.d.ts created
```

### `husako add` — chart and resource flows (Track B)

```
add_artifacthub_chart_writes_toml
  Setup: husako.toml (empty)
         HUSAKO_ARTIFACTHUB_URL → mockito responding at /api/v1/packages/helm/metallb/metallb
           body: {"version": "0.15.3", "available_versions": [...]}
         write_release_cache not needed (chart add, not gen)
  Run:   husako add metallb/metallb --name metallb
  Check: exit 0; husako.toml has metallb entry; stderr contains "Added metallb to [charts]"
  NOTE: `add` calls `discover_latest_artifacthub` → needs HUSAKO_ARTIFACTHUB_URL mock
        Then calls `run_auto_generate` on success → with no [resources] in toml, exits OK (no-op)

add_release_resource_writes_toml
  Setup: husako.toml (empty)
         HUSAKO_GITHUB_API_URL → mockito returning tags [v1.35.0] (for version discovery — unused
           since --release 1.35 provides version directly, no discovery needed)
         write_release_cache(root, "1.35") — for run_auto_generate that fires after add
  Run:   husako add --release 1.35 --name k8s
  Check: exit 0; husako.toml has k8s = {source=release, version=1.35}; stderr contains "Added k8s"
  NOTE: `--release` provides version directly — NO network call for version discovery.
        BUT `run_auto_generate` is called on success → hits release spec download.
        write_release_cache pre-seeds the cache so auto-generate is a cache hit (no network).

add_duplicate_resource_shows_message
  Setup: husako.toml with [resources] k8s = release/1.35
  Run:   husako add --release 1.35 --name k8s
  Check: exit 0; husako.toml unchanged; stderr "k8s is already in [resources]"
  NOTE: `--release` branch returns immediately WITHOUT any network call.
        `AlreadyExists` branch does NOT call run_auto_generate. Track A — no mockito needed.

add_duplicate_chart_shows_message
  Setup: husako.toml with [charts] metallb = {source=artifacthub, package=metallb/metallb, version=0.15.3}
         HUSAKO_ARTIFACTHUB_URL → mockito (returns any valid JSON — version discovery runs before duplicate check)
  Run:   husako add metallb/metallb --name metallb
  Check: exit 0; husako.toml unchanged; stderr "metallb is already in [charts]"
  NOTE: ArtifactHub chart add calls discover_latest_artifacthub BEFORE duplicate check.
        Mockito is required even for the duplicate case.
```

### `husako outdated` — all source types (Track B)

```
outdated_release_source_newer_available
  Setup: husako.toml k8s = release/1.34; HUSAKO_GITHUB_API_URL → mockito [v1.35.0]
  Run:   husako outdated
  Check: exit 0; output contains "1.34" and "1.35"; indicates outdated

outdated_release_source_up_to_date
  Setup: husako.toml k8s = release/1.35; HUSAKO_GITHUB_API_URL → mockito [v1.35.0]
  Run:   husako outdated
  Check: exit 0; output contains "up to date"

outdated_artifacthub_chart_newer_available
  Setup: husako.toml metallb = {artifacthub, version=0.14.0}
         HUSAKO_ARTIFACTHUB_URL → mockito returns {version: "0.15.3"}
  Run:   husako outdated
  Check: exit 0; output contains "0.14.0" and "0.15.3"

outdated_no_config_shows_no_deps
  Setup: empty dir (no husako.toml)
  Run:   husako outdated
  Check: exit 0; no crash; stderr "No versioned dependencies found"
  NOTE: `husako outdated` has no --resources-only/--charts-only flags (those are on `husako update`)

outdated_network_error_graceful
  Setup: husako.toml with [resources] k8s = release/1.34  (must have a resource to trigger network)
         HUSAKO_GITHUB_API_URL → mockito returning 500
  Run:   husako outdated
  Check: exit 0 (error entry logged); no panic; stderr contains error description
  NOTE:  Without a husako.toml resource, check_outdated returns early (no network call fired)
```

### `husako update` — all scenarios (Track B)

```
update_bumps_resource_version_in_toml
  Setup: husako.toml k8s = release/1.34; HUSAKO_GITHUB_API_URL → mockito [v1.35.0]
         write_release_cache(root, "1.35") so regen uses cache
  Run:   husako update
  Check: exit 0; husako.toml contains "1.35"; stderr "Updated k8s"

update_dry_run_does_not_modify_toml
  Setup: same husako.toml; same mocks
  Run:   husako update --dry-run
  Check: exit 0; husako.toml still contains "1.34"; stderr "Would update k8s"

update_resources_only_skips_charts
  Setup: husako.toml k8s (1.34) + metallb (0.14.0)
         HUSAKO_GITHUB_API_URL → mockito [v1.35.0] (for k8s version check)
         write_release_cache(root, "1.35") (for auto-regen after update)
  Run:   husako update --resources-only
  Check: exit 0; k8s updated to 1.35; metallb version unchanged in toml

update_charts_only_skips_resources
  Setup: husako.toml k8s (1.34) + metallb (0.14.0)
         HUSAKO_ARTIFACTHUB_URL → mockito returning {version: "0.15.3", available_versions: [...]}
  Run:   husako update --charts-only
  Check: exit 0; metallb updated to 0.15.3; k8s version unchanged in toml

update_name_filter_updates_one
  Setup: husako.toml k8s (1.34) + metallb (0.14.0)
         HUSAKO_GITHUB_API_URL → mockito [v1.35.0]  (k8s version check)
         HUSAKO_ARTIFACTHUB_URL → mockito [{version:"0.15.3"}]  (metallb checked by check_outdated even though not updated)
         write_release_cache(root, "1.35")  (for auto-regen after update)
  Run:   husako update k8s
  Check: exit 0; k8s version updated to 1.35; metallb still 0.14.0 in toml
  NOTE:  update calls check_outdated first, which checks ALL deps including metallb → both mocks needed

update_all_current_no_changes
  Setup: husako.toml k8s = 1.35; HUSAKO_GITHUB_API_URL → mockito [v1.35.0]
  Run:   husako update
  Check: exit 0; husako.toml unchanged; stderr "up to date"
```

---

## Complete file change list

| File | Change | Lines affected |
|---|---|---|
| `crates/husako-core/src/version_check.rs` | Add `github_api_base()` + `artifacthub_base()` helpers; update 5 public fns | ~10 lines |
| `crates/husako-openapi/src/release.rs` | Extract `_from()` + read env var in public fn | ~15 lines |
| `crates/husako-helm/src/artifacthub.rs` | Read env var in `resolve()` | ~5 lines |
| `crates/husako-cli/tests/integration.rs` | Add `write_release_cache()` + ~50 new tests | ~800 lines |

---

## Tests to add / modify

- **Add**: ~50 new tests in `crates/husako-cli/tests/integration.rs`
- **Add**: `write_release_cache()` helper in `integration.rs`
- **Modify**: None — existing tests unaffected (env vars default to real URLs when unset)

## Docs to add / modify

**`.claude/testing.md`** — add new subsection after the existing "CLI integration tests (assert_cmd)" block (line 230):

```markdown
### CLI integration tests — mocking network calls in subprocess

The `husako` binary runs as a subprocess in integration tests (assert_cmd). Mockito runs
in the test process, so URL injection via `_from()` is not directly usable. Instead:

**Strategy A — env var injection** (for HTTP calls to GitHub API / ArtifactHub):

Two env vars override the hardcoded base URLs inside the binary:

| Env var | Overrides | Used by |
|---|---|---|
| `HUSAKO_GITHUB_API_URL` | `https://api.github.com` | version_check + release spec listing |
| `HUSAKO_ARTIFACTHUB_URL` | `https://artifacthub.io` | version_check + helm/artifacthub |

Pattern:
```rust
#[tokio::test]
async fn example() {
    let mut server = mockito::Server::new_async().await;
    server.mock("GET", "/repos/kubernetes/kubernetes/tags")
        .match_query(mockito::Matcher::Any)
        .with_body(r#"[{"name":"v1.35.0"}]"#)
        .with_status(200)
        .create_async().await;

    husako_at(root)
        .args(["outdated"])
        .env("HUSAKO_GITHUB_API_URL", server.url())
        .assert()
        .success();
}
```

**Strategy B — cache pre-seeding** (for `husako gen` with release source):

Pre-populate `.husako/cache/release/v{tag}/` before running the binary.
`fetch_release_specs()` returns the cached specs without any HTTP call.

Use the `write_release_cache(root, version)` helper in `integration.rs` to seed
a minimal valid OpenAPI spec (sufficient for type generation, no network needed).

**When to use which:**
- `husako outdated` / `husako update` → env var (always makes live API calls)
- `husako gen --[resources]` → cache pre-seeding (cache hit skips network)
- `husako gen --[charts]` → env var (ArtifactHub / registry calls happen at gen time)
- `husako add <chart>` → env var (version discovery during add)
```

---

## Verification

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
# Spot-check specific groups:
cargo test -p husako --test integration clean_
cargo test -p husako --test integration list_
cargo test -p husako --test integration remove_
cargo test -p husako --test integration info_
cargo test -p husako --test integration debug_
cargo test -p husako --test integration outdated_
cargo test -p husako --test integration update_
cargo test -p husako --test integration add_
cargo test -p husako --test integration gen_with
```
