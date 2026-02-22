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
| Selected value (after) | Cyan | `Resource` |
| FuzzySelect cursor | Black on white | dialoguer default |
| Success prefix | Green+bold `✔` (`\u{2714}`) | After selection confirmed |
| Failure prefix | Red+bold `✘` (`\u{2718}`) | After failure |

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

## Infinite Scroll (search_select)

- Items remain visible during loading (no full re-render)
- Spinning indicator appears at the bottom of the list while loading
- After load, spinner is replaced by new items or "no more results"

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

## NO_COLOR Support

All styling uses the `console` crate, which automatically disables ANSI escape codes
when `NO_COLOR=1` is set or when output is not a TTY.
