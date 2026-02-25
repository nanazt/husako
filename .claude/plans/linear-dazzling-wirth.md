# Plan: Add rquickjs Async Features + Migrate Runtime to AsyncRuntime

## Context

The previous async migration (already committed on `feat/async-tokio`) made
`husako-openapi`, `husako-helm`, and the orchestration layer in `husako-core`
(schema_source, version_check, plugin) async. The CLI entry point already uses
`#[tokio::main]`.

However `husako-runtime-qjs` still uses the synchronous `Runtime`/`Context`
API, and `husako_core::render()` and `run_tests()` are still sync — they block
the tokio thread while QuickJS executes. This is the last layer to migrate.

rquickjs offers `AsyncRuntime` + `AsyncContext` (behind the `futures` and
`tokio` features) that integrates with tokio's event loop. Additionally,
`macro` (proc macros for Rust/JS interop) and `phf` (faster builtin module
lookup) are useful non-breaking additions.

---

## Task 1 — Workspace `Cargo.toml`

**File:** `Cargo.toml` (line 21)

Change:
```toml
rquickjs = { version = "0.11", features = ["bindgen", "loader"] }
```
To:
```toml
rquickjs = { version = "0.11", features = ["bindgen", "loader", "futures", "parallel", "macro", "phf"] }
```

- `futures` — enables `AsyncRuntime`, `AsyncContext`, `Promise::into_future()`
- `parallel` — makes `set_interrupt_handler` require `Send + 'static` (needed with `Arc<AtomicBool>`); there is **no** `"tokio"` feature in rquickjs 0.11
- `macro` — proc macros (`#[derive(JsClass)]`, `#[methods]`) for future JS/Rust bindings
- `phf` — perfect hash function lookup for builtin modules (minor perf win, no API change)

---

## Task 2 — `husako-runtime-qjs/Cargo.toml`

**File:** `crates/husako-runtime-qjs/Cargo.toml`

Add tokio dependency:
```toml
tokio.workspace = true
```

---

## Task 3 — Migrate `husako-runtime-qjs/src/lib.rs`

**File:** `crates/husako-runtime-qjs/src/lib.rs`

### Import changes
```rust
// Replace:
use rquickjs::{Context, Ctx, Error, Function, Module, Promise, Runtime, Value};
// With:
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use rquickjs::{AsyncContext, AsyncRuntime, Ctx, Error, Function, Module, Promise, Value};
// Remove: use std::rc::Rc; use std::cell::{Cell, RefCell};
```

### `execute()` signature
```rust
pub async fn execute(
    js_source: &str,
    options: &ExecuteOptions,
) -> Result<serde_json::Value, RuntimeError>
```

### Runtime init (both `execute` and `execute_tests`)
```rust
let rt = AsyncRuntime::new().map_err(|e| RuntimeError::Init(e.to_string()))?;
tokio::spawn(rt.drive()); // drives the QuickJS job queue via tokio
```

> Note: `rt.spawn_executor(rquickjs::Tokio)` does not exist in rquickjs 0.11.
> The correct API is `rt.drive()` which returns a `DriveFuture`; spawn it with `tokio::spawn`.

### Timeout: `Arc<AtomicBool>` instead of `Rc<Cell<bool>>`
`AsyncRuntime::set_interrupt_handler` requires `Send + 'static`. Replace:
```rust
let timed_out = Rc::new(Cell::new(false));
// closure uses flag.set(true)
```
With:
```rust
let timed_out = Arc::new(AtomicBool::new(false));
// closure uses flag.store(true, Ordering::Relaxed)
// check: timed_out.load(Ordering::Relaxed)
```

### `AsyncRuntime` setup methods are all `async fn` — need `.await`
```rust
// set_interrupt_handler, set_memory_limit, set_loader are all async on AsyncRuntime:
rt.set_interrupt_handler(Some(Box::new(move || { ... }))).await;
rt.set_memory_limit(mb * 1024 * 1024).await;
rt.set_loader(resolver, loader).await;
```

### Context creation
```rust
// Replace:
let ctx = Context::full(&rt).map_err(|e| RuntimeError::Init(e.to_string()))?;
// With:
let ctx = AsyncContext::full(&rt).await.map_err(|e| RuntimeError::Init(e.to_string()))?;
```

### Captured state: `Arc<Mutex<>>` instead of `Rc<RefCell<>>`
`async_with` callbacks must be `Send`. Replace shared captures:
```rust
// Replace Rc<RefCell<Option<serde_json::Value>>> with:
let result: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
let call_count: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
let capture_error: Arc<Mutex<Option<RuntimeError>>> = Arc::new(Mutex::new(None));
```
All `.borrow_mut()` → `.lock().unwrap()`.

### `ctx.with()` → `ctx.async_with()`
```rust
let eval_result: Result<(), RuntimeError> = ctx.async_with(|ctx| async move {
    // ... same body ...
    let promise = Module::evaluate(ctx.clone(), "main", js_source)
        .map_err(|e| execution_error(&ctx, e))?;
    promise.into_future::<()>().await
        .map_err(|e| execution_error(&ctx, e))?;
    Ok(())
}).await;
```

