# Plan: Migrate `scripts/e2e.sh` to Rust test files in `crates/husako-cli/tests/`

## Context

`scripts/e2e.sh` is an 828-line bash script covering 7 test scenarios (A–G) for the husako CLI.
Every time a new scenario or sub-test is added, subtle OS-specific issues surface (e.g. macOS `/tmp` symlink mismatch in Scenario G, `set -e` side effects, bash version differences) that are invisible to me until the script actually runs — at which point I've already produced incorrect code.

The goal is to replace the shell script with `assert_cmd`-based Rust test files where:
- I can verify structural correctness at compile time
- I can run `cargo test` and see real test output
- Cross-platform issues (temp dirs, path canonicalization) are handled by std/tempfile
- Local (Scenario G) and network (Scenarios A–F) tests are cleanly separated by `#[ignore]`

## Approach: `crates/husako-cli/tests/` 에 시나리오별 파일 추가

- 기존 `husako-cli/tests/`에 시나리오별 파일 추가 (새 crate 불필요)
- `CARGO_BIN_EXE_husako`가 자동 설정됨 — 기존 integration.rs와 완전히 동일한 패턴
- 네트워크 필요 Scenarios A–F: `#[ignore]` 어노테이션 (CI에서 `--include-ignored`로 실행)
- Scenario G: `#[ignore]` 없음, 로컬에서 항상 실행
- `tempfile::TempDir` replaces `mktemp` — no symlink issues on macOS
- `serde_yaml_ng` (already in workspace) replaces `ruby -e "require 'psych'"`
- `kubeconform` invoked via `std::process::Command`; skips gracefully if not installed

## File Structure

```
crates/husako-cli/tests/          ← 기존 tests/ 디렉터리에 추가
├── integration.rs                ← 기존 (변경 없음)
├── generate_output.rs            ← 기존 (변경 없음)
├── real_spec_e2e.rs              ← 기존 (변경 없음)
├── e2e_helpers.rs                ← NEW: 공통 헬퍼 (husako_at, assert_contains 등)
├── e2e_a.rs                      ← NEW: Static k8s + local Helm [#[ignore]]
├── e2e_b.rs                      ← NEW: Chart sources: artifacthub/registry/git [#[ignore]]
├── e2e_c.rs                      ← NEW: Resource sources: file/git CRDs [#[ignore]]
├── e2e_d.rs                      ← NEW: Version management (husako update) [#[ignore]]
├── e2e_e.rs                      ← NEW: Plugin system + husako clean [#[ignore]]
├── e2e_f.rs                      ← NEW: OCI chart source [#[ignore]]
└── e2e_g.rs                      ← NEW: husako test command [항상 실행]
```

파일 구조는 기능/시나리오 기준, 네트워크 gate는 `#[ignore]` 어노테이션이 담당.

## Cargo.toml 변경 없음

`crates/husako-cli/Cargo.toml`은 이미 필요한 dev-deps를 모두 가지고 있습니다:
- `assert_cmd.workspace = true` ✓
- `tempfile.workspace = true` ✓
- `predicates = "3"` ✓

`serde_yaml_ng`만 dev-deps에 추가 필요:
```toml
# crates/husako-cli/Cargo.toml [dev-dependencies]에 추가
serde_yaml_ng.workspace = true
```

`CARGO_BIN_EXE_husako`가 자동으로 설정됨 — 기존 integration.rs와 동일한 패턴.

## `tests/e2e_helpers.rs` — Shell-to-Rust Helper Mapping

| Shell helper | Rust equivalent |
|---|---|
| `husako_at(dir)` | `assert_cmd::Command::cargo_bin("husako").unwrap(); cmd.current_dir(dir)` |
| `assert_contains(desc, pattern, output)` | `assert!(content.contains(pattern), ...)` with `#[track_caller]` |
| `assert_not_contains(...)` | `assert!(!content.contains(pattern), ...)` |
| `assert_file(path)` | `assert!(path.is_file(), ...)` |
| `assert_no_dir(path)` | `assert!(!path.is_dir(), ...)` |
| `assert_valid_yaml(yaml, desc)` | `serde_yaml_ng::from_str::<Value>(yaml).unwrap_or_else(\|e\| panic!(...))` |
| `assert_toml_field(dir, field, val, desc)` | read `husako.toml`, check line contains both substrings |
| `assert_toml_key_absent(dir, key)` | read `husako.toml`, assert no line starts with `key =` |
| `assert_k8s_valid(yaml, desc)` | pipe to `kubeconform -strict` via `Command`; skip if not in PATH |
| `assert_dts_exports(path, symbol)` | read file, assert contains `export` and `symbol` |
| `init_project(dir, toml_content)` | `fs::write("husako.toml", ...)` + copy `tsconfig.json` from fixtures |
| `write_configmap(path)` | `fs::write(path, configmap_ts_template)` |
| `mktemp -d` | `tempfile::TempDir::new()` OR `Builder::new().tempdir_in(&e2e_dir)` for Scenario G |
| `e2e_fixtures_dir` | `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/e2e").canonicalize()` |

