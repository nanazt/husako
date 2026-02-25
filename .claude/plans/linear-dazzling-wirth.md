# Plan: Remove confirmation prompt from `husako clean`

## Context

`husako clean` currently shows two prompts when run without flags:
1. `prompt_clean()` — select what to clean (Cache / Types / Both)
2. `confirm()` — "Remove {targets}?" yes/no

The second confirmation is unnecessary friction. The user already expressed intent by running `husako clean`. Remove it entirely.

## Change

**File**: `crates/husako-cli/src/main.rs`

Delete the `if !cli.yes { ... confirm ... }` block from the `Commands::Clean` handler:

```rust
// REMOVE this entire block:
if !cli.yes {
    let targets = match (do_cache, do_types) {
        (true, true) => "cache and types",
        (true, false) => "cache",
        (false, true) => "types",
        _ => unreachable!(),
    };
    match interactive::confirm(&format!("Remove {targets}?")) {
        Ok(true) => {}
        Ok(false) => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{} {e}", style::error_prefix());
            return ExitCode::from(1);
        }
    }
}
```

The `prompt_clean()` selection (when no flags given) is kept — it determines *what* to clean.

## Docs Update

**File**: `.worktrees/docs-site/docs/reference/cli.md`

Remove the two lines under the `husako clean` section:
```
Prompts for confirmation by default.

Use `-y` / `--yes` to skip.
```

## Verification

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```
