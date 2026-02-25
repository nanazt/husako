# Plan: Tokio-Based Async / Parallel Migration

## Context

The entire project is synchronous. The two biggest bottlenecks are:

1. **`husako generate`**: fetches OpenAPI specs one-by-one via HTTP (~40 GVs × 100 ms =
   ~4 s), then resolves Helm charts one-by-one (~2–10 s each). All I/O is sequential.
2. **Multi-file render**: each TypeScript entry file is compiled + executed sequentially.

Introducing `tokio` enables:
- Parallel HTTP fetches for OpenAPI specs → 5–10× speedup on `husako generate`
- Parallel chart resolution → 2–4× speedup when multiple charts are configured
- `spawn_blocking` for CPU-bound work (oxc compile, QuickJS execute, dts codegen)
  so the async runtime is never blocked
- `tokio::process::Command` for git clone subprocesses (currently uses
  `std::process::Command` which blocks the thread)

## Design Decisions

| Decision | Choice | Reason |
|----------|--------|--------|
| Runtime | `tokio` | standard; already implied by `reqwest` 0.13 async |
| Parallelism primitive | `tokio::task::JoinSet` | no extra dep, handles dynamic tasks + errors |
| CPU-bound work | `spawn_blocking` | oxc/QuickJS/dts are not `Send`-safe or are heavy compute |
| `reqwest` | remove `blocking` feature, keep `json` | blocking client no longer needed |
| `ProgressReporter` trait | keep sync | progress calls are fast; `dyn ProgressReporter + Send + Sync` |
| `HusakoError` | verify `Send + Sync` | required for `.await?` propagation across tasks |
| Async fn in trait | native (Rust 2024, stable) | no `async_trait` crate needed |

## Crate Dependency Order (bottom-up)

```
husako-openapi    ← async reqwest + JoinSet for parallel spec fetching
husako-helm       ← async reqwest + JoinSet + tokio::process::Command
husako-core       ← async generate() + schema_source + plugin git clone
husako-cli        ← #[tokio::main], spawn_blocking for render, parallel multi-file
```

`husako-compile-oxc`, `husako-runtime-qjs`, `husako-dts`, `husako-yaml`, `husako-sdk`,
`husako-config` stay **synchronous** (CPU-bound or trivial I/O; called via
`spawn_blocking` where needed).

---

## Task 1 — Workspace `Cargo.toml`

**File:** `Cargo.toml`

```toml
# Change:
reqwest = { version = "0.13", features = ["blocking", "json"] }
# To:
reqwest = { version = "0.13", features = ["json"] }

# Add:
tokio = { version = "1", features = ["rt-multi-thread", "macros", "process"] }
```

Add `tokio.workspace = true` to every crate that needs it (openapi, helm, core, cli).

---

## Task 2 — `husako-openapi`: Async HTTP + Parallel Spec Fetching

**Files:** `Cargo.toml`, `src/lib.rs`, `src/fetch.rs`, `src/release.rs`, `src/kubeconfig.rs`

### Cargo.toml
```toml
[dependencies]
tokio = { workspace = true }
```

### `fetch.rs` / `lib.rs`
- `fn fetch_spec(gv) -> Result<...>` → `async fn fetch_spec(gv) -> Result<...>`
- `fn discover() -> Result<...>` → `async fn discover() -> Result<...>`
- `fn fetch_all_specs()` → `async fn fetch_all_specs()` using `JoinSet`:

```rust
pub async fn fetch_all_specs(&self) -> Result<HashMap<String, Value>, OpenApiError> {
    let index = self.discover().await?;
    let mut set = tokio::task::JoinSet::new();
    for gv in index.paths.into_keys() {
        let client = self.client.clone();   // reqwest::Client is Arc inside, cheap to clone
        let base_url = self.base_url.clone();
        set.spawn(async move { fetch_spec(&client, &base_url, gv).await });
    }
    let mut specs = HashMap::new();
    while let Some(res) = set.join_next().await {
        let (key, value) = res??;
        specs.insert(key, value);
    }
    Ok(specs)
}
```

### `release.rs`
- `fn fetch_release_specs()` → `async fn`
- Uses reqwest async client throughout

### `kubeconfig.rs`
- `fn load_credentials()` → stays sync (reads local files, fast)

