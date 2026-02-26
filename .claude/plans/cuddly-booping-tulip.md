# Plan: Commit pending changes (plan files)

## Context

The existing `cli-design.md` documents interactive prompts and style helpers thoroughly,
but the **Command Output** section is sparse (3 subsections, no concrete rules). An audit
of `main.rs` shows ~85% compliance with unwritten conventions, plus a few real inconsistencies.

Adding explicit output standards to the doc will:
1. Give future Claude sessions a concrete reference when writing new commands
2. Resolve the genuine inconsistencies (table headers, marker usage)
3. Prevent new inconsistencies from being introduced

**File to modify:** `/Users/syr/Developments/husako/.claude/cli-design.md`

No code changes. No tests. Documentation only.

---

## What the Audit Found

### Already Consistent (just undocumented)
- All output goes to `stderr` (`eprintln!`) except `husako render` → stdout
- Empty state messages are always plain text ("No dependencies configured")
- Dependency names are always `dep_name()` (cyan)
- Blank lines appear before instruction blocks and between major sections

### Real Inconsistency: Table headers
- `list` / `debug`: section headers bold (`style::bold("Resources:")`)
- `outdated`: column headers plain text (no bold)
→ Rule needed: section headers and column headers both bold

### Real Inconsistency: `cross_mark` vs `error_prefix`
- Currently used interchangeably in some places
- Clear semantic split exists in practice:
  - `error_prefix()` = command/operation failed (exit non-zero)
  - `cross_mark()` = check failed or item not present (informational)

### Missing Rules
- When to use each marker (full decision table, including dual role of `arrow_mark`)
- Key-value pair format (info command)
- Instruction block format (new/init "Next steps:")
- Blank line conventions
- stdout vs stderr
- Progress spinner format (active vs finished)
- Success + inline detail (two-line success pattern in add/remove)
- Test output structure (file header → cases → summary)
- Action preview format (pre-confirmation display for destructive operations)

---

## Changes to `cli-design.md`

Expand the `## Command Output` section with five new subsections.
Add a new `## Stdout vs Stderr` section after `## Status Messages`.

### 1. Expand "Status Messages" with a marker decision table

After the existing code block, add:

```markdown
### Choosing the Right Marker

| Situation | Marker |
|-----------|--------|
| Command completed successfully | `check_mark()` |
| Health check passed, test passed | `check_mark()` |
| Dependency added / removed / updated | `check_mark()` |
| Command failed, unrecoverable error | `error_prefix()` |
| Invalid input, bad config | `error_prefix()` |
| Non-fatal issue (operation still succeeded) | `warning_prefix()` |
| Health check failed, test failed | `cross_mark()` |
| Item not found (informational, not a failure) | `cross_mark()` |
| Suggestion or action preview (line prefix) | `arrow_mark()` |
| Inline transition between two values (e.g. old version → new version) | `arrow_mark()` |

**Key distinction:** `error_prefix` = the command itself failed.
`cross_mark` = a check or lookup found a negative result (but the command completed).

**`arrow_mark()` has two distinct roles:**
- **Line prefix**: `  → suggestion text` or `  → will add [section] to husako.toml`
- **Inline separator**: `old_version → new_version` in update output
```

### 2. New `## Stdout vs Stderr` section

```markdown
## Stdout vs Stderr

All output goes to **stderr** (`eprintln!`). Only `husako render` writes YAML to **stdout**
(`println!`). This keeps the YAML pipe-safe and separates user-visible diagnostics from
machine-readable output.
```

### 3. Expand `## Command Output` — Section Headers rule

Replace the current brief mention with a concrete rule:

```markdown
### Section Headers

Bold, followed by rows or a blank line:

```
Resources:
  kubernetes    release  v1.35
  cert-manager  git      github.com/...
```

- Section headers: `style::bold("Resources:")` (no blank line after)
- Column headers in tables (outdated, list): also `style::bold(...)` for the header row
- Blank line **between** sections, not within
```

### 4. New `### Key-Value Pairs` subsection

```markdown
### Key-Value Pairs

For info-style output (the `info` command), use fixed-width labels and plain values.
Ancillary details (file sizes, counts) are dim:

```
cert-manager  (git)
  Version:    v1.17.2
  Repo:       https://github.com/cert-manager/cert-manager
  Cache:      .husako/cache/... (dim: 1.2 MB)
```

- Dependency name line: `dep_name()` + `dim("(type)")` in parens
- Label column: plain text, fixed width (e.g. `{:<12}`)
- Value: plain text
- Ancillary detail in parens: `dim()`
```

### 5. New `### Instruction Blocks` subsection

```markdown
### Instruction Blocks

Post-operation "next steps" guidance:

```
✔ Created 'simple' project in my-project
  kubernetes 1.35  (dim: · edit husako.toml to configure dependencies)

Next steps:
  cd my-project
  husako gen
