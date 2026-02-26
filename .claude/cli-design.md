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
| Scroll indicator | Dim `↑ more above` / `↓ more below` | Replaced by `loading…` during fetch |

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

## Status Messages

```
Success:    ✔ Operation succeeded        ← green+bold ✔
Error:      error: something went wrong  ← red+bold "error:"
Warning:    warning: something unusual   ← yellow+bold "warning:"
Loading:    Fetching versions...         ← dim
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

Long-running operations (gen, update) use an `indicatif` spinner:

```
⠹ Fetching kubernetes release schema...    ← spinner + plain message (while running)
✔ Generated k8s types for v1.35.0          ← check_mark when done OK
✘ Failed to fetch schema: ...              ← cross_mark when done with error
```

- Active: cyan spinner (via `ProgressStyle`) + plain message
- `finish_ok(msg)` → `check_mark()` + message (spinner replaced)
- `finish_err(msg)` → `cross_mark()` + message (spinner replaced)

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

## Custom Widgets

One custom widget built on `console::Term` with raw key input. Writes to stderr.

### search_select (`search_select.rs`)

Scrollable list with infinite scroll, used for Kubernetes version selection in `husako new` / `husako init`.

```
? Kubernetes version:
  > 1.35 (latest)     ← cyan+bold
    1.34
    1.33
    ↓ more below      ← dim, or "loading…" during fetch
```

- Up/Down navigate without wrapping
- Auto-loads more items when cursor approaches bottom (`LOAD_THRESHOLD = 3`)
- `↓ more below` swaps to `loading…` in-place during fetch (no layout shift)
- `↑ more above` shown when scrolled past the top
- Max 10 items visible at once
- Enter confirms (`✔ prompt value`), Escape cancels

### Echo Suppression

The widget uses `with_echo_suppressed()` during blocking network calls:

1. Enter crossterm raw mode to prevent arrow key escape sequences from echoing
2. Execute the blocking fetch
3. Restore normal mode
4. Drain any buffered key events via `crossterm::event::poll()` + `read()`

This prevents stray characters from appearing in the terminal while the user presses keys during loading.

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
