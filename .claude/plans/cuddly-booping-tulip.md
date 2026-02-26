# Plan: Enforce cli-design rules + design improvements

## Context

Four rule-compliance fixes and three design improvements, agreed in discussion:

- **Compliance**: bold headers in `outdated`, `cross_mark`→`error_prefix` for plugin not found,
  `plugin add` source detail, `debug` summary line
- **Design**: `outdated` filters to only outdated items, `husako remove` drops confirmation
- All changes reflected in `cli-design.md`

---

## Code changes — `crates/husako-cli/src/main.rs`

### 1. `husako outdated` — bold headers + filter to only outdated items

Replace lines 785–805 entirely:

```rust
// Collect only outdated entries
let outdated: Vec<_> = entries.iter().filter(|e| !e.up_to_date).collect();

if outdated.is_empty() {
    eprintln!("{} All dependencies are up to date", style::check_mark());
} else {
    eprintln!(
        "{:<16} {:<10} {:<12} {:<10} {:<10}",
        style::bold("Name"),
        style::bold("Kind"),
        style::bold("Source"),
        style::bold("Current"),
        style::bold("Latest"),
    );
    for entry in outdated {
        let latest = entry.latest.as_deref().unwrap_or("?");
        eprintln!(
            "{:<16} {:<10} {:<12} {:<10} {:<10} {}",
            style::dep_name(&entry.name),
            entry.kind,
            entry.source_type,
            entry.current,
            latest,
            style::arrow_mark(),
        );
    }
}
```

Result:
```
Name            Kind      Source      Current   Latest
cert-manager    resource  git         v1.16.0   v1.17.2  →
postgresql      chart     registry    16.3.0    16.4.0   →
```
or, when everything is current:
```
✔ All dependencies are up to date
```

### 2. `husako remove` — remove confirmation block (lines 740–750)

Delete:
```rust
// Confirm removal only in CLI mode (not interactive, user already chose)
if from_cli && !cli.yes {
    match interactive::confirm(&format!("Remove '{dep_name}'?")) {
        Ok(true) => {}
        Ok(false) => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{} {e}", style::error_prefix());
            return ExitCode::from(1);
        }
    }
}
```

The interactive branch (no name arg) already uses `prompt_remove()` which is a selection,
not a confirmation — that stays. Only the confirm-after-CLI-name block is removed.

Also simplify the binding at line 711 — `from_cli` is only used in the now-deleted block:

```rust
// Before:
let (dep_name, from_cli) = if let Some(n) = name {
    (n, true)
} else { ... Ok(n) => (n, false), ... };

// After:
let dep_name = if let Some(n) = name {
    n
} else { ... Ok(n) => n, ... };
```

### 3. `husako debug` — add summary line (after line 1040)

After the `for suggestion in &report.suggestions` block, add:

```rust
let issue_count = [
    !matches!(report.config_ok, Some(true)),
    !report.types_exist,
    !report.tsconfig_ok || !report.tsconfig_has_paths,
    report.stale,
]
.iter()
.filter(|&&b| b)
.count();

eprintln!();
if issue_count == 0 {
    eprintln!("{} All checks passed", style::check_mark());
} else {
    eprintln!(
        "{} {} issue{} found",
        style::cross_mark(),
        issue_count,
        if issue_count == 1 { "" } else { "s" }
    );
}
```

Result:
```
✔ husako.toml found and valid
✔ .husako/types/ exists (42 type files)
✘ tsconfig.json is missing husako path mappings
  → run husako gen to regenerate

✘ 1 issue found
```

### 4. `cross_mark` → `error_prefix` for plugin not found (line 1135)

```rust
// Before:
eprintln!("{} Plugin '{}' not found", style::cross_mark(), name);
// After:
eprintln!("{} Plugin '{}' not found", style::error_prefix(), name);
```

### 5. `plugin add` success with dim source detail (lines 1083–1087)

`source` is already in scope as `PluginSource::Git { url }` or `PluginSource::Path { path }`.

```rust
let source_detail = match &source {
    husako_config::PluginSource::Git { url, .. } => url.clone(),
    husako_config::PluginSource::Path { path } => path.clone(),
};
eprintln!(
    "{} Added plugin {} to [plugins]\n  {}",
    style::check_mark(),
    style::dep_name(&name),
    style::dim(&source_detail),
);
```

---

## Test changes

### `crates/husako-cli/tests/e2e_b.rs` — line 173

```rust
// Before:
.args(["-y", "remove", "pg"])
// After:
.args(["remove", "pg"])
```

### `crates/husako-cli/tests/e2e_c.rs` — line 133

```rust
// Before:
.args(["-y", "remove", "cert-manager"])
// After:
.args(["remove", "cert-manager"])
```

---

## Doc changes — `crates/husako-cli/src/main.rs` → `.claude/cli-design.md`

### 1. Add interaction rule to `Interactive Prompts → Rules`

```markdown
- **Minimize interactions** — interactive prompts (selections, confirmations, inputs)
  are a last resort, used only when the information truly cannot be inferred from
  arguments or context. Prefer flags over prompts; prefer direct action over confirmation.
  Example: `husako remove <name>` removes immediately — no confirm needed since the
  operation is reversible and the intent is unambiguous from the argument.
```

### 2. Add "Filter to actionable" rule to `Command Output`

New subsection after `### Empty State`:

```markdown
### Filter to Actionable

Status/check commands show only items that need attention. If all items pass, show a
single plain success line instead of a full table:

```
✔ All dependencies are up to date    ← when nothing is outdated
```

Don't mix passing items into a table alongside failing ones — showing every entry with
a ✔ or ✘ adds noise. Show only the actionable items.
```

### 3. Add "Structured Checks" pattern to `Command Output`

New subsection (after `### Filter to Actionable`):

```markdown
### Structured Checks

Commands that run multiple named checks (e.g., `husako debug`) list each result then
append a blank line and a summary:

```
✔ husako.toml found and valid
✘ tsconfig.json is missing husako path mappings
  → run husako gen

✘ 1 issue found
```

- Individual pass: `check_mark()` + description
- Individual fail: `cross_mark()` + description, then suggestions with `arrow_mark()`
- Summary: blank line, then `check_mark()` "All checks passed" or `cross_mark()` "N issues found"
- `cross_mark()` is correct for the summary even when the command exits 0 — the command
  succeeded; the checks found issues
```

---

## Doc changes — user-facing

### `docs/reference/cli.md` — `husako remove` section

Remove the sentence: `Prompts for confirmation. Use -y / --yes to skip.`

### `.claude/testing.md` — update CLI flag notes

Change:
```
- `husako remove <name>` requires `-y` to skip the confirmation dialog when name is a CLI arg.
```
To:
```
- `husako remove <name>` does not prompt for confirmation — name on CLI removes directly.
```

---

## Critical Files

- `crates/husako-cli/src/main.rs` — 5 code changes
- `crates/husako-cli/tests/e2e_b.rs` — remove `-y`
- `crates/husako-cli/tests/e2e_c.rs` — remove `-y`
- `.claude/cli-design.md` — 3 new rules
- `.worktrees/docs-site/docs/reference/cli.md` — remove confirmation note
- `.claude/testing.md` — update remove flag note

---

## Verification

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p husako --test e2e_g   # local E2E
```

Manual spot-check:
```bash
husako outdated          # only shows outdated; ✔ message when all current
husako remove pg         # no confirmation prompt, removes directly
husako debug             # shows "✔ All checks passed" or "✘ N issues found"
husako plugin remove x   # "error:" not "✘" when not found
husako plugin add ...    # success line has dim source URL/path
```
