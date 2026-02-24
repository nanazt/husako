# husako test — JS/TS Test Runner

## Context

Users write TypeScript code using husako builders (resource factory helpers, value transformers, etc.)
and currently have no way to unit-test that code in isolation. husako has Rust `#[cfg(test)]` and
E2E bash scripts, but nothing for the TypeScript layer. This plan adds a `husako test` command that
discovers `*.test.ts` / `*.spec.ts` files and runs them through the same QuickJS runtime with a
Jest-like assertion API exposed via a new `"husako/test"` builtin module.

No changes to production behavior of `render`, `generate`, or `validate`.

## Example usage

```typescript
// helpers.test.ts
import { test, expect, describe } from "husako/test";
import { makeDeployment } from "./helpers";

describe("Deployment factory", () => {
  test("sets replicas", () => {
    const json = makeDeployment("my-app", 3)._render();
    expect(json.spec.replicas).toBe(3);
  });
  test("sets name", () => {
    const json = makeDeployment("my-app", 3)._render();
    expect(json.metadata.name).toBe("my-app");
  });
});
```

```bash
husako test                      # discover *.test.ts + *.spec.ts under project root
husako test helpers.test.ts      # explicit file(s)
```

## New files

### `crates/husako-sdk/src/js/husako_test.js`

Jest-like test API as a plain ES module. Key design:

- `test(name, fn)` / `it(name, fn)` — register a test case (sync or async fn)
- `describe(name, fn)` — synchronously groups tests; prefixes test names with `"suite > "`
  by pushing/popping a `_suiteStack` array
- `expect(value)` — returns an `Expect` instance; `expect(v).not.toBe(x)` via a negation flag

`Expect` methods: `.toBe()`, `.toEqual()` (JSON.stringify comparison), `.toBeDefined()`,
`.toBeNull()`, `.toBeUndefined()`, `.toBeTruthy()`, `.toBeFalsy()`,
`.toBeGreaterThan/LessThan/OrEqual()`, `.toContain()`, `.toHaveProperty()`,
`.toHaveLength()`, `.toMatch()`, `.toThrow()`

Runner: `globalThis.__husako_run_all_tests` is set as an `async` function that iterates
`_tests`, `await`s each `fn()`, catches errors, and returns `JSON.stringify(results)`.
Rust calls this after module evaluation.

Result JSON shape:
```json
[{ "name": "suite > test name", "passed": true, "error": null }, ...]
```

### `crates/husako-sdk/src/dts/husako_test.d.ts`

TypeScript declarations for the `"husako/test"` module — `Expect` interface + function exports.
Written to `.husako/types/husako/test.d.ts` during `husako generate` so IDEs pick it up.

## Modified files

### `crates/husako-sdk/src/lib.rs`

Add two constants after the existing ones:
```rust
pub const HUSAKO_TEST_MODULE: &str = include_str!("js/husako_test.js");
pub const HUSAKO_TEST_DTS: &str = include_str!("dts/husako_test.d.ts");
```

### `crates/husako-runtime-qjs/src/lib.rs`

Add public types:
```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TestCaseResult { pub name: String, pub passed: bool, pub error: Option<String> }

pub type ExecuteTestsOptions = ExecuteOptions;  // same fields, separate name for clarity
```

Add `execute_tests(js_source: &str, options: &ExecuteTestsOptions) -> Result<Vec<TestCaseResult>, RuntimeError>`:
- Same runtime setup as `execute()` (timeout, heap limit, module resolver chain)
- Additionally registers `"husako/test"` in both `BuiltinResolver` and `BuiltinLoader`
- Does **NOT** register `__husako_build`, does **NOT** check build call count
- All JS interaction happens inside a single `ctx.with()` closure (same pattern as `execute()` —
  `promise.finish::<()>()` must be inside `ctx.with()`, and `__husako_run_all_tests()` is called
  in the same closure immediately after):
  ```rust
  let json_str: String = ctx.with(|ctx| {
      let promise = Module::evaluate(ctx.clone(), "main", js_source)
          .map_err(|e| execution_error(&ctx, e))?;
      promise.finish::<()>().map_err(|e| execution_error(&ctx, e))?;

      // Module evaluated — now run the registered tests
      let run_fn: Function = ctx.globals()
          .get("__husako_run_all_tests")
          .map_err(|_| RuntimeError::Execution(
              "no tests found — did you import from 'husako/test'?".into()
          ))?;
      let promise: Value = run_fn.call(()).map_err(|e| execution_error(&ctx, e))?;
      Promise::from_value(promise)
          .map_err(|e| RuntimeError::Execution(e.to_string()))?
          .finish::<String>()
          .map_err(|e| execution_error(&ctx, e))
  })?;
  serde_json::from_str(&json_str)
  ```

