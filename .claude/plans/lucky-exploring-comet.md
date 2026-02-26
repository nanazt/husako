# Plan: Remove Step Counter from Progress Output

## Context

The CLI shows `[N/M]` step counters in spinner messages (e.g. `[1/4] Compiling…`, `[2/4] Executing…`). The user finds this noisy and wants it removed.

## Changes

### 1. `crates/husako-cli/src/progress.rs` — main change

- Remove `counter: Arc<AtomicUsize>` and `total: Arc<AtomicUsize>` fields from `IndicatifReporter`.
- Remove the `set_total` override from `impl ProgressReporter for IndicatifReporter` — the trait's default no-op is sufficient.
- In `start_task`, remove the `count`/`total` fetch and the `prefix` format block entirely.
- Remove the `prefix: String` field from `IndicatifTaskHandle` and its doc comment.
- In `set_message`, `set_progress`, `finish_ok`, `finish_err` — remove `self.prefix` interpolation.
- The `IndicatifReporter::new()` constructor simplifies to `Self {}` (or removes the field inits).

### 2. `crates/husako-core/src/progress.rs` — doc comment update

Line 7-8 says: *"Set the total number of steps so the reporter can show `[N/M]` counters."*
Update to remove the `[N/M]` mention since the CLI no longer renders it. The method itself stays (it's a no-op default; callers can still call it harmlessly).

### 3. `set_total()` call sites — leave as-is

`lib.rs` calls `progress.set_total(4)` and `schema_source.rs` calls `progress.set_total(n)`.
These now hit the default no-op and are harmless. Leaving them avoids scope creep.

## Tests — no changes needed

Confirmed by search:
- No test asserts on `[1/1]`, `[2/4]` or any `[N/M]` pattern in output.
- `verbose_produces_stderr_output` (integration.rs:1122) checks for `[compile]`, `[execute]`, `[validate]`, `[emit]` — those come from `eprintln!()` verbose lines in `lib.rs`, **not** from the spinner prefix. They are unaffected.
- `husako-cli/src/progress.rs` inline tests only test `build_suffix()` — no prefix involved.
- `husako-core/src/progress.rs` tests only cover `SilentProgress` — unaffected.

## Docs — no user-facing doc changes

No user-facing documentation mentions the `[N/M]` format. The only update is the internal doc comment on the `set_total` trait method (covered above).

## Verification

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Then smoke-check that spinners no longer show `[1/4]` etc. by running a render manually.
