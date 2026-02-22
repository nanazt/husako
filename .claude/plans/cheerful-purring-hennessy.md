# M24: CLI Interface Design & Visual Consistency — Execution Plan

## Context

M24 establishes a consistent visual language across all CLI prompts in husako. The design uses:
- No prompt prefix (no `?`)
- Colon suffix on all prompts (`Prompt:`)
- Cyan+bold `>` for active items
- `✔ Prompt: value` after-selection confirmation
- Dim placeholder text for default values
- Consistent marks: `✔` (green+bold), `✘` (red+bold), `warning:` (yellow+bold)

Work is in the git worktree at `.worktrees/m24-cli-design` on branch `feature/m24-cli-design`.

## Current State (Batch 1 + partial Batch 2 complete)

### Completed
- Task 7: `.claude/cli-design.md` — design doc ✅
- Task 8: `crates/husako-cli/src/theme.rs` — `HusakoTheme` implementing `dialoguer::theme::Theme` ✅
- Task 9: `crates/husako-cli/src/style.rs` — bold marks, new `dim()`, `bold()`, `warning_prefix()` helpers ✅
- Task 10: `crates/husako-cli/src/text_input.rs` — dim placeholder raw-key input widget ✅
- Task 11: `crates/husako-cli/src/interactive.rs` — theme applied to all prompts, **but BROKEN** (10 compile errors)

### Remaining Tasks

#### Fix Task 11: Repair interactive.rs (10 compile errors)

The `sed` command that was meant to revert `items(&[...])` → `items([...])` instead corrupted the file with backslash escapes. Five lines need fixing:

| Line | Current (broken) | Should be |
|------|-----------------|-----------|
| ~10  | `\.items(\["Resource", "Chart"])` | `.items(["Resource", "Chart"])` |
| ~26  | `\.items(\["release", "cluster", "git", "file"])` | `.items(["release", "cluster", "git", "file"])` |
| ~90  | `\.items(\["registry", "artifacthub", "git", "file"])` | `.items(["registry", "artifacthub", "git", "file"])` |
| ~175 | `\.items(\["Search ArtifactHub", "Enter manually"])` | `.items(["Search ArtifactHub", "Enter manually"])` |
| ~431 | `\.items(\["Cache", "Types", "Both"])` | `.items(["Cache", "Types", "Both"])` |

Fix: Read `interactive.rs`, find each corrupted `.items()` call, and Edit them back to correct syntax. Then run `cargo build -p husako 2>&1 | head -30` to verify.

Note: `dialoguer` accepts `impl IntoIterator<Item = impl ToString>` — both `["a", "b"]` (array) and `&["a", "b"]` (slice) work; use `["a", "b"]` without `&` (clippy prefers non-borrowed form).

#### Task 12: search_select.rs — ALREADY COMPLETE

`search_select.rs` was checked and is complete. The infinite scroll behavior, `>` prefix, and loading indicator are already implemented correctly. No changes needed.

#### Task 13: main.rs — Bold headers, dim secondary info

Key changes to `main.rs`:
- Section headers in list/info output: use `style::bold()` for section titles
- Secondary/dim information: use `style::dim()` for less important text
- `mod theme` and `mod text_input` already declared (lines 1-6)

Specific locations to update in `main.rs`:
1. `husako list` output — bold dependency names/types, dim source URLs
2. `husako info` output — bold section headers ("Resources:", "Charts:", "Cluster:")
3. `husako debug` output — bold check names, dim descriptions
4. `husako outdated` output — bold dep names, dim current version

## Files to Modify

- `crates/husako-cli/src/interactive.rs` — fix 5 corrupted `.items()` lines
- `crates/husako-cli/src/main.rs` — add bold/dim styling to list/info/debug/outdated output

## Files Already Complete (no changes needed)

- `crates/husako-cli/src/theme.rs`
- `crates/husako-cli/src/style.rs`
- `crates/husako-cli/src/text_input.rs`
- `crates/husako-cli/src/search_select.rs`
- `.claude/cli-design.md`

## Verification

```bash
# In .worktrees/m24-cli-design/
cargo build -p husako 2>&1 | head -30   # must be 0 errors
cargo clippy -p husako --all-targets -- -D warnings 2>&1 | head -30
cargo test -p husako 2>&1 | tail -20    # all tests pass
```