Add unit tests: `execute_tests_all_pass`, `execute_tests_failure_captured`,
`execute_tests_describe_prefixes_name`, `execute_tests_not_negation`,
`execute_tests_no_build_call_required`, `execute_tests_async_test_function`

### `crates/husako-core/src/lib.rs`

Add public types:
```rust
pub struct TestOptions {
    pub project_root: PathBuf,
    pub files: Vec<PathBuf>,   // empty = discover
    pub timeout_ms: Option<u64>,
    pub max_heap_mb: Option<usize>,
    pub allow_outside_root: bool,
}

pub struct TestResult {
    pub file: PathBuf,          // relative to project_root
    pub cases: Vec<husako_runtime_qjs::TestCaseResult>,
}

pub use husako_runtime_qjs::TestCaseResult;
```

Add `discover_test_files(root: &Path) -> Vec<PathBuf>`:
- Recursive `std::fs::read_dir` (no new deps)
- Collect `*.test.ts` + `*.spec.ts`
- Skip dirs: `.husako`, `node_modules`, any dir starting with `.`

Add `run_test_file(source, filename, options) -> Result<Vec<TestCaseResult>, HusakoError>`:
- Compile TS→JS via `husako_compile_oxc::compile()`
- Derive `generated_types_dir` as `options.project_root.join(".husako/types").canonicalize().ok()`
- Load `plugin_modules` via the same `load_plugin_modules(&options.project_root)` helper that `render()` uses
  (reads from `.husako/plugins/` — already installed by `husako generate`)
- Build `ExecuteOptions` and call `husako_runtime_qjs::execute_tests()`

Add `run_tests(options: &TestOptions) -> Result<Vec<TestResult>, HusakoError>`:
- Determine files: `options.files` if non-empty, else `discover_test_files()`
- For each file, call `run_test_file()`; on compile/runtime error wrap as a synthetic
  `TestCaseResult { passed: false, error: Some(e.to_string()) }` so the test summary stays accurate
- Return `Vec<TestResult>`

In **`generate()`**: after writing `husako/_base.d.ts`, also write:
```
.husako/types/husako/test.d.ts  ←  husako_sdk::HUSAKO_TEST_DTS
```

In **`write_tsconfig()`** (wherever `"husako/_base"` path is added): also add:
```json
"husako/test": [".husako/types/husako/test.d.ts"]
```

Add unit tests: `discover_test_files_finds_test_ts`,
`discover_test_files_excludes_husako_dir`, `discover_test_files_excludes_node_modules`

### `crates/husako-cli/src/main.rs`

Add to `Commands` enum:
```rust
/// Run test files
Test {
    #[arg(value_name = "FILE")]
    files: Vec<PathBuf>,
    #[arg(long)]
    timeout_ms: Option<u64>,
    #[arg(long)]
    max_heap_mb: Option<usize>,
}
```

Handler in `match cli.command`:
1. Resolve explicit file paths relative to cwd
2. Build `TestOptions`, call `husako_core::run_tests()`; on `Err` → `eprintln!` + `exit_code(&e)`
3. If empty → `"No test files found"` + `ExitCode::SUCCESS`
4. For each file: print filename header (`eprintln!`), then per case: `✔ name` / `✘ name` + `dim(error)`
5. Print summary: `N passed, M failed`
6. Test failures are handled directly: `ExitCode::from(1u8)` if `total_failed > 0`, else `SUCCESS`.
   This bypasses `exit_code()` — no new `HusakoError` variant needed.

### `scripts/e2e.sh` — Scenario G: husako test

Add `scenario_g()` after the existing scenario F. Uses a temp dir. Tests:

1. **G1**: passing tests (all green) → exit 0
   - Write `calc.ts` helper that exports an `add(a, b)` function using `_ResourceBuilder` pattern (or just plain TS arithmetic)
   - Write `calc.test.ts` with `describe` + `test` + `expect().toBe()`, `expect().toEqual()`
   - Run `husako test calc.test.ts` → assert exit 0, output contains "passed"

