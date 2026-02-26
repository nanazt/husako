# CLI Visual Design

This document is the authoritative reference for husako's CLI visual language.
All interactive prompts, status messages, and command output must follow these rules.

## Interactive Prompts

### Elements

| Element | Style | Notes |
|---------|-------|-------|
| Prompt prefix | None | No leading character or space |
| Prompt suffix | Bold text + `:` | `**Source type:**` |
| Active item | Cyan+bold `>` | `> Resource` |
| Inactive item | Plain | `  Chart` |
| Default value hint | Cyan in parens | `**Name** (postgresql):` |
| Inline hint | Dim text after placeholder | `(Enter to confirm)` |
| Selected value (after) | Cyan | `Resource` |
| FuzzySelect cursor | Black on white | dialoguer default |
| Success prefix | Green+bold `✔` (`\u{2714}`) | After selection confirmed |
| Failure prefix | Red+bold `✘` (`\u{2718}`) | After failure |
| Latest tag | Appended to first version item | `1.16.3 (latest)` |

### Prompt Flow

```
Select:
    Source type:
    > Resource        ← cyan+bold
      Chart

Input (no default):
    Name: |           ← cursor, no placeholder

Input (with default):
    Name: postgresql  ← dim placeholder, disappears on type
    Name: my-chart|   ← user typing, placeholder gone

Confirm:
    Remove cache? (y/n):

After confirmation:
    ✔ Source type: Resource   ← ✔ green+bold, prompt bold, value cyan
    ✔ Name: my-chart

Cancelled:
    (cleared, no output)
```

### Rules

- **No prompt prefix** — no `?` or other leading character
- **Colon suffix** — every prompt ends with `:` (or `: ` before input field)
- **After-selection format** — `✔ {bold prompt}: {cyan value}` on one line
- **Validation errors** — shown in red on the line below the prompt, then re-render
- **Inline hints** — dim text placed on the same line as the prompt to avoid layout shift
- **Minimize interactions** — interactive prompts (selections, confirmations, inputs)
  are a last resort, used only when information truly cannot be inferred from arguments
  or context. Prefer flags over prompts; prefer direct action over confirmation.
  Example: `husako remove <name>` removes immediately — no confirm needed since the
  operation is reversible and the intent is unambiguous from the argument.

## Status Messages

```
Success:    ✔ Operation succeeded        ← green+bold ✔
Error:      error: something went wrong  ← red+bold "error:"
Warning:    warning: something unusual   ← yellow+bold "warning:"
Suggestion: → try this instead           ← cyan →
```

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
| Inline transition between two values | `arrow_mark()` |

**Key distinction:** `error_prefix` = the command itself failed (exits non-zero).
`cross_mark` = a check or lookup found a negative result, but the command completed.

**`arrow_mark()` has two distinct roles:**
- **Line prefix**: `  → suggestion text` or `  → will add [section] to husako.toml`
- **Inline separator**: `old_version → new_version` in update output

## Stdout vs Stderr

All output goes to **stderr** (`eprintln!`). Only `husako render` writes YAML to **stdout**
(`println!`). This keeps the YAML pipe-safe and separates user-visible diagnostics from
machine-readable output.

## Command Output

### Section Headers

Bold, followed by rows or a blank line:

```
Resources:
  kubernetes    release  v1.35
  cert-manager  git      github.com/...

Charts:
  postgresql    registry https://charts.bitnami.com
```

- Section headers: `style::bold("Resources:")` — no blank line after, rows follow immediately
- Column headers in multi-column tables (`outdated`): also `style::bold(...)` for the header row
- Blank line **between** sections, not within

### Dependency Names

Always **cyan** (`style::dep_name()`).

### Secondary Information

Paths, metadata, version details, extra context — all **dim** (`style::dim()`).

### Table Alignment

Columns in `list` and `outdated` output use `{:<N}` fixed-width fields for legibility.
The width is chosen to fit the longest realistic value in each column.

### Key-Value Pairs

For info-style output (the `info` command), use fixed-width labels and plain values.
Ancillary details (file sizes, counts) are dim:

```
cert-manager  (git)
  Version:    v1.17.2
  Repo:       https://github.com/cert-manager/cert-manager
  Cache:      .husako/cache/... (1.2 MB)
```

- Dependency name line: `dep_name()` + `dim("(type)")` in parens
- Label column: plain text, fixed width (`{:<12}`)
- Value: plain text
- Ancillary detail in parens: `dim()`

### Empty State

When a command has nothing to show, use plain text with no marker:

```
No dependencies configured
No versioned dependencies found
No test files found
```

No `check_mark`, no `error_prefix` — these are neutral informational states.

### Filter to Actionable

Status/check commands show only items that need attention. If all items pass, show a
single success line instead of a full table:

```
✔ All dependencies are up to date
```

Don't mix passing items into a table alongside actionable ones — showing every entry
with a `✔` or `✘` adds noise. Show only items the user needs to act on.

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

