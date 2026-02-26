# Plan: Add optional `path` subdir field to `PluginSource::Git`

## Context

The Flux CD plugin is bundled inside the husako repository at `plugins/flux/`. External users who want to reference it need to either (a) clone husako and use `source = "path"`, or (b) reference it from the git repo with a subdirectory. Option (a) is impractical for real projects. Option (b) doesn't work today because `PluginSource::Git` only has a `url` field.

`SchemaSource::Git` and `ChartSource::Git` already support an optional subdirectory via a `path` field. `PluginSource::Git` should follow the same pattern.

**Goal:** Add an optional `path: Option<String>` to `PluginSource::Git` so users can reference a plugin inside a monorepo:

```toml
flux = { source = "git", url = "https://github.com/nanazt/husako", path = "plugins/flux" }
```

## Files to modify

| File | Change |
|------|--------|
| `crates/husako-config/src/lib.rs` | Add `path: Option<String>` to `PluginSource::Git` |
| `crates/husako-core/src/plugin.rs` | Update `install_git` to support subdir via sparse-checkout |
| `crates/husako-config/src/edit.rs` | Update `add_plugin` serialization (TOML editing helper) |
| `.claude/plugin-spec.md` | Update source type table + example |
| `docs/advanced/plugins.md` | Update install example to use husako repo + path |
| `docs/guide/configuration.md` | Same fix in overview snippet and plugins section |

## Implementation

### 1. `crates/husako-config/src/lib.rs`

Change `PluginSource::Git`:

```rust
#[serde(rename = "git")]
Git { url: String, path: Option<String> },
```

### 2. `crates/husako-core/src/plugin.rs`

Update `install_plugin` to pass `path` to `install_git`:

```rust
PluginSource::Git { url, path } => install_git(name, url, path.as_deref(), target_dir),
```

Update `install_git` signature and logic:

```rust
fn install_git(name: &str, url: &str, subdir: Option<&str>, target_dir: &Path) -> Result<(), HusakoError>
```

**When `subdir` is `None`:** behavior unchanged — shallow-clone the whole repo.

**When `subdir` is `Some(sub)`:** use git sparse-checkout to download only the subdirectory, then move its contents to `target_dir`:

```
git init <tmp>
git -C <tmp> remote add origin <url>
git -C <tmp> sparse-checkout set <sub>
git -C <tmp> pull --depth 1 origin HEAD
cp -r <tmp>/<sub>/* <target_dir>/
rm -rf <tmp>
```

Use a temp directory alongside `target_dir` (e.g., `target_dir` + `_tmp`) to stage the clone, then move the subdirectory into `target_dir`. Clean up on failure.

### 3. `crates/husako-config/src/edit.rs`

The `add_plugin` function builds the TOML inline table. It should emit `path` when set:

```toml
flux = { source = "git", url = "...", path = "plugins/flux" }
```

Check existing `add_plugin` logic and add `path` field serialization when `Some`.

### 4. Docs and spec

- `docs/advanced/plugins.md` line 32: change to:
  ```toml
  flux = { source = "git", url = "https://github.com/nanazt/husako", path = "plugins/flux" }
  ```
- `docs/guide/configuration.md`: same in overview snippet and plugins section
- `.claude/plugin-spec.md`: update source table row for `git` to add `path (optional)` column and update example

## Verification

1. `cargo fmt --all` + `cargo clippy --workspace --all-targets --all-features -- -D warnings`
2. `cargo test --workspace --all-features` — all existing tests pass
3. Add unit test in `husako-config` verifying that a git source with `path` deserializes correctly
4. Add unit test in `husako-core/plugin.rs` for the subdir branch of `install_git` (can mock or skip git network call — just verify error on missing git, or use a local bare repo fixture if one exists)
5. Verify `husako add` CLI still works for plugin git source (manual smoke test or existing integration test)
