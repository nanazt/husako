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

## Command Output

### Section Headers

Bold, followed by a blank line or indent:

```
Resources:
  kubernetes    release v1.35
  cert-manager  git     github.com/cert-manager/...

Charts:
  postgresql    registry https://charts.bitnami.com
```

### Dependency Names

Always **cyan** (`style::dep_name()`).

### Secondary Information

Paths, metadata, version details, extra context — all **dim** (`style::dim()`).

### Table Alignment

Columns in `list` and `outdated` output use consistent minimum widths for legibility.

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
