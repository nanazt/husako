# Plan: Commit pending changes

## Context

After writing the output consistency standards, a re-read and codebase audit of
`cli-design.md` reveals four issues worth fixing:

1. **Stale `## Custom Widgets` section** — documents `search_select.rs` which no longer
   exists. Verified: no `search_select.rs`, `text_input.rs`, `SearchSelect`,
   `with_echo_suppressed`, or `LOAD_THRESHOLD` anywhere in the codebase. Interactive
   selection now uses standard dialoguer `Select`/`FuzzySelect` with `HusakoTheme`.

2. **Stale `Scroll indicator` row in Elements table** — `↑ more above / ↓ more below` /
   `loading…` was specific to the removed custom widget.

3. **`Loading:` line in Status Messages block** — referred to the custom widget's
   `loading…` feedback. Command-level progress is already covered by `Progress Spinners`.

4. **Missing "Silent Success" rule** — `husako check` exits 0 with no output on success.
   This rule isn't documented; new commands might add unnecessary output.

**File to modify:** `/Users/syr/Developments/husako/.claude/cli-design.md`

No code changes. No tests.

---

## Changes

### 1. Remove `## Custom Widgets` section

Delete the entire section and both subsections (`search_select`, `Echo Suppression`).
Roughly 30 lines of dead documentation.

### 2. Remove `Scroll indicator` row from Elements table

```diff
- | Scroll indicator | Dim `↑ more above` / `↓ more below` | Replaced by `loading…` during fetch |
```

### 3. Fix Status Messages block — remove `Loading:` line

```diff
  Success:    ✔ Operation succeeded        ← green+bold ✔
  Error:      error: something went wrong  ← red+bold "error:"
  Warning:    warning: something unusual   ← yellow+bold "warning:"
- Loading:    Fetching versions...         ← dim
  Suggestion: → try this instead           ← cyan →
```

### 4. Add `### Silent Success` subsection

Insert after `### Empty State`:

```markdown
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
```

---

## Critical Files

- `/Users/syr/Developments/husako/.claude/cli-design.md` — only file to edit

---

## Tests and Docs

- **Tests**: None — internal dev documentation only.
- **Docs**: This edit IS the docs change.

---

## Verification

Read the updated `cli-design.md` and confirm:
1. No references to `search_select.rs`, `text_input.rs`, `with_echo_suppressed`, `LOAD_THRESHOLD`
2. Elements table has no `Scroll indicator` row
3. Status Messages block has no `Loading:` line
4. `### Silent Success` subsection is present after `### Empty State`
