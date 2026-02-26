# husako Testing Standards

## Test Levels

1. **Unit tests** — inline `#[cfg(test)]` in each source file. Test single functions in isolation,
   mock network calls via `mockito`, use `tempfile` for filesystem. No external network.
   Target: every public function has at least one happy-path and one error test.

2. **Integration tests** — `tests/` dir in crates that need cross-function flows
   (e.g., `husako-core`). Use `assert_cmd` for CLI exit codes, `insta` for YAML snapshots.

3. **E2E tests** — `crates/husako-cli/tests/e2e_*.rs`. Tests the full binary against real network
   sources using `assert_cmd`. Local tests (Scenario G) always run; network tests (A–F) are tagged
   `#[ignore]` and run in CI via `--include-ignored`.

## E2E Test Principles

### Verify side effects, not just output

Every state-changing command must have its side effects explicitly verified:

| Command | What to verify |
|---------|----------------|
| `husako add` | husako.toml has correct `source`, `package`/`repo`/`path`, `version` fields |
| `husako gen` | `.d.ts` file exists AND contains expected `export` declarations; `husako.lock` exists at project root with correct `format_version = 1` |
| `husako remove` | Key is completely absent from husako.toml; re-gen and verify type file also absent |
| `husako update` | Version string actually changed in husako.toml; type file mtime changed (regeneration happened) |
| `husako clean` | `.husako/` directory completely removed (not just emptied) |
| `husako plugin add` | husako.toml entry present AND `.husako/plugins/<name>/` directory exists after gen |
| `husako plugin remove` | husako.toml entry absent AND `.husako/plugins/<name>/` directory removed |
| `husako plugin list` | Output contains / does not contain the plugin name |
| `husako test` | Exit 0 on all-pass, exit 1 on any failure; output contains test names and pass/fail marks |

### Validate Kubernetes YAML with kubeconform

Standard k8s resource outputs must pass `kubeconform -strict -`:

```bash
assert_k8s_valid "Deployment" "$yaml"
```

This validates that the YAML is structurally correct using kubeconform's built-in schemas
(no cluster required). The CI workflow installs kubeconform automatically.

**Custom resource outputs** (CRDs, FluxCD objects) and non-k8s YAML (Helm values) use
`assert_valid_yaml` instead, which validates YAML syntax via Ruby's built-in Psych parser
(no extra dependencies — works on macOS and ubuntu-latest out of the box):

```bash
assert_valid_yaml "HelmRelease" "$yaml"
```

### Match specific expected values

Pattern matches must be specific enough to distinguish correct from incorrect output:

```bash
# BAD: matches anything with "replicaCount"
grep -q "replicaCount"

# GOOD: checks exact rendered value
assert_contains "replicaCount is 2" "replicaCount: 2" "$yaml"
```

### Generated type name convention

Chart and resource type names are derived from the config key via `to_pascal_case()`, which
splits on `-`, `_`, `.` and capitalizes each segment:

| Config key | Generated type name |
|------------|---------------------|
| `pg` | `Pg` |
| `redis-reg` | `RedisReg` |
| `local-chart` | `LocalChart` |
| `prom-git` | `PromGit` |
| `cert-manager` | (resource types use API group names, not the config key) |

### Source kind coverage

All source kinds are exercised end-to-end:

| Source | Kind | Scenario |
|--------|------|----------|
| `release` | resource | A — k8s 1.35 |
| `file` | resource | C — local CRD YAML |
| `git` | resource | C — cert-manager CRDs |
| `file` | chart | A — local JSON schema |
| `artifacthub` | chart | B — bitnami/postgresql |
| `registry` | chart | B — bitnami HTTP → OCI delegation |
| `git` | chart | B — prometheus-community/helm-charts |
| `oci` | chart | F — bitnamicharts/postgresql OCI registry |
| `husako test` | TS test runner | G — passing tests, failing tests, discovery, plugin testing |
| `husako test` | SDK unit tests | G5 — husako/base/matchers |

### State isolation

- Read-only tests (Scenario A) use the committed `test/e2e/` fixture directory
- State-modifying tests (B, C, D, E, F) use `tempfile::TempDir` — auto-cleaned on drop
- Scenario G tests use `e2e_tmpdir()` — creates `TempDir` inside `test/e2e/` to avoid macOS `/tmp` symlink mismatch
- SDK unit tests (Scenario G5) use `crates/husako-sdk/tests/` directly — no tempdir, no network, no husako gen

### Stderr vs stdout

husako diagnostic output (list, info, debug, check, outdated, plugin list) goes to stderr.
Only `husako render` writes to stdout.

In Rust tests, capture both and check appropriately:
```rust
let output = husako_at(dir).args(["list"]).output().unwrap();
let stderr = String::from_utf8_lossy(&output.stderr);  // diagnostics
let stdout = String::from_utf8_lossy(&output.stdout);  // render YAML only
// For commands that mix stdout+stderr (e.g. husako test):
let combined = output_combined(&output);
```

### CLI flag notes

- `husako clean` requires `--all` (or `--cache`/`--types`) for non-interactive operation.
  Without these flags it opens an interactive prompt even with `-y`.
- `husako remove <name>` requires `-y` to skip the confirmation dialog when name is a CLI arg.
- `husako plugin add` uses `--path <dir>` or `--url <url>`. There is no `--source` flag.
- `husako plugin remove` and `husako plugin add` do not ask for confirmation; `-y` is not needed.
- `husako gen --no-incremental` regenerates all types regardless of `husako.lock`. Use in tests that need a clean generation state without relying on lock skip behavior.
- `husako add --chart --source oci` requires `--reference <oci://...>` and `--version <tag>`.
  The chart name for type generation is derived from the last path component of the reference
  (e.g. `oci://registry-1.docker.io/bitnamicharts/postgresql` → chart name `postgresql`).

