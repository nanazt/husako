# Plan: Remove interactive k8s version picker, auto-select latest, show version in output

## Context

`husako new` / `husako init` currently run `select_k8s_version()` which shows an
interactive list of recent k8s releases for the user to pick from. This adds friction
to project creation — users almost always want the latest version, and can always edit
`husako.toml` later if they need a different one.

Goal:
1. Remove the interactive version picker entirely
2. Auto-select the latest k8s version (fetch from GitHub; fall back to `"1.35"` on
   network failure, silently)
3. Show the chosen version in the success output with a hint to edit `husako.toml`

---

## Target UX

```
✔ Created 'simple' project in myapp
  kubernetes 1.35  · edit husako.toml to use a different version

Next steps:
  cd myapp
  husako generate
```

- `1.35` → `style::bold()`
- `· edit husako.toml...` → `style::dim()`
- No prompt, no spinner — creation is instant

---

## Changes

### `crates/husako-cli/src/main.rs`

#### a) Replace `select_k8s_version()` with `latest_k8s_version()`

Remove the old function entirely. Add:

```rust
fn latest_k8s_version() -> String {
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(husako_core::version_check::discover_recent_releases(1, 0))
    });
    match result {
        Ok(versions) if !versions.is_empty() => versions.into_iter().next().unwrap(),
        _ => DEFAULT_K8S_VERSION.to_string(),
    }
}
```

- Fetches only 1 entry (the latest) from `discover_recent_releases`
- Falls back to `DEFAULT_K8S_VERSION = "1.35"` on any error, silently
- No interactive input, no spinner

#### b) `New` command handler

Replace the `select_k8s_version()` call with `latest_k8s_version()` and add the
version info line:

```rust
Commands::New { directory, template } => {
    let k8s_version = latest_k8s_version();

    let options = ScaffoldOptions { directory: directory.clone(), template, k8s_version: k8s_version.clone() };

    match husako_core::scaffold(&options) {
        Ok(()) => {
            eprintln!(
                "{} Created '{}' project in {}",
                style::check_mark(),
                template,
                directory.display()
            );
            eprintln!(
                "  kubernetes {}  {}",
                style::bold(&k8s_version),
                style::dim("· edit husako.toml to use a different version")
            );
            eprintln!();
            eprintln!("Next steps:");
            eprintln!("  cd {}", directory.display());
            eprintln!("  husako generate");
            ExitCode::SUCCESS
        }
        Err(e) => { ... }
    }
}
```

#### c) `Init` command handler

Same replacement — remove `select_k8s_version()` call, add version info line:

```rust
Commands::Init { template } => {
    let project_root = cwd();
    let k8s_version = latest_k8s_version();

    let options = husako_core::InitOptions { directory: project_root, template, k8s_version: k8s_version.clone() };

    match husako_core::init(&options) {
        Ok(()) => {
            eprintln!(
                "{} Created '{template}' project in current directory",
                style::check_mark()
            );
            eprintln!(
                "  kubernetes {}  {}",
                style::bold(&k8s_version),
                style::dim("· edit husako.toml to use a different version")
            );
            eprintln!();
            eprintln!("Next steps:");
            eprintln!("  husako generate");
            ExitCode::SUCCESS
        }
        Err(e) => { ... }
    }
}
```

#### d) Remove dead code

- Delete `select_k8s_version()` function (~65 lines)
- Remove unused imports introduced by it (infinite scroll logic, `console::Term` TTY check, etc.) — let clippy guide

---

## Tests

### Existing tests — no changes needed

The 3 tests asserting on success text use `contains()`, so they pass unchanged:
- `new_simple_creates_project()` — `contains("Created 'simple' project")`
- `new_project_template()` — `contains("Created 'project' project")`
- `new_multi_env_template()` — `contains("Created 'multi-env' project")`

Tests run non-interactively, so `discover_recent_releases` will likely fail (no GitHub
access in test env) and fall back to `"1.35"` — same as before.

### New assertions to add

Extend the 3 existing `new_*` tests with one additional assertion:

```rust
.stderr(predicates::str::contains("kubernetes"))
```

Confirms the version info line appears. No new test functions needed.

---

## Docs

CLI output-only change, no new flags or config options. No doc update needed.

---

## Verification

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Manual smoke test:
```bash
husako new /tmp/test-proj
# Should print kubernetes X.Y line immediately, no interactive prompt
```
