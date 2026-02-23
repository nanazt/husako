# M22: Colored Output

**Status**: Complete

## Summary

Added structural-only color to CLI output using the `console` crate.

## Color Palette

| Role | Style | Where Used |
|------|-------|------------|
| `error:` prefix | `red().bold()` | All error messages |
| `✓` checkmark | `green()` | debug, validate, outdated, progress finish, new/init, clean, add/remove, update |
| `✗` cross | `red()` | debug, progress fail, update failed |
| `→` arrow | `cyan()` | debug suggestions, update version arrows |
| Dependency name | `cyan()` | list, info, outdated, update, add/remove results |

## Files Changed

| File | Action | Description |
|------|--------|-------------|
| `crates/husako-cli/src/style.rs` | **Created** | Color helpers: `error_prefix()`, `check_mark()`, `cross_mark()`, `arrow_mark()`, `dep_name()` |
| `crates/husako-cli/src/main.rs` | **Modified** | All markers and error prefixes use styled output |
| `crates/husako-cli/src/progress.rs` | **Modified** | `finish_ok` / `finish_err` use styled marks |

## Tests

- `style::tests::helpers_return_non_empty` — validates all helpers return non-empty strings
- Existing integration tests pass unchanged (console disables colors on non-TTY)
