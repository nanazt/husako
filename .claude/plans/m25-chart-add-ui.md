# M25: Chart Add — Name + Version Selection UI

**Status:** Completed

**Goal:** When adding a chart via `husako add`, show a Name prompt (pre-filled, customizable) followed by a version/tag selection list for all versioned sources.

**Architecture:** Add `discover_artifacthub_versions()` and `discover_git_tags()` to `version_check.rs`. Create a shared `prompt_version_select()` helper in `interactive.rs` (extracts the duplicated pattern from `prompt_release_version` / `prompt_registry_version`). Refactor each chart source flow to use Name + version list.

**Tech Stack:** `reqwest` (ArtifactHub API), `semver` (version sorting), `git ls-remote` (tag discovery), `text_input::run` (dim placeholder Name prompt), `dialoguer::Select` (version list)

---

## Bug Fix (from M24)

Reordered chart source items from `["registry", "artifacthub", "git", "file"]` to `["artifacthub", "registry", "git", "file"]` (ArtifactHub as default).

## Flow Changes

| Source | Before | After |
|--------|--------|-------|
| **ArtifactHub Search** | Search -> Package select -> Done (auto name+version) | Search -> Package select -> **Name** (pre-filled) -> **Version list** -> Done |
| **ArtifactHub Manual** | Package -> Version text -> Done | Package -> **Name** (pre-filled) -> **Version list** -> Done |
| **Registry** | Repo URL -> Chart name -> Version list -> Done | Repo URL -> Chart name -> **Name** (pre-filled) -> Version list -> Done |
| **Git** | Name -> Repo URL -> Tag text -> Path -> Done | Repo URL -> **Name** (pre-filled from repo) -> **Tag list** -> Path -> Done |

---

## Tasks

### Task 1: `discover_artifacthub_versions()` in `version_check.rs`

**File:** `crates/husako-core/src/version_check.rs`

```rust
pub fn discover_artifacthub_versions(
    package: &str,
    limit: usize,
) -> Result<Vec<String>, HusakoError>
```

- Same API endpoint as `discover_latest_artifacthub`: `GET /api/v1/packages/helm/{package}`
- Parse `data["available_versions"]` array of `{version, prerelease, ts}` objects
- Filter: skip entries where `prerelease == true`
- Already sorted newest-first by API; truncate to `limit`
- Return `Vec<String>` of version strings

### Task 2: `discover_git_tags()` in `version_check.rs`

**File:** `crates/husako-core/src/version_check.rs`

```rust
pub fn discover_git_tags(repo: &str, limit: usize) -> Result<Vec<String>, HusakoError>
```

- Same `git ls-remote --tags --sort=-v:refname` as `discover_latest_git_tag`
- Same parsing (strip `refs/tags/`, `^{}`; parse semver; filter `pre.is_empty()`)
- Deduplicate, sort descending, truncate to `limit`
- Return tag strings (preserving `v` prefix)

### Task 3: `is_valid_name()` + `prompt_version_select()` in `interactive.rs`

**File:** `crates/husako-cli/src/interactive.rs`

- Extract `is_valid_name(&str)` from `validate_name(&String)` for use with `text_input::run`
- Create shared `prompt_version_select(fetch_label, prompt_label, fetch_fn)` helper
- Refactor `prompt_release_version()` to delegate to `prompt_version_select()`
- Remove `prompt_registry_version()` — callers use `prompt_version_select` directly

### Task 4: Refactor chart flows

- Reorder items: `["artifacthub", "registry", "git", "file"]`
- ArtifactHub search: add Name (pre-filled from `pkg.name`) + Version list
- ArtifactHub manual: add Name (pre-filled from package path) + Version list
- Registry: add Name (pre-filled from chart name) between chart input and version select
- Git: reorder to Repo URL -> Name (pre-filled from repo basename) -> Tag list -> Path

### Task 5: Tests

- `is_valid_name_works_with_str` — validates `&str` version
- `artifacthub_versions_filtering` — mock JSON, verify prerelease filtering + limit
- `git_tags_multiple` — mock `ls-remote` output, verify multi-tag parsing + sorting

---

## Files Changed

| File | Action |
|------|--------|
| `crates/husako-core/src/version_check.rs` | +`discover_artifacthub_versions`, +`discover_git_tags`, +2 tests |
| `crates/husako-cli/src/interactive.rs` | +`is_valid_name`, +`prompt_version_select`, refactored all chart flows, +1 test |

## Result

- 387 tests passing (+3 new)
- Commit: `2cfad6a feat: add chart name prompt and version selection UI (M25)`