### Silent Success

Commands that only check without mutating state emit **no output on success** — exit 0
is the signal:

```
$ husako check entry.ts   ← no output on success
$ echo $?
0
```

- Check/validate commands: silent on success, `error_prefix()` + non-zero on failure
- Mutating commands (add, remove, gen): always emit a `check_mark()` success line

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

### Action Preview

Before a destructive or irreversible confirmation prompt, show the user what the command
**will** do. The preview is always shown, even when `--yes` skips the prompt.

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
- Action line: `arrow_mark()` prefix + `bold()` for the key target
- Warning (if applicable) appears **before** the preview block using `warning_prefix()`

### Instruction Blocks

Post-operation "next steps" guidance:

```
✔ Created 'simple' project in my-project
  kubernetes 1.35  · edit husako.toml to configure dependencies

Next steps:
  cd my-project
  husako gen
```

- Blank line before "Next steps:"
- `Next steps:` is plain text (not bold)
- Commands are plain, indented two spaces
- Suggestion detail on the success line: `dim()`

### Progress Spinners

Every long-running step (gen, render, update, plugin add, …) gets **one line per step**.
Each line starts as a spinner and is replaced in-place by a `✔` or `✘` on completion.
**No** `set_message` calls to change the task description mid-flight — only `set_progress`
to append a byte/percentage suffix.

```
⠹ [1/3] Resolving cert-manager... (52% · 5.0 MB / 9.5 MB)   ← spinner + progress
✔ [1/3] cert-manager: 4 group-versions                        ← replaced on finish_ok
✔ [2/3] kustomize: 2 group-versions
⠋ [3/3] ingress-nginx: 2 group-versions
```

Network progress suffix format:
- HTTP with total:   `(52% · 5.0 MB / 9.5 MB)`
- HTTP no total:     `(5.0 MB Received)`
- Git (% from object count + bytes): `(45% · 53.3 MB Received)`
- Percentage only:   `(30%)`
- Separator:         `·` (U+00B7)

`[N/M]` counter prefix is shown when `set_total(N)` is called before `start_task`.
The counter resets each time `set_total` is called (one counter per phase).

Render pipeline (4 steps):
```
⠹ [1/4] Compiling entry.ts...
✔ [1/4] Compiled entry.ts
✔ [2/4] Executed
✔ [3/4] Validated
✔ [4/4] Emitted 3 document(s)
```

- Active: cyan spinner (via `ProgressStyle`) + plain message
- `finish_ok(msg)` → `check_mark()` + `[N/M]` prefix + message (spinner replaced)
- `finish_err(msg)` → `cross_mark()` + `[N/M]` prefix + message (spinner replaced)

### Test Output

The `test` command uses a structured report format:

```
filename.test.ts
  ✔ test name passes
  ✘ test name fails
    expected 1 to equal 2

✔ 5 passed, 0 failed
```

- File header: `style::bold(filename)`
- Passed case: 2-space indent + `check_mark()` + plain test name
- Failed case: 2-space indent + `cross_mark()` + plain test name
- Error detail: 4-space indent + `dim(error)`
- Summary: blank line before, then `check_mark()` or `cross_mark()` + count string

### Blank Lines

- One blank line **between** major sections in multi-section output (list, debug)
- One blank line **before** instruction blocks ("Next steps:")
- One blank line **after** a success message that is followed by a warning or suggestion
- No blank line at the end of command output

## Color Helpers (style.rs)

| Function | Output |
|----------|--------|
| `check_mark()` | Green+bold ✔ |
| `cross_mark()` | Red+bold ✘ |
| `arrow_mark()` | Cyan → |
| `error_prefix()` | Red+bold `error:` |
| `warning_prefix()` | Yellow+bold `warning:` |
| `dep_name(s)` | Cyan text |
| `dim(s)` | Dim text |
| `bold(s)` | Bold text |

## Theme (theme.rs)

`HusakoTheme` wraps `dialoguer::ColorfulTheme` with husako-specific overrides:

| Method | Format |
|--------|--------|
| `format_prompt` | `**Prompt:**` — bold, no prefix, colon suffix |
| `format_input_prompt` | `**Prompt** *(default)*: ` — default in cyan parens |
| `format_confirm_prompt` | `**Prompt** *(y/n)*: ` — hint in dim parens |
| `format_input_prompt_selection` | `✔ **Prompt**: *value*` — green check, cyan value |
| `format_confirm_prompt_selection` | `✔ **Prompt**: *yes/no*` |
| `format_fuzzy_select_prompt` | `**Prompt**: search_term` — no `?` prefix |

Used by all `dialoguer` prompts (`Select`, `Input`, `Confirm`, `FuzzySelect`) via `husako_theme()`.

## NO_COLOR Support

All styling uses the `console` crate, which automatically disables ANSI escape codes
when `NO_COLOR=1` is set or when output is not a TTY.