### Tests (`tests/`)
- `#[test]` → `#[tokio::test]` for any test calling async functions
- `mockito` is async-compatible; mock server creation unchanged

---

## Task 3 — `husako-helm`: Async HTTP + Parallel Chart Resolution

**Files:** `Cargo.toml`, `src/lib.rs`, `src/registry.rs`, `src/artifacthub.rs`, `src/git.rs`, `src/file.rs`

### Cargo.toml
```toml
[dependencies]
tokio = { workspace = true }
```

### `lib.rs` — `resolve_all()` → async + parallel
```rust
pub async fn resolve_all(
    charts: &HashMap<String, ChartSource>,
    project_root: &Path,
    cache_dir: &Path,
) -> Result<HashMap<String, Value>, HelmError> {
    let mut set = tokio::task::JoinSet::new();
    for (name, source) in charts {
        let name = name.clone();
        let source = source.clone();           // ChartSource must impl Clone
        let project_root = project_root.to_owned();
        let cache_dir = cache_dir.to_owned();
        set.spawn(async move {
            let schema = resolve(&name, &source, &project_root, &cache_dir).await?;
            Ok::<_, HelmError>((name, schema))
        });
    }
    let mut result = HashMap::new();
    while let Some(res) = set.join_next().await {
        let (name, schema) = res??;
        result.insert(name, schema);
    }
    Ok(result)
}
```

### `registry.rs`, `artifacthub.rs`
- All `reqwest::blocking::*` → `reqwest::*` (async)
- Functions become `async fn`

### `git.rs` — `std::process::Command` → `tokio::process::Command`
```rust
// Before:
std::process::Command::new("git").args([...]).output()?;

// After:
tokio::process::Command::new("git").args([...]).output().await?;
```

### Tests
- `#[test]` → `#[tokio::test]` for async test functions

---

## Task 4 — `husako-core`: Async `generate()` + Parallel Schema Sources

**Files:** `Cargo.toml`, `src/lib.rs`, `src/schema_source.rs`, `src/plugin.rs`

### Cargo.toml
```toml
[dependencies]
tokio = { workspace = true }
```

### `src/schema_source.rs` — `resolve_all()` → async + parallel

```rust
pub async fn resolve_all(
    resources: &IndexMap<String, ResourceSource>,
    ...
) -> Result<HashMap<String, Value>, HusakoError> {
    let mut set = tokio::task::JoinSet::new();
    for (name, source) in resources {
        // spawn per source
    }
    // collect and merge
}
```

### `src/plugin.rs` — `install_plugins()`: git clone → `tokio::process::Command`
Plugin installation becomes `async fn install_plugins()`.

### `src/lib.rs` — `generate()` → `async fn`

```rust
pub async fn generate(
    options: &GenerateOptions,
    progress: &(dyn ProgressReporter + Send + Sync),
) -> Result<(), HusakoError> {
    install_plugins(...).await?;
    // parallel: schema sources + helm charts concurrently
    let (specs, chart_schemas) = tokio::try_join!(
        schema_source::resolve_all(...),
        husako_helm::resolve_all(...),
    )?;
    // CPU-bound dts codegen in spawn_blocking
    let result = tokio::task::spawn_blocking(move || {
        husako_dts::generate(&GenerateOptions { specs })
    }).await??;
    // write files (fast I/O)
    write_output(result)?;
    Ok(())
}
```

### `src/lib.rs` — `render()`: stays `fn` (CPU-bound)

`render()` itself stays synchronous (all CPU-bound steps). Callers use
`spawn_blocking` when rendering concurrently.

### `ProgressReporter` trait
Change bound to `ProgressReporter + Send + Sync` at call sites.
Implementations (`IndicatifReporter`, `SilentProgress`) already implement `Send + Sync`
(verify: `ProgressBar` from `indicatif` is `Send + Sync`).

### `HusakoError` — verify Send + Sync
Confirm all error variants contain `Send + Sync` types. If any variant wraps a
non-Send type, wrap it in `Arc` or convert to `String`.

---

## Task 5 — `husako-cli`: `#[tokio::main]` + Parallel Multi-File Render

**File:** `Cargo.toml`, `src/main.rs`

