# Plan: husako render --output (file output support)

## Context

`husako render` currently only outputs to stdout. For FluxCD/GitOps workflows — and general use
cases where users want to write rendered manifests to files — a `--output` flag is needed.

Multi-document YAML (multiple resources with `---`) is fully supported by FluxCD, so one YAML
file per entry is sufficient; no need to split by resource.

## Target behaviour

```bash
# Write to a specific file
husako render entry.ts --output out.yaml
# → writes out.yaml

# Write to a directory (uses file stem as filename)
husako render entry.ts --output ./dist
# → writes dist/entry.yaml  (creates dirs as needed)

# Alias with slash → preserves path structure
husako render apps/my-app --output ./dist
# → writes dist/apps/my-app.yaml  (alias string used as path)

# No --output → stdout (unchanged)
husako render entry.ts
```

**Path resolution rule:**
- `--output` path ends with `.yaml` or `.yml` → write to that exact file
- Otherwise → treat as directory:
  - Entry resolved by alias → `<dir>/<alias>.yaml` (preserving `/` in alias)
  - Entry resolved by direct path → `<dir>/<stem>.yaml` (file stem only)

## Changes

### 1. `crates/husako-cli/src/main.rs` — `Commands::Render` struct

Add `--output` flag (keep `file: String` as required positional, no changes to other flags):

```rust
Render {
    /// Path to the TypeScript entry file, or an entry alias from husako.toml
    file: String,

    /// Write output to a file or directory instead of stdout.
    /// If path ends with .yaml/.yml, write to that file directly.
    /// Otherwise treat as directory: writes <dir>/<name>.yaml
    /// where <name> is the alias string or the entry file's stem.
    #[arg(long, short = 'o', value_name = "PATH")]
    output: Option<PathBuf>,

    // existing flags unchanged
    #[arg(long)] allow_outside_root: bool,
    #[arg(long)] timeout_ms: Option<u64>,
    #[arg(long)] max_heap_mb: Option<usize>,
    #[arg(long)] verbose: bool,
},
```

### 2. `crates/husako-cli/src/main.rs` — new helper `derive_out_name()`

Add below `resolve_entry`:

```rust
/// Return the output name for a file argument (no extension, may contain `/`).
/// - If file_arg matches an alias in config → return the alias string as-is
///   (so "apps/my-app" becomes the path component "apps/my-app").
/// - Otherwise → return the file stem of the path.
fn derive_out_name(file_arg: &str, project_root: &Path) -> String {
    if let Ok(Some(cfg)) = husako_config::load(project_root) {
        if cfg.entries.contains_key(file_arg) {
            return file_arg.to_string();
        }
    }
    Path::new(file_arg)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_arg.to_string())
}
```

### 3. `crates/husako-cli/src/main.rs` — render handler output logic

After `render()` returns `Ok(yaml)`:

```rust
Ok(yaml) => {
    if let Some(out_path) = output {
        // Determine actual file path
        let file_path = if out_path.extension().is_some_and(|e| e == "yaml" || e == "yml") {
            out_path
        } else {
            let name = derive_out_name(&file, &project_root);
            out_path.join(format!("{name}.yaml"))
        };
        // Create parent directories (async — handler is already in tokio runtime)
        if let Some(parent) = file_path.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                eprintln!("{} {e}", style::error_prefix());
                return ExitCode::from(1);
            }
        }
        // Write file (async)
        if let Err(e) = tokio::fs::write(&file_path, &yaml).await {
            eprintln!("{} {e}", style::error_prefix());
            return ExitCode::from(1);
        }
        eprintln!(
            "{} Written to {}",
            style::check_mark(),
            style::bold(&file_path.display().to_string())
        );
    } else {
        print!("{yaml}");
    }
    ExitCode::SUCCESS
}
```

### 4. `crates/husako-cli/tests/integration.rs` — new tests

```
render_output_to_yaml_file
  render entry.ts --output out.yaml → file exists, contains valid YAML, stdout empty

render_output_to_dir_uses_stem
  render entry.ts --output ./dist → dist/entry.yaml exists

render_output_alias_preserves_path
  alias "apps/my-app" + --output ./dist → dist/apps/my-app.yaml (subdirs created)

render_output_creates_nested_dirs
  --output ./a/b/c/out.yaml → creates a/b/c/ and writes out.yaml

render_output_overwrites_existing
  write existing file → no error, content replaced
```

### 5. `.worktrees/docs-site/docs/reference/cli.md` — update husako render section

Three targeted changes (lines 48–61):

1. Description: `"Compile a TypeScript entry file and emit YAML to stdout."` →
   `"Compile a TypeScript entry file and emit YAML to stdout or a file."`

2. Add `--output` row to the flags table:
   ```
   | `-o, --output <path>` | Write YAML to a file or directory instead of stdout. If path ends with `.yaml` or `.yml`, writes to that exact file. Otherwise treated as a directory: writes `<dir>/<name>.yaml` where `<name>` is the entry alias or file stem. |
   ```

3. No other changes — no new snapshot files, no existing test changes needed.

## Critical files

- `crates/husako-cli/src/main.rs` — `Commands::Render` + handler + `derive_out_name()`
- `crates/husako-cli/tests/integration.rs` — 5 new tests
- `.worktrees/docs-site/docs/reference/cli.md` — flags table update

No changes to `husako-core` (render() already returns `String`).

## Verification

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```