2. **G2**: failing test → exit 1
   - Write `fail.test.ts` with one deliberately failing assertion
   - Run `husako test fail.test.ts` → assert exit 1, output contains "failed"

3. **G3**: auto-discovery
   - Drop `*.test.ts` in subdir, run `husako test` (no args) → assert both files found and run

4. **G4**: plugin testing
   - Use `husako.toml` with `[plugins] myplugin = { source = "path", path = "./myplugin" }`
   - Write minimal `myplugin/plugin.toml` + `myplugin/index.js` exporting a helper
   - Write `plugin.test.ts` that `import`s the helper by plugin name: `import { greet } from "myplugin"`
   - Run `husako generate --skip-k8s` then `husako test plugin.test.ts` → assert pass

Append `scenario_g` to the call list at the bottom of the script.

Update `.claude/testing.md`:
- Add `husako test` to the "verify side effects" table
- Add `scenario_g` row to the E2E source kind coverage table
- Document test file naming convention (`*.test.ts`, `*.spec.ts`)

### `.worktrees/docs-site/docs/reference/cli.md`

Add `## husako test` section (following the existing per-command H2 pattern):

```markdown
## husako test

Run TypeScript test files using the built-in `"husako/test"` assertion module.

```
husako test [FILE...] [options]
```

| Flag | Description |
|------|-------------|
| `--timeout-ms <ms>` | Execution timeout per file in milliseconds |
| `--max-heap-mb <mb>` | Maximum heap memory per file in megabytes |

With no FILE arguments, husako discovers all `*.test.ts` and `*.spec.ts` files under the
project root (excluding `.husako/` and `node_modules/`).

Test files import from `"husako/test"`:

```typescript
import { test, describe, expect } from "husako/test";
```

Exit code is 0 if all tests pass, 1 if any test fails.

See [Writing Tests](/guide/testing) for full examples and the assertion API reference.

---
```

Also create `.worktrees/docs-site/docs/guide/testing.md` — new guide page covering:
- When to write husako tests (unit-testing resource factory helpers)
- `test()`, `describe()`, `expect()` API with examples
- Full `Expect` method reference table
- How to test with k8s builders (using `_render()`)
- Plugin testing workflow (`husako.toml` path source)
- Running tests (discovery vs explicit files)

Update `.worktrees/docs-site/docs/.vitepress/config.ts` (or equivalent sidebar config):
- Add `{ text: 'Testing', link: '/guide/testing' }` to the Guide sidebar section

## Implementation order

1. `husako_test.js` + `husako_test.d.ts` (new JS/DTS files)
2. `husako-sdk/src/lib.rs` (two new constants)
3. `husako-runtime-qjs/src/lib.rs` (`TestCaseResult`, `execute_tests()`, unit tests)
4. `husako-core/src/lib.rs` (`TestOptions`, `TestResult`, `discover_test_files()`, `run_test_file()`, `run_tests()`, generate/tsconfig changes, unit tests)
5. `husako-cli/src/main.rs` (`Test` command + handler)
6. `scripts/e2e.sh` — Scenario G (4 substeps + append to call list)
7. `.claude/testing.md` — document `husako test` patterns
8. `.worktrees/docs-site/docs/reference/cli.md` — add `## husako test`
9. `.worktrees/docs-site/docs/guide/testing.md` — new guide page

## Verification

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features

# Smoke test
mkdir /tmp/ht-smoke && cd /tmp/ht-smoke
husako new . --template simple
husako generate --skip-k8s
cat > hello.test.ts << 'EOF'
import { test, expect, describe } from "husako/test";
describe("math", () => {
  test("addition", () => { expect(1 + 1).toBe(2); });
  test("failure", () => { expect(1).toBe(999); });
});
EOF
husako test hello.test.ts
# ✔ math > addition
# ✘ math > failure
#   Expected 1 to be 999
# ✘ 1 passed, 1 failed  (exit 1)

# E2E
cargo build --bin husako
HUSAKO_BIN=./target/debug/husako bash scripts/e2e.sh
# Scenario G should pass all 4 substeps
```

## Scope exclusions (deferred)

- `beforeEach` / `afterEach` hooks
- Per-test timeout
- Watch mode (`husako test --watch`)
- Coverage reporting
- `husako/test` available in non-test render files (intentionally blocked)