### Cargo.toml
```toml
[dependencies]
tokio = { workspace = true }
```

### `src/main.rs`
```rust
#[tokio::main]
async fn main() -> ExitCode {
    ...
}
```

All `husako_core::generate(...)` calls become `.await`.

For multi-file render (if user passes multiple entry files), spawn concurrently:
```rust
let mut set = tokio::task::JoinSet::new();
for file in entry_files {
    set.spawn(tokio::task::spawn_blocking(move || {
        husako_core::render(&source, &file, &options)
    }));
}
while let Some(res) = set.join_next().await { ... }
```

---

## Task 6 — Drop Large Values in Background

**Reference:** https://abrams.cc/rust-dropping-things-in-another-thread

Dropping large heap-allocated values (e.g. `HashMap<String, serde_json::Value>` of
OpenAPI specs) can take several milliseconds (O(n) in number of nodes). In async code
this blocks the task executor. Move the drop to a blocking thread for those cases.

**Helper utility** in `husako-core/src/lib.rs` (or a `util.rs`):

```rust
/// Moves `value` to a blocking thread for deallocation,
/// so the async task is not paused by a large drop.
fn drop_in_background<T: Send + 'static>(value: T) {
    tokio::task::spawn_blocking(move || drop(value));
}
```

**Use in 2 places only:**

1. After `husako_dts::generate()` consumes the spec map, drop the input specs in background:
   ```rust
   // Inside generate(), after dts result is written:
   drop_in_background(specs_map); // HashMap<String, Value> — can be 5-20 MB
   ```

2. After `husako_helm::resolve_all()` result is written to disk, drop the chart schemas
   map in background:
   ```rust
   drop_in_background(chart_schemas); // HashMap<String, Value>
   ```

Do **not** apply to small or trivially-dropped values (String, Vec<u8> under ~1 MB,
owned primitives). Only use where the HashMap/Value tree is known to be large.

---

## Task 7 — Test Infrastructure

- `mockito::Server::new()` → `mockito::Server::new_async().await` in async tests
- All test functions in `husako-openapi` and `husako-helm` that use HTTP mocks:
  `#[test]` → `#[tokio::test]`
- `husako-core` tests calling `generate()`: `#[tokio::test]`
- E2E tests (`assert_cmd`): unchanged (CLI subprocess, not affected)

---

## Verification

```bash
# 1. Compile check
cargo build --workspace

# 2. Lint
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 3. Tests
cargo test --workspace --all-features

# 4. E2E (no network required)
cargo test -p husako --test e2e_g

# 5. Manual: measure generate speedup
time cargo run --release --bin husako -- gen  # in a project with husako.toml
```

Expected: `husako generate` with k8s release source drops from ~4 s to ~0.8 s
(parallel fetch of ~40 GVs). Chart resolution drops proportionally.

---

## Files Changed Summary

| File | Change |
|------|--------|
| `Cargo.toml` | add tokio workspace dep, remove `blocking` from reqwest |
| `husako-openapi/Cargo.toml` | add tokio |
| `husako-openapi/src/lib.rs` | async fetch_all_specs + JoinSet |
| `husako-openapi/src/fetch.rs` | async fn + async reqwest |
| `husako-openapi/src/release.rs` | async fn |
| `husako-openapi/src/**` (inline tests) | #[tokio::test] for mock-using tests |
| `husako-helm/Cargo.toml` | add tokio |
| `husako-helm/src/lib.rs` | async resolve_all + JoinSet |
| `husako-helm/src/registry.rs` | async reqwest |
| `husako-helm/src/artifacthub.rs` | async reqwest |
| `husako-helm/src/git.rs` | tokio::process::Command |
| `husako-helm/src/**` (inline tests) | #[tokio::test] for async test fns |
| `husako-core/Cargo.toml` | add tokio |
| `husako-core/src/lib.rs` | async generate() |
| `husako-core/src/schema_source.rs` | async resolve_all + JoinSet |
| `husako-core/src/plugin.rs` | async install_plugins |
| `husako-core/src/**` (inline tests) | #[tokio::test] for tests calling generate() |
| `husako-cli/Cargo.toml` | add tokio |
| `husako-cli/src/main.rs` | #[tokio::main], .await on generate |