Scenario G specifically: `tempfile::Builder::new().prefix("tmp.").tempdir_in(&e2e_fixtures_dir())` — keeps tempdir inside `test/e2e/` to avoid macOS symlink issue.

## Test Organization

A–F: 각 파일은 시나리오 전체를 **단일 `#[test]` 함수**로 표현. 상태가 sequential하게 흘러야 하는 시나리오(B: gen → remove → check)는 하나의 함수 안에서 처리.

```rust
// e2e_b.rs
#[test]
#[ignore]  // CI only: cargo test -p husako -- --include-ignored
fn scenario_b_chart_sources() {
    let dir = tempfile::TempDir::new().unwrap();
    // B1: artifacthub
    init_project(dir.path(), artifacthub_toml());
    husako_at(dir.path()).args(["gen"]).assert().success();
    assert_file(&dir.path().join(".husako/types/helm/cert-manager.d.ts"));
    // B2: registry ...
    // B-remove ...
}
```

Scenario G: **4개 독립 `#[test]` 함수** (G1–G4), 각자 TempDir.

## stdout vs stderr

- `husako render` → stdout (YAML)
- `husako list`, `husako info`, `husako debug`, `husako plugin list`, `husako test` → stderr (diagnostics)
- Capture both: `let output = husako_at(dir).args([...]).output().unwrap()`
- Then check `String::from_utf8_lossy(&output.stderr)` or `&output.stdout` as appropriate

## CI Update

**File to edit**: `workflows/check.ts` (gaji source; regenerates `check.yml` via `gaji build`)

Changes to the `e2e` job:
1. Remove the "Build husako binary" step (cargo test handles it)
2. Replace `run: bash scripts/e2e.sh` with `run: cargo test -p husako --tests -- --include-ignored`
3. Keep: `Install kubeconform`, Scenario A cache step, rust-toolchain, rust-cache

After updating `workflows/check.ts`, run `gaji build` to regenerate `check.yml`.

## `.gitignore` Update

Add `test/e2e/tmp.*/` to ignore Scenario G tempdirs (in case of test crash leaving them behind).

## Documentation Updates

Update `CLAUDE.md` Quick Reference Commands:
```bash
# E2E local (Scenario G only, no network)
cargo test -p husako --test e2e_g

# E2E full (all scenarios, requires network + kubeconform)
cargo test -p husako -- --include-ignored
```

Update `.claude/testing.md` testing table to replace `bash scripts/e2e.sh` row.

## Retirement

Delete `scripts/e2e.sh` only after CI green on the new Rust tests.

## Implementation Steps

1. `crates/husako-cli/Cargo.toml`에 `serde_yaml_ng.workspace = true` dev-dep 추가
2. `tests/e2e_helpers.rs` 생성 (모든 헬퍼 함수)
3. `tests/e2e_g.rs` 생성 (G1–G4, `#[ignore]` 없음)
4. **검증**: `cargo test -p husako --test e2e_g` → 4개 통과
5. `tests/e2e_a.rs` ~ `tests/e2e_f.rs` 생성 (모두 `#[ignore]`)
6. **검증 로컬**: `cargo test -p husako` → A–F ignored, G passes
7. `workflows/check.ts` 업데이트 + `gaji build`
8. `.gitignore` 업데이트 (`test/e2e/tmp.*/`)
9. `CLAUDE.md`, `.claude/testing.md` 업데이트
10. CI green 확인 후 `scripts/e2e.sh` 삭제

## Verification

```bash
# Local: only Scenario G runs (no network needed)
cargo test -p husako --test e2e_g
# Expected: 4 tests pass (g1_*, g2_*, g3_*, g4_*)

# Local: all tests (A-F should show as "ignored")
cargo test -p husako
# Expected: A-F ignored, G passes

# Full e2e (requires network + kubeconform):
cargo test -p husako -- --include-ignored
# Expected: all scenarios pass

# Clippy clean:
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Critical Files

- `scripts/e2e.sh` — 7개 시나리오 번역 원본
- `crates/husako-cli/tests/integration.rs` — `husako_at()` 패턴 참조
- `crates/husako-cli/Cargo.toml` — dev-dep 추가 대상
- `Cargo.toml` — workspace deps (`serde_yaml_ng`, `assert_cmd`, `tempfile`)
- `.github/workflows/check.yml` — e2e job 업데이트 (edit `workflows/check.ts`)
- `test/e2e/` — Scenario A fixture 디렉터리, Scenario G tempdir parent
