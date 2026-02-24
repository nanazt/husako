# husako Testing Standards

## Test Levels

1. **Unit tests** — inline `#[cfg(test)]` in each source file. Test single functions in isolation,
   mock network calls via `mockito`, use `tempfile` for filesystem. No external network.
   Target: every public function has at least one happy-path and one error test.

2. **Integration tests** — `tests/` dir in crates that need cross-function flows
   (e.g., `husako-core`). Use `assert_cmd` for CLI exit codes, `insta` for YAML snapshots.

3. **E2E tests** — `scripts/e2e.sh`. Tests the full binary against real network sources.
   Network access is allowed. Run with `bash scripts/e2e.sh` (release binary) or
   `HUSAKO_BIN=./target/debug/husako bash scripts/e2e.sh`.

## E2E Test Principles

### Verify side effects, not just output

Every state-changing command must have its side effects explicitly verified:

| Command | What to verify |
|---------|----------------|
| `husako add` | husako.toml has correct `source`, `package`/`repo`/`path`, `version` fields |
| `husako gen` | `.d.ts` file exists AND contains expected `export` declarations |
| `husako remove` | Key is completely absent from husako.toml; re-gen and verify type file also absent |
| `husako update` | Version string actually changed in husako.toml; type file mtime changed (regeneration happened) |
| `husako clean` | `.husako/` directory completely removed (not just emptied) |
| `husako plugin add` | husako.toml entry present AND `.husako/plugins/<name>/` directory exists after gen |
| `husako plugin remove` | husako.toml entry absent AND `.husako/plugins/<name>/` directory removed |
| `husako plugin list` | Output contains / does not contain the plugin name |

### Validate Kubernetes YAML with kubectl

Standard k8s resource outputs must pass `kubectl apply --dry-run=client -f -`:

```bash
assert_k8s_valid "Deployment" "$yaml"
```

This validates that the YAML is structurally correct and the `apiVersion`, `kind`,
`metadata.name` fields are present with correct values.

**Custom resource outputs** (CRDs, FluxCD objects) cannot be validated with client-side dry-run
because kubectl doesn't know their schema without the CRD installed. Use YAML validation instead:

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

### State isolation

- Read-only tests (Scenario A) use the committed `test/e2e/` directory
- State-modifying tests (B, C, D, E) use `mktemp -d` temp dirs
- Temp dirs are cleaned with `trap 'rm -rf "$tmpdir"' EXIT` inside subshell `()`

### Stderr vs stdout

husako diagnostic output (list, info, debug, validate, outdated, plugin list) goes to stderr.
Capture it with `2>&1`:

```bash
local list_out; list_out=$("$HUSAKO" list 2>&1)
```

Only `husako render` writes to stdout.

### CLI flag notes

- `husako clean` requires `--all` (or `--cache`/`--types`) for non-interactive operation.
  Without these flags it opens an interactive prompt even with `-y`.
- `husako remove <name>` requires `-y` to skip the confirmation dialog when name is a CLI arg.
- `husako plugin add` uses `--path <dir>` or `--url <url>`. There is no `--source` flag.
- `husako plugin remove` and `husako plugin add` do not ask for confirmation; `-y` is not needed.

### Code reuse via helpers

```bash
init_project()        # write husako.toml + copy tsconfig.json
write_configmap()     # write a minimal ConfigMap entry.ts
assert_contains()     # grep-based string assertion with pass/fail output
assert_file()         # file existence assertion
assert_no_dir()       # directory absence assertion
assert_toml_field()   # check husako.toml has field=value on the same line
assert_toml_key_absent()  # check dep name is not a top-level TOML key
assert_dts_exports()  # check .d.ts file has expected export symbol
assert_k8s_valid()    # kubectl --dry-run=client validation (standard k8s only)
assert_valid_yaml()   # python3 yaml.safe_load_all validation (any YAML)
```

## Unit Test Patterns

### Helm resolver tests
- Use `mockito::Server` for HTTP mock server
- Pre-populate cache directory to test delegation paths without real network
- Test: cache hit → returns cached; cache miss + valid network → caches; network error → error
- Test each source kind's happy path and common error cases

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

# E2E (network OK, requires kubectl + built binary)
cargo build --release --bin husako
bash scripts/e2e.sh

# E2E with debug binary (faster iteration)
cargo build --bin husako
HUSAKO_BIN=./target/debug/husako bash scripts/e2e.sh
```