For `execute_tests`, the two-phase pattern (evaluate module, then call `__husako_run_all_tests`) similarly becomes:
```rust
ctx.async_with(|ctx| async move {
    let promise = Module::evaluate(ctx.clone(), "main", js_source)
        .map_err(|e| execution_error(&ctx, e))?;
    promise.into_future::<()>().await
        .map_err(|e| execution_error(&ctx, e))?;

    let run_fn: Function = ctx.globals().get("__husako_run_all_tests").map_err(|_| {
        RuntimeError::Execution("no tests found — did you import from 'husako/test'?".into())
    })?;
    let promise: rquickjs::Value = run_fn.call(()).map_err(|e| execution_error(&ctx, e))?;
    Promise::from_value(promise)
        .map_err(|e| RuntimeError::Execution(e.to_string()))?
        .into_future::<String>().await
        .map_err(|e| execution_error(&ctx, e))
}).await
```

### `execute_tests()` signature
```rust
pub async fn execute_tests(
    js_source: &str,
    options: &ExecuteTestsOptions,
) -> Result<Vec<TestCaseResult>, RuntimeError>
```

---

### Note on `.finish()` removal
Every `promise.finish::<T>()` call must be replaced with `promise.into_future::<T>().await`.
`finish()` is a sync blocking poll that belongs to the sync `Promise` type and does not exist on the async version. Both in `execute()` (module evaluation promise) and `execute_tests()` (module + test runner promise).

### Test functions need `#[tokio::test]`
All 20+ tests in `crates/husako-runtime-qjs/src/lib.rs` currently use `#[test]`.
After `execute()` and `execute_tests()` become `async fn`, every test that calls them must become:
```rust
#[tokio::test]
async fn test_name() { ... execute(...).await ... }
```

---

## Task 4 — `husako-core/src/lib.rs`: propagate async

**File:** `crates/husako-core/src/lib.rs`

Three call sites:
- Line 115: `husako_runtime_qjs::execute(&js, &exec_options)?` — in `render()`
- Line 1625: `husako_runtime_qjs::execute(&js, &exec_options)?` — in `validate_file()`
- Line 1825: `husako_runtime_qjs::execute_tests(&js, &exec_options)?` — in `run_test_file()`

Each becomes `.await?`. The enclosing functions need `async fn`:
- `pub fn render(...)` → `pub async fn render(...)`
- `pub fn validate_file(...)` → `pub async fn validate_file(...)` ← **do not miss this one**
- `fn run_test_file(...)` (private) → `async fn run_test_file(...)`
- `pub fn run_tests(...)` → `pub async fn run_tests(...)` (calls `run_test_file`, must also be async)

---

## Task 5 — `husako-cli/src/main.rs`: add `.await` on all async core calls

**File:** `crates/husako-cli/src/main.rs`

- Line 312: `husako_core::render(...)` → `husako_core::render(...).await`
- Line 1157: `husako_core::validate_file(...)` → `husako_core::validate_file(...).await` ← **new**
- Line 1205: `husako_core::run_tests(...)` → `husako_core::run_tests(...).await`

(CLI already has `#[tokio::main]` from the previous migration.)

---

## Task 6 — `husako-bench`: fix sync `b.iter()` calls for async functions

Criterion's `b.iter()` closure is synchronous. Two bench files are affected.

**File A:** `crates/husako-bench/benches/execute.rs` (lines 39, 48 — calls `execute()`)

**File B:** `crates/husako-bench/benches/render.rs` (lines 27, 34 — calls `husako_core::render()`)

Fix pattern for both: create a tokio runtime once outside `b.iter()`, use `block_on` inside:

```rust
let rt = tokio::runtime::Runtime::new().unwrap();
group.bench_function("...", |b| {
    b.iter(|| rt.block_on(async_fn_call(...)).unwrap());
});
```

---

## Task 7 — `husako-core/Cargo.toml`: add tokio dev-dependency + convert core tests

**File:** `crates/husako-core/Cargo.toml`

Add to `[dev-dependencies]`:
```toml
tokio = { workspace = true, features = ["rt", "macros"] }
```

**File:** `crates/husako-core/src/lib.rs`

Inline tests that call `render()` or `validate_file()` must change from `#[test]` to `#[tokio::test]` and `fn` to `async fn`. Only tests that directly invoke these two async functions need updating — helper/setup tests that do not call them remain `#[test]`.

---

## Verification

```bash
cd .worktrees/feat-async-tokio

# 1. Format
cargo fmt --all

# 2. Build runtime crate first to catch API errors early
cargo build -p husako-runtime-qjs

# 3. Build bench crate to verify block_on fix compiles
cargo build -p husako-bench

# 4. Full workspace lint
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 5. Unit tests for the runtime (now all #[tokio::test])
cargo test -p husako-runtime-qjs

# 6. Full workspace tests
cargo test --workspace --all-features

# 7. E2E (no network required)
cargo test -p husako --test e2e_g
```
