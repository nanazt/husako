# Plan: Auto-run `husako gen` after config-mutating commands and before render (if types missing)

## Context

`husako update` already auto-regenerates types after updating versions — this is the
right pattern. `husako add`, `husako remove`, `husako plugin add`, and `husako plugin
remove` all modify `husako.toml` but currently leave type regeneration to the user.
`husako render` fails with a runtime error (exit 4) when types are missing, forcing
users to know they need `husako gen` first.

Goal: make the most common workflows self-contained — adding a dependency or rendering
should just work without manual gen steps.

**Design decisions:**
- `husako update`: already auto-gens — no change needed
- `husako add / remove / plugin add / plugin remove`: auto-gen after config write; gen
  failure is non-fatal (warn + succeed, since the config change itself succeeded)
- `husako render`: if `.husako/types/` is missing → auto-gen then render; stale types
  → no warning, render proceeds (husako debug handles staleness info)
- No `--no-gen` flag — always auto-gen for simplicity

---

## Changes

### `crates/husako-cli/src/main.rs` only

#### A) New async helper `run_auto_generate(project_root)`

Add near the top of the file (after existing helpers):

```rust
/// Run generate with default options derived from husako.toml config.
/// Returns Ok(()) on success, or an error on failure.
/// Used by commands that implicitly need fresh types after config changes.
async fn run_auto_generate(project_root: &std::path::Path) -> Result<(), husako_core::HusakoError> {
    let progress = IndicatifReporter::new();
    let config = husako_config::load(project_root).ok().flatten();
    let options = husako_core::GenerateOptions {
        project_root: project_root.to_path_buf(),
        openapi: None,   // derive from config; no CLI overrides
        skip_k8s: false,
        config,
    };
    husako_core::generate(&options, &progress).await
}
```

#### B) `Commands::Add` success branch

After the existing `✔ Added ...` eprintln chain, call auto-gen.
Gen failure prints a warning but the command still exits SUCCESS:

```rust
eprintln!();
match run_auto_generate(&project_root).await {
    Ok(()) => {}
    Err(e) => eprintln!("{} Type generation failed: {e}", style::warning_prefix()),
}
```

#### C) `Commands::Remove` success branch

Same pattern after the `✔ Removed ...` line.

#### D) `Commands::Plugin { PluginAction::Add }` success branch

Replace the current `eprintln!("Run 'husako gen' to install the plugin and generate types.")`:

```rust
// Remove suggestion line, add auto-gen:
eprintln!();
match run_auto_generate(&project_root).await {
    Ok(()) => {}
    Err(e) => eprintln!("{} Type generation failed: {e}", style::warning_prefix()),
}
```

#### E) `Commands::Plugin { PluginAction::Remove }` success branch

Add auto-gen after the `✔ Removed plugin ...` line (same pattern).

#### F) `Commands::Render` — pre-flight check

Before calling `husako_core::render(...)`, check if `.husako/types/` exists:

```rust
// Pre-flight: if types directory is missing, auto-gen first
let types_dir = project_root.join(".husako").join("types");
if !types_dir.exists() {
    if let Err(e) = run_auto_generate(&project_root).await {
        eprintln!("{} Could not generate types: {e}", style::error_prefix());
        return ExitCode::from(exit_code(&e));
    }
}
// ... existing render call follows
```

For render, gen failure IS fatal (render can't proceed without types), so exit with error.
Stale types (toml newer than types dir): no warning — render proceeds with existing types.

---

## Tests

### Integration test impact

Existing `husako add` / `husako remove` tests use minimal `husako.toml` files (usually
`[resources]\n` or similar). When `run_auto_generate` is triggered:
- No `[resources]` or `[charts]` configured → `generate()` writes only the base SDK
  types (`husako.d.ts`, `tsconfig.json`) and succeeds
- Tests should still pass without changes

If any add/remove tests break due to auto-gen output (stderr assertions), update those
assertions to allow the additional output lines.

### New test for render pre-flight (optional)

```rust
#[test]
fn render_auto_generates_when_types_missing() {
    // Create project with husako.toml but no .husako/types/
    // Run husako render — assert no "husako gen to be run first" error
}
```

---

## Docs

Update `.worktrees/docs-site/docs/guide/getting-started.md` to remove the explicit
"run husako gen after adding dependencies" step — the commands handle it now.

---

## Verification

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Manual smoke tests:
```bash
husako add --resource release 1.35  # → auto-gens types after adding
husako remove kubernetes            # → auto-gens types after removing
husako plugin add --url <url>       # → auto-gens (not just suggestion message)
husako render entry.ts              # (no types) → auto-gens first, then renders
```
