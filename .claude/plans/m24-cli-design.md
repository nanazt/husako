# M24: CLI Interface Design & Visual Consistency

## Context

The CLI has two incompatible visual systems:
- **dialoguer SimpleTheme** (default): No colors. Plain `prompt: value` after selection.
- **Custom `search_select.rs`**: Green+bold `?`, cyan+bold active items, `✔ prompt value` after selection.

Other inconsistencies: `style::check_mark()` is green (no bold) vs search_select uses green+bold. Loading messages unstyled. Command output has no section header emphasis or dim secondary info.

**Goal**: Define and implement a consistent visual language across all CLI prompts and output.

---

## Design Decisions

### Interactive Prompts

| Element | Choice |
|---------|--------|
| Prompt prefix | **None** (just bold text) |
| Prompt suffix | **Colon** `:` |
| Active item | **Cyan+bold `>`** |
| Default value hint | **Cyan parenthesized** `Name (postgresql):` |
| FuzzySelect cursor | **Black on white** (dialoguer default) |
| Selected value color | **Cyan** |
| After-selection | **Colon** `✔ Prompt: value` |
| Success prefix | **Green+bold `✔`** (`\u{2714}`) |
| Failure prefix | **Red+bold `✘`** (`\u{2718}`) |

### Prompt Flow

```
Select:     Dependency type:
            > Resource        (cyan+bold)
              Chart

Input:      Name: postgresql   (dim placeholder, disappears on type)
            Name: my-chart|    (user typing, placeholder gone)

Confirm:    Remove cache? (y/n):

After:      ✔ Dependency type: Resource   (✔ green+bold, prompt bold, value cyan)
            ✔ Name: my-chart

Cancelled:  (cleared, no output)
```

### Command Output

| Element | Style |
|---------|-------|
| Section headers (`Resources:`, `Charts:`) | **Bold** |
| Dep names | **Cyan** (existing) |
| Secondary info (details, paths, metadata) | **Dim** |
| Table columns | Improved alignment |

### Status Messages

```
Success:    ✔ message            (green+bold ✔)
Error:      error: message       (red+bold prefix)
Warning:    warning: message     (yellow+bold prefix)
Loading:    Fetching versions... (dim)
Suggestion: → suggestion         (cyan →)
```

### Infinite Scroll Loading

Instead of clearing the screen and showing a loading message, keep current items visible and show a spinner/progress bar at the bottom of the list.

---

## Implementation

### Step 1: Create `.claude/cli-design.md`

Document the visual language above as the authoritative reference.

### Step 2: Create `crates/husako-cli/src/theme.rs`