### Code reuse via helpers

Shared helpers live in `crates/husako-cli/tests/e2e_common/mod.rs`.
Each scenario file accesses them with `mod e2e_common; use e2e_common::*;`.

```rust
husako_at(dir)              // assert_cmd::Command for husako in dir
e2e_fixtures_dir()          // absolute path to test/e2e/
e2e_tmpdir()                // TempDir inside test/e2e/ (macOS-safe for Scenario G)
init_project(dir, toml)     // write husako.toml + copy tsconfig.json
write_configmap(path)       // write a minimal ConfigMap entry.ts
output_combined(&output)    // combine stdout+stderr into one string
assert_contains(desc, pat, content)     // substring assertion with #[track_caller]
assert_not_contains(...)                // negative substring assertion
assert_file(path)           // file existence assertion
assert_no_dir(path)         // directory absence assertion
assert_toml_field(dir, f, v, desc)      // husako.toml has field+value on same line
assert_toml_key_absent(dir, key)        // dep name not a top-level TOML key
assert_dts_exports(path, symbol)        // .d.ts file has expected export symbol
assert_k8s_valid(yaml, desc)            // kubeconform -strict (skips if not installed)
assert_valid_yaml(yaml, desc)           // serde_yaml_ng structural validation
copy_dir_all(src, dst)      // recursive directory copy
```

## TypeScript Test Files (`husako test`)

Test files for husako TypeScript code use the naming convention `*.test.ts` or `*.spec.ts`.
They import from the `"husako/test"` builtin module:

```typescript
import { test, describe, expect } from "husako/test";

describe("suite name", () => {
  test("case name", () => {
    expect(value).toBe(expected);
  });
});
```

Discovery skips `.husako/`, `node_modules/`, and all hidden directories. Discovery is recursive.

Available matchers: `toBe`, `toEqual`, `toBeDefined`, `toBeUndefined`, `toBeNull`, `toBeTruthy`,
`toBeFalsy`, `toBeGreaterThan`, `toBeGreaterThanOrEqual`, `toBeLessThan`, `toBeLessThanOrEqual`,
`toContain`, `toHaveProperty`, `toHaveLength`, `toMatch`, `toThrow`. All support `.not` negation.

`husako gen --skip-k8s` must be run before `husako test` so `husako/test.d.ts` and
`tsconfig.json` path mappings are written. For plugin tests, run `husako gen` first to
install plugins.

Exit code: 0 if all tests pass, 1 if any test fails or cannot compile/run.

## Unit Test Patterns

### Helm resolver tests

| Source | File | Strategy |
|--------|------|----------|
| `registry` | `registry.rs` | mockito for index.yaml + .tgz; tempdir cache pre-populate for cache-hit + OCI-delegation paths |
| `artifacthub` | `artifacthub.rs` | mockito via `resolve_from(url)` pattern; tempdir cache for cache-hit |
| `oci` | `oci.rs` | mockito for v2 ping / manifests / blobs; tempdir cache for cache-hit |
| `git` | `git.rs` | E2E only (requires `git clone`) |
| `file` | `file.rs` | tempdir only; no HTTP |

Key patterns:
- Use `mockito::Server` for HTTP mock server
- Pre-populate cache directory (`{tmpdir}/helm/{source}/{hash}/{version}.json`) to test
  delegation paths without real network
- For OCI delegation inside registry tests: pre-populate `helm/oci/{cache_hash(oci_url)}/{version}.json`
- Test: cache hit → returns cached; cache miss + valid network → caches; network error → error

### version_check.rs tests — `_from()` pattern

Functions that hard-code base URLs are split into a public wrapper and a private `_from(base_url)`
variant for testability. The public function passes the production constant; tests pass the
mockito server URL.

```
const GITHUB_API_BASE: &str = "https://api.github.com";
const ARTIFACTHUB_BASE: &str = "https://artifacthub.io";

pub fn discover_latest_release() -> Result<…> {
    discover_latest_release_from(GITHUB_API_BASE)
}
fn discover_latest_release_from(base_url: &str) -> Result<…> { … }
```

Functions covered by this pattern:
- `search_artifacthub` / `search_artifacthub_from`
- `discover_recent_releases` / `discover_recent_releases_from`
- `discover_latest_release` / `discover_latest_release_from`
- `discover_latest_artifacthub` / `discover_latest_artifacthub_from`
- `discover_artifacthub_versions` / `discover_artifacthub_versions_from`

Functions already URL-parametric (no refactoring needed):
- `discover_registry_versions(repo, chart, …)` — pass mockito URL as `repo`
- `discover_latest_registry(repo, chart)` — same

### OpenAPI / DTS tests
- Use fixture JSON from `tests/fixtures/` (checked-in real k8s spec excerpts)
- Snapshot test generated `.d.ts` with `insta` for regression detection
- Test both builder generation and interface generation paths

### CLI integration tests (assert_cmd)
- Test exit codes for each error condition
- Use tempdir + write minimal husako.toml
- Capture stdout/stderr and check for key strings
- Do NOT make real network calls — mock everything at unit/integration level

## Running Tests

```bash
# All tests (no network)
cargo test --workspace --all-features

# Specific crate
cargo test -p husako-helm

# Specific test by name
cargo test -p husako-core schema_source

# E2E local only (Scenario G — no network)
cargo test -p husako --test e2e_g

# SDK unit tests only (Scenario G5, no network)
cargo test -p husako --test e2e_g g5_sdk_unit_tests

# E2E full (all scenarios A-G — requires network + kubeconform)
cargo test -p husako --test e2e_a --test e2e_b --test e2e_c --test e2e_d --test e2e_e --test e2e_f --test e2e_g -- --include-ignored
```