```

- Blank line before "Next steps:"
- `Next steps:` is plain text (not bold) — it's a run-on from the success output
- Commands are plain, indented two spaces
- Suggestion detail on the success line: `dim()`
```

### 6. New `### Empty State` subsection

```markdown
### Empty State

When a command has nothing to show, use plain text with no marker:

```
No dependencies configured
No versioned dependencies found
No test files found
```

No `check_mark`, no `error_prefix` — these are neutral informational states.
```

### 7. New `### Progress Spinners` subsection

```markdown
### Progress Spinners

Long-running operations (gen, update) use an `indicatif` spinner:

```
⠹ Fetching kubernetes release schema...    ← cyan spinner + dim message (while running)
✔ Generated k8s types for v1.35.0          ← check_mark when done OK
✘ Failed to fetch schema: ...              ← cross_mark when done with error
```

- Active: cyan spinner (via `ProgressStyle`) + plain message
- `finish_ok(msg)` → `check_mark()` + message (spinner replaced)
- `finish_err(msg)` → `cross_mark()` + message (spinner replaced)
```

### 8. New `### Success With Detail` subsection

```markdown
### Success With Detail

When a success message has a relevant source detail (URL, path, version), append it as a
dim second line in the same `eprintln!` call:

```rust
eprintln!(
    "{} Added {} to [{}]\n  {}",
    style::check_mark(),
    style::dep_name(name),
    section,
    style::dim(&detail),
);
```

Renders as:
```
✔ Added postgresql to [charts]
  https://charts.bitnami.com/bitnami  bitnami/postgresql  16.4.0
```

- Single `eprintln!` with embedded `\n  ` (not two separate calls)
- Detail line indented 2 spaces, wrapped in `dim()`
- No trailing blank line from this call — add a separate `eprintln!()` if needed
```

### 9. New `### Test Output` subsection

```markdown
### Test Output

The `test` command uses a structured report format:

```
filename.test.ts                        ← bold file header
  ✔ test name passes                    ← indented 2, check_mark
  ✘ test name fails                     ← indented 2, cross_mark
    expected 1 to equal 2               ← indented 4, dim error detail

✔ 5 passed, 0 failed                    ← summary line (blank line before)
```

- File header: `style::bold(filename)`
- Passed case: 2-space indent + `check_mark()` + plain test name
- Failed case: 2-space indent + `cross_mark()` + plain test name
- Error detail: 4-space indent + `dim(error)`
- Summary: blank line, then `check_mark()` or `cross_mark()` + count string
```

### 10. New `### Blank Lines` subsection

```markdown
### Blank Lines

- One blank line **between** major sections in multi-section output (list, debug)
- One blank line **before** instruction blocks ("Next steps:")
- One blank line **after** a success message that is followed by a warning or suggestion
- No blank line at the end of command output
```

### 11. New `### Action Preview` subsection

```markdown
### Action Preview

Before a destructive-or-irreversible confirmation prompt, show the user what the command
**will** do, then ask for confirmation. The preview is always shown, even with `--yes`.

```rust
eprintln!(
    "  Cluster: {}  {}",
    style::dep_name(display_name),
    style::dim(&server_url),
);
eprintln!(
    "  {} will add {} to husako.toml",
    style::arrow_mark(),
    style::bold(&section),
);
```

Renders as:
```
  Cluster: my-cluster  https://kubernetes.example.com
  → will add [clusters.my-cluster] to husako.toml
```

- Preview block is **always shown**, even when `--yes` skips the prompt
- Lines are indented 2 spaces — no top-level marker
- Entity name: `dep_name()`, secondary detail (URL, path): `dim()`
- Action line: `arrow_mark()` prefix + `bold()` for the key target (section name, path)
- Warning (if applicable) appears **before** the preview block using `warning_prefix()`
```

---

## Summary of Insertions

| Location in current doc | Insertion |
|--------------------------|-----------|
| After Status Messages code block | Marker decision table (incl. dual role of `arrow_mark`) |
| After Status Messages section | New `## Stdout vs Stderr` section |
| Command Output → Section Headers | Expand with column header rule |
| Command Output (new subsection) | Key-Value Pairs |
| Command Output (new subsection) | Instruction Blocks |
| Command Output (new subsection) | Empty State |
| Command Output (new subsection) | Progress Spinners |
| Command Output (new subsection) | Success With Detail |
| Command Output (new subsection) | Test Output |
| Command Output (new subsection) | Blank Lines |
| Command Output (new subsection) | Action Preview |

---

## Tests and Docs

- **Tests**: No changes needed — this is internal dev documentation only.
- **Docs**: This edit IS the docs change. No user-facing docs in `.worktrees/docs-site/` need updating (cli-design.md is in `.claude/`, not user-facing).

---

## Verification

Read the updated `cli-design.md` and verify:
1. Each new rule is consistent with the observed patterns in `main.rs`
2. The marker decision table covers all current usages
3. No contradictions with existing prompt rules