Custom `Theme` implementation needed because:
- No prompt prefix (ColorfulTheme's empty prefix leaves a leading space)
- Colon suffix instead of `›`
- Colon in after-selection format (ColorfulTheme's success_suffix gets extra spaces)
- Cyan default value hint (ColorfulTheme uses dim)

Create `HusakoTheme` wrapping `ColorfulTheme` for item rendering, override format methods:

```rust
pub struct HusakoTheme {
    inner: ColorfulTheme,  // for item rendering (active_item_prefix, etc.)
}
```

Inner `ColorfulTheme` customizations:
- `values_style`: cyan (not green)
- `active_item_prefix`: `>` cyan+bold (not `❯` green)
- `active_item_style`: cyan+bold (not just cyan)
- `defaults_style`: cyan (for default value hints)
- `fuzzy_cursor_style`: black on white (keep default)

Override format methods:

| Method | Format |
|--------|--------|
| `format_prompt` | `"{bold prompt}:"` |
| `format_input_prompt` | `"{bold prompt} ({cyan default}): "` or `"{bold prompt}: "` |
| `format_confirm_prompt` | `"{bold prompt} {dim (y/n)}: "` |
| `format_input_prompt_selection` | `"{green+bold ✔} {bold prompt}: {cyan value}"` |
| `format_confirm_prompt_selection` | `"{green+bold ✔} {bold prompt}: {cyan yes/no}"` |
| `format_fuzzy_select_prompt` | `"{bold prompt}: {search_term}"` |

Delegate to `inner` (keep ColorfulTheme behavior):
- `format_error`
- `format_select_prompt_item`
- `format_multi_select_prompt_item`
- `format_fuzzy_select_prompt_item`

### Step 3: Update `crates/husako-cli/src/style.rs`

| Change | Before | After |
|--------|--------|-------|
| `check_mark()` | `green()`, `\u{2713}` | `green().bold()`, `\u{2714}` |
| `cross_mark()` | `red()`, `\u{2717}` | `red().bold()`, `\u{2718}` |
| Add `warning_prefix()` | — | `yellow().bold()` `"warning:"` |
| Add `dim(s)` | — | `Style::new().dim()` |
| Add `bold(s)` | — | `Style::new().bold()` |

### Step 4: Create `crates/husako-cli/src/text_input.rs`

Custom text input widget (~80 lines) for dim placeholder behavior that dialoguer doesn't support natively:

- Shows dim placeholder text when input is empty (either the default value, or "press Enter for default")
- Placeholder disappears as soon as the user types
- On Enter with empty input, returns the default value
- On Enter with typed input, returns the typed value
- Shows `✔ Prompt: value` after confirmation (matching theme)
- Handles Escape to cancel, Backspace, basic text editing
- Supports optional validation callback (same `Fn(&String) -> Result<(), String>` as dialoguer)
- Shows validation error inline on Enter if invalid (red, same line re-render)

```
Empty:      Name: postgresql     (dim placeholder = default value)
Typing:     Name: my-chart|      (placeholder gone, user text shown)
Invalid:    Name: My Chart!      (red error below: "must contain only lowercase...")
Confirmed:  ✔ Name: my-chart     (green+bold ✔, bold prompt, cyan value)
```

Uses `console::Term` for raw key input (same approach as `search_select.rs`).

Add defaults where reasonable:
- **Cluster name**: placeholder `"default"` (currently uses `allow_empty` → None means default cluster)
- **Version prompts**: No default — "latest" would cause reproducibility issues. Keep manual entry without placeholder.
- Other prompts: no sensible defaults, keep dialoguer `Input::with_theme(&theme)`

### Step 5: Update `crates/husako-cli/src/interactive.rs`

Apply theme to all prompt call sites:

```rust
let theme = crate::theme::husako_theme();
Select::with_theme(&theme).with_prompt("Source type")...
```

- `Select::new()` → `Select::with_theme(&theme)` (8 sites)
- `FuzzySelect::new()` → `FuzzySelect::with_theme(&theme)` (1 site)
- `Input` prompts with defaults → `text_input::run()` (uses custom widget)
- `Input` prompts without defaults → `Input::with_theme(&theme)` (keep dialoguer)
- `Confirm::new()` → `Confirm::with_theme(&theme)` (1 site)

Style loading/warning messages:

| Before | After |
|--------|-------|
| `"Searching ArtifactHub..."` | `style::dim("Searching ArtifactHub...")` |
| `"Fetching Kubernetes versions..."` | `style::dim("Fetching ...")` |
| `"Fetching chart versions..."` | `style::dim("Fetching ...")` |
| `"Warning: search failed..."` | `style::warning_prefix()` + message |
| `"No packages found..."` | `style::warning_prefix()` + message |
| `"Warning: could not fetch..."` | `style::warning_prefix()` + message |

### Step 6: Update `crates/husako-cli/src/search_select.rs`

**Match the new theme** — remove `?` prefix, use colon suffix/separator:

```
// Prompt line: no prefix, colon suffix
// Before: "? Prompt"  →  After: "Prompt:"

// Confirmation: colon separator
// Before: "✔ Prompt value"  →  After: "✔ Prompt: value"

// Loading: no prefix, use bottom spinner
// Before: clear all → "? Prompt (loading...)"
// After: keep items visible → add spinner line at bottom
```

**Infinite scroll loading improvement**: Instead of clearing the entire screen and showing a loading message, keep the current items rendered and show a spinner at the bottom (replacing the "↓ more below" indicator with a spinning loading indicator). After data arrives, re-render the full list with new items.

### Step 7: Update `crates/husako-cli/src/main.rs`

- Add `mod theme;`
- Bold section headers: `"Resources:"` → `style::bold("Resources:")`
- Dim secondary info in list/info/debug output
- Improve table column alignment in `list` and `outdated` commands

---

## Files Summary

| File | Action | Description |
|------|--------|-------------|
| `.claude/cli-design.md` | Create | Design document |
| `crates/husako-cli/src/theme.rs` | Create | `HusakoTheme` with custom `Theme` impl (~100 lines) |
| `crates/husako-cli/src/text_input.rs` | Create | Custom text input with dim placeholder (~80 lines) |
| `crates/husako-cli/src/style.rs` | Modify | Bold marks, heavy glyphs, add helpers |
| `crates/husako-cli/src/interactive.rs` | Modify | `.with_theme()` sites, `text_input` for defaults, 6 message styles |
| `crates/husako-cli/src/search_select.rs` | Modify | Remove prefix, add colon, bottom-spinner loading |
| `crates/husako-cli/src/main.rs` | Modify | `mod theme`, `mod text_input`, bold headers, dim secondary, alignment |

## Tests

- `style.rs`: `warning_prefix_is_non_empty`, `dim_returns_non_empty`, `bold_returns_non_empty`
- `theme.rs`: `format_prompt_no_prefix`, `format_selection_has_colon`, `format_confirm_selection`, `format_input_with_default`
- `search_select.rs`: existing `constants_are_valid` still passes
- Existing tests unchanged (console strips ANSI on non-TTY)

## Verification

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
# Manual: husako add → no prefix, colon suffix, cyan highlights, ✔ Prompt: value
# Manual: husako add --chart → artifacthub search → infinite scroll with bottom spinner
# Manual: husako clean → styled confirm
# Manual: husako remove → styled FuzzySelect
# Manual: husako list → bold headers, dim details
# Manual: husako debug → bold marks, dim suggestions
# Manual: NO_COLOR=1 husako add → no ANSI escapes
```
