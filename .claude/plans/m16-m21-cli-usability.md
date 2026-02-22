# M16-M21: CLI Usability Improvements

## Context

Before these milestones, husako had only 3 commands: `render`, `generate`/`gen`, and `new`. Managing dependencies in `husako.toml` required manual editing, there was no way to check for version updates, no diagnostic tooling for debugging IDE integration issues, and long-running network operations (fetching OpenAPI specs, cloning git repos) produced no visual feedback.

This plan adds **10 new commands** and **progress bars** across 6 milestones, transforming husako from a build tool into a complete project management CLI.

**Before**: `render` | `generate` | `new`
**After**: `render` | `generate` | `new` | `init` | `clean` | `list` | `add` | `remove` | `outdated` | `update` | `info` | `debug` | `validate`

## Architecture Overview

```
+-----------------------------------------------------+
|                    husako-cli                        |
|  Commands enum (13 variants)                        |
|  interactive.rs (dialoguer prompts)                 |
|  progress.rs (IndicatifReporter)                    |
+------------+----------------------------+-----------+
             |                            |
             v                            v
+---------------------+     +--------------------------+
|     husako-core      |     |     husako-config         |
|  init()              |     |  edit.rs (toml_edit)      |
|  clean()             |     |    load_document()        |
|  list_dependencies() |     |    add_resource/chart()   |
|  add_dependency()    |     |    update_*_version()     |
|  remove_dependency() |     |    remove_resource/chart()|
|  check_outdated()    |     |    save_document()        |
|  update_dependencies()|    +--------------------------+
|  project_summary()   |
|  dependency_detail() |     +--------------------------+
|  debug_project()     |     |  husako-core              |
|  validate_file()     |     |  version_check.rs         |
|                      |     |    discover_latest_*()     |
|  progress.rs (trait) |     |    versions_match()       |
+----------------------+     +--------------------------+
```

**Dependency graph between milestones:**

```
M16 (init, clean, list)
M17 (add, remove) --+-- M19 (update) -- M21 (progress bars)
M18 (outdated) -----+
M20 (info, debug, validate)
```

M19 depends on M17 (TOML write-back) and M18 (version discovery). M21 cross-cuts M18 and M19. All others are independent.

## New Workspace Dependencies

```toml
# Cargo.toml [workspace.dependencies]
dialoguer = "0.11"    # Interactive terminal prompts (M16-M17)
console = "0.15"      # Terminal styling (shared by dialoguer/indicatif)
indicatif = "0.17"    # Progress bars and spinners (M21)
toml_edit = "0.22"    # Format-preserving TOML editing (M17)
semver = "1"          # Semantic version comparison (M18)
```

## New Files

| File | Milestone | Purpose |
|------|-----------|---------|
| `crates/husako-config/src/edit.rs` | M17 | Format-preserving TOML editing via `toml_edit::DocumentMut` |
| `crates/husako-core/src/version_check.rs` | M18 | Version discovery from GitHub API, Helm registries, ArtifactHub, git |
| `crates/husako-core/src/progress.rs` | M21 | `ProgressReporter` trait + `SilentProgress` no-op |
| `crates/husako-cli/src/interactive.rs` | M17 | `dialoguer` prompts for `add`, `remove`, `clean` |
| `crates/husako-cli/src/progress.rs` | M21 | `IndicatifReporter` implementation with braille spinners |

## Exit Codes

All new commands reuse existing exit codes. No new exit codes needed.

| Code | Meaning | Commands |
|------|---------|----------|
| 0 | Success | All |
| 1 | I/O error (`GenerateIo`) | clean, update |
| 2 | Config error (`Config`) | list, add, remove, outdated, update, info, debug |
| 6 | Network error (`OpenApi`, `Chart`) | outdated, update |

---

## M16: `husako init` + `husako clean` + `husako list`

**Status**: Complete

**Goal**: Three simple commands that establish the CLI extension pattern for subsequent milestones.

### `husako init`

Initialize husako in the current directory. Complements `husako new <dir>` which creates a new directory.

**Design decision**: Unlike `husako new` which rejects non-empty directories, `init` works in-place, writing only files that don't already exist. This supports the workflow of adding husako to an existing project.

**CLI definition**:

```rust
/// Initialize husako in the current directory
Init {
    /// Project template
    #[arg(long, default_value = "simple")]
    template: TemplateName,
},
```

**Core API**:

```rust
pub struct InitOptions {
    pub directory: PathBuf,  // current working directory
    pub template: TemplateName,
}

pub fn init(options: &InitOptions) -> Result<(), HusakoError>;
```

**Implementation** (`husako-core/src/lib.rs:463-555`):

1. Check if `husako.toml` already exists -- error with guidance to use `husako new <dir>` instead
2. Handle `.gitignore`:
   - If doesn't exist: write standard `.gitignore` from template
   - If exists but doesn't contain `.husako/`: append `.husako/` line
   - If exists and already contains `.husako/`: skip
3. Write `husako.toml` from template
4. Write template entry files only if they don't already exist (non-destructive)
   - Simple: `entry.ts`
   - Project: `env/dev.ts`, `deployments/nginx.ts`, `lib/index.ts`, `lib/metadata.ts`
   - MultiEnv: `base/*`, `dev/main.ts`, `staging/main.ts`, `release/main.ts`

**Edge cases**:
- Non-empty directory: allowed (unlike `new`)
- Existing entry files: preserved (not overwritten)
- Existing `.gitignore` without `.husako/`: appended

**Output**:

```
$ husako init
Created 'simple' project in current directory

$ husako init
Error: husako.toml already exists. Use 'husako new <dir>' to create a new project.
```

### `husako clean`

Remove cache and/or generated types from `.husako/`.

**CLI definition**:

```rust
/// Clean cache and/or generated types
Clean {
    /// Remove cached schemas
    #[arg(long)]
    cache: bool,
    /// Remove generated types
    #[arg(long)]
    types: bool,
    /// Remove both cache and types
    #[arg(long)]
    all: bool,
},
```

**Core API**:

```rust
pub struct CleanOptions {
    pub project_root: PathBuf,
    pub cache: bool,
    pub types: bool,
}

pub struct CleanResult {
    pub cache_removed: bool,
    pub types_removed: bool,
    pub cache_size: u64,   // bytes, measured before deletion
    pub types_size: u64,
}

pub fn clean(options: &CleanOptions) -> Result<CleanResult, HusakoError>;
```

**Implementation** (`husako-core/src/lib.rs:574-600`):

1. If `cache == true` and `.husako/cache/` exists: measure size with `dir_size()`, then `remove_dir_all()`
2. If `types == true` and `.husako/types/` exists: measure size with `dir_size()`, then `remove_dir_all()`
3. Return `CleanResult` with sizes and removal status

**CLI behavior**:
- `husako clean` (no flags): interactive prompt via `dialoguer::Select` -- "What do you want to clean?" [Cache, Types, Both]
- `husako clean --cache`: remove `.husako/cache/` only
- `husako clean --types`: remove `.husako/types/` only
- `husako clean --all`: remove both

**Helper function**: `dir_size()` (`husako-core/src/lib.rs:602-621`) -- recursive directory traversal via `walkdir()`, sums file sizes.

**Output**:

```
$ husako clean --all
Removed .husako/cache/ (12.3 MB)
Removed .husako/types/ (1.8 MB)

$ husako clean --cache
Removed .husako/cache/ (12.3 MB)

$ husako clean --types
.husako/types/ does not exist
```

### `husako list` (alias `ls`)

Show all configured dependencies from `husako.toml`.

**CLI definition**:

```rust
/// List configured dependencies
#[command(alias = "ls")]
List {
    /// Show only resources
    #[arg(long)]
    resources: bool,
    /// Show only charts
    #[arg(long)]
    charts: bool,
},
```

**Core API**:

```rust
pub struct DependencyList {
    pub resources: Vec<DependencyInfo>,
    pub charts: Vec<DependencyInfo>,
}

pub struct DependencyInfo {
    pub name: String,
    pub source_type: &'static str,  // "release", "cluster", "git", "file", "registry", "artifacthub"
    pub version: Option<String>,     // None for file/cluster sources
    pub details: String,             // repo URL, file path, cluster name, etc.
}

pub fn list_dependencies(project_root: &Path) -> Result<DependencyList, HusakoError>;
```

**Implementation** (`husako-core/src/lib.rs:639-725`):

1. Load config via `husako_config::load()`, return empty lists if no config
2. Iterate resources in **sorted order** (by name), map each `SchemaSource` variant to `DependencyInfo` via `resource_info()` helper
3. Iterate charts in **sorted order** (by name), map each `ChartSource` variant to `DependencyInfo` via `chart_info()` helper

**Helper functions**:
- `resource_info()` (`lib.rs:662-692`): Maps `SchemaSource` enum to `DependencyInfo`
- `chart_info()` (`lib.rs:694-725`): Maps `ChartSource` enum to `DependencyInfo`

**Output**:

```
$ husako ls
Resources:
  kubernetes       release      1.35
  cert-manager     git          v1.17.2   https://github.com/.../cert-manager
  my-crds          file         -         ./crds/

Charts:
  ingress-nginx    registry     4.12.0    https://kubernetes.github.io/ingress-nginx
  postgresql       artifacthub  16.4.0    bitnami/postgresql

$ husako ls
No dependencies configured. Use 'husako add' to add one.
```

### M16 Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Add `dialoguer`, `console` to workspace deps |
| `crates/husako-cli/Cargo.toml` | Add `dialoguer`, `console` |
| `crates/husako-cli/src/main.rs` | Add `Init`, `Clean`, `List` variants + handlers |
| `crates/husako-core/src/lib.rs` | Add `init()`, `clean()`, `list_dependencies()` + structs |

### M16 Tests (15 tests)

| Test | Location | What it verifies |
|------|----------|------------------|
| `init_simple_template` | `core/lib.rs:2012` | Simple template creates `husako.toml` + `entry.ts` |
| `init_project_template` | `core/lib.rs:2027` | Project template creates multi-file structure |
| `init_error_if_config_exists` | `core/lib.rs:2041` | Rejects if `husako.toml` already exists |
| `init_works_in_nonempty_dir` | `core/lib.rs:2054` | Allows non-empty directories |
| `init_appends_gitignore` | `core/lib.rs:2069` | Appends `.husako/` to existing `.gitignore` |
| `init_skips_gitignore_if_husako_present` | `core/lib.rs:2085` | Skips `.gitignore` if already contains `.husako/` |
| `clean_cache_only` | `core/lib.rs:2101` | Removes only cache, preserves types |
| `clean_types_only` | `core/lib.rs:2122` | Removes only types, preserves cache |
| `clean_both` | `core/lib.rs:2142` | Removes both cache and types |
| `clean_nothing_exists` | `core/lib.rs:2159` | Returns cleanly when nothing to remove |
| `list_empty_config` | `core/lib.rs:2173` | Empty config returns empty lists |
| `list_resources_only` | `core/lib.rs:2183` | Lists only resources |
| `list_charts_only` | `core/lib.rs:2200` | Lists only charts |
| `list_mixed` | `core/lib.rs:2215` | Lists both resources and charts |
| `list_no_config` | `core/lib.rs:2229` | No config file returns empty lists |

---

## M17: `husako add` + `husako remove`

**Status**: Complete

**Goal**: Add and remove dependencies in `husako.toml` with both interactive (wizard) and non-interactive (flag-based) UX.

### TOML Write-Back (`crates/husako-config/src/edit.rs`)

Format-preserving editing using `toml_edit::DocumentMut`. This module is the foundation for M17 (`add`/`remove`) and M19 (`update`).

**Design decision**: Entries are stored as inline tables for compactness:
```toml
kubernetes = { source = "release", version = "1.35" }
```

**Public API**:

```rust
/// Load the husako.toml as a format-preserving TOML document.
pub fn load_document(project_root: &Path) -> Result<(DocumentMut, PathBuf), ConfigError>;

/// Save the TOML document back to disk.
pub fn save_document(doc: &DocumentMut, path: &Path) -> Result<(), ConfigError>;

/// Add a resource entry to the [resources] section.
pub fn add_resource(doc: &mut DocumentMut, name: &str, source: &SchemaSource);

/// Add a chart entry to the [charts] section.
pub fn add_chart(doc: &mut DocumentMut, name: &str, source: &ChartSource);

/// Update the version of a resource entry. Returns true if found and updated.
pub fn update_resource_version(doc: &mut DocumentMut, name: &str, new_version: &str) -> bool;

/// Update the version of a chart entry. Returns true if found and updated.
pub fn update_chart_version(doc: &mut DocumentMut, name: &str, new_version: &str) -> bool;

/// Remove a resource entry. Returns true if found and removed.
pub fn remove_resource(doc: &mut DocumentMut, name: &str) -> bool;

/// Remove a chart entry. Returns true if found and removed.
pub fn remove_chart(doc: &mut DocumentMut, name: &str) -> bool;
```

**Internal helpers**:

- `ensure_table()` (`edit.rs:86-90`): Creates an empty `[resources]` or `[charts]` section if it doesn't exist
- `update_version_in_item()` (`edit.rs:92-116`): Handles both inline tables (`Item::InlineTable`) and standard tables (`Item::Table`); updates `version` field first, falls back to `tag` field (for git sources)
- `source_to_inline_table()` (`edit.rs:118-143`): Converts `SchemaSource` enum to `toml_edit::InlineTable`
- `chart_source_to_inline_table()` (`edit.rs:145-175`): Converts `ChartSource` enum to `toml_edit::InlineTable`

**Key guarantee**: Comments, whitespace, and line structure are preserved across load/edit/save cycles. The `toml_edit` crate maintains a CST (Concrete Syntax Tree) rather than an AST.

### `husako add`

**CLI definition**:

```rust
/// Add a resource or chart dependency
Add {
    /// Dependency name
    name: Option<String>,
    /// Add as resource
    #[arg(long, group = "kind")]
    resource: bool,
    /// Add as chart
    #[arg(long, group = "kind")]
    chart: bool,
    /// Source type (release, cluster, git, file, registry, artifacthub)
    #[arg(long)]
    source: Option<String>,
    /// Version
    #[arg(long)]
    version: Option<String>,
    /// Repository URL (for git, registry sources)
    #[arg(long)]
    repo: Option<String>,
    /// Git tag (for git source)
    #[arg(long)]
    tag: Option<String>,
    /// Path (for file, git sources)
    #[arg(long)]
    path: Option<String>,
    /// Chart name in repository (for registry source)
    #[arg(long)]
    chart_name: Option<String>,
    /// ArtifactHub package (for artifacthub source)
    #[arg(long)]
    package: Option<String>,
},
```

**Core API**:

```rust
pub enum AddTarget {
    Resource { name: String, source: SchemaSource },
    Chart { name: String, source: ChartSource },
}

pub fn add_dependency(project_root: &Path, target: &AddTarget) -> Result<(), HusakoError>;
```

**Implementation** (`husako-core/src/lib.rs:741-755`):

1. Load TOML document via `edit::load_document()` (format-preserving)
2. Dispatch: `AddTarget::Resource` -> `edit::add_resource()`, `AddTarget::Chart` -> `edit::add_chart()`
3. Save document via `edit::save_document()` (preserves comments/formatting)

**CLI dispatch logic** (`main.rs:460-498`):

```
if name AND source are provided:
    -> Non-interactive: build target from flags via build_resource_target() / build_chart_target()
else:
    -> Interactive: call interactive::prompt_add()
```

**Interactive mode** (missing required args):

```
$ husako add
? Dependency type: [Resource, Chart]
> Chart
? Source type: [registry, artifacthub, git, file]
> registry
? Name: ingress-nginx
? Repository URL: https://kubernetes.github.io/ingress-nginx
? Chart name in repository: ingress-nginx
? Version: 4.12.0
Added 'ingress-nginx' to [charts]
```

**Non-interactive mode** (all flags):

```
$ husako add ingress-nginx --chart --source registry \
    --repo https://kubernetes.github.io/ingress-nginx \
    --chart-name ingress-nginx --version 4.12.0
Added 'ingress-nginx' to [charts]
```

**CLI helper functions**:

- `build_resource_target()` (`main.rs:889-924`): Validates required flags per source type (release needs `--version`, git needs `--repo`+`--tag`+`--path`, etc.)
- `build_chart_target()` (`main.rs:927-968`): Same validation for chart source types

### `husako remove` (alias `rm`)

**CLI definition**:

```rust
/// Remove a resource or chart dependency
#[command(alias = "rm")]
Remove {
    /// Dependency name to remove
    name: Option<String>,
},
```

**Core API**:

```rust
pub struct RemoveResult {
    pub name: String,
    pub section: &'static str, // "resources" or "charts"
}

pub fn remove_dependency(project_root: &Path, name: &str) -> Result<RemoveResult, HusakoError>;
```

**Implementation** (`husako-core/src/lib.rs:763-785`):

1. Load TOML document
2. Try `edit::remove_resource(name)` -- if found, return `RemoveResult { section: "resources" }`
3. If not found, try `edit::remove_chart(name)` -- if found, return `RemoveResult { section: "charts" }`
4. If neither, error: `"dependency '{name}' not found in resources or charts"`
5. Save document

**CLI behavior**: If no `name` arg, interactive prompt lists all deps via `dialoguer::Select`:

```
$ husako rm
? Which dependency to remove?
> ingress-nginx (chart, registry)
  kubernetes (resource, release)
Removed 'ingress-nginx' from [charts]
```

### Interactive Prompts (`crates/husako-cli/src/interactive.rs`)

Uses `dialoguer::Select` for choices and `dialoguer::Input` for text input.

| Function | Signature | Purpose |
|----------|-----------|---------|
| `prompt_add()` | `() -> Result<AddTarget, String>` | Top-level wizard: resource or chart? |
| `prompt_add_resource()` | `() -> Result<AddTarget, String>` | Resource source selection + details |
| `prompt_add_chart()` | `() -> Result<AddTarget, String>` | Chart source selection + details |
| `prompt_remove()` | `(&[(String, &str, &str)]) -> Result<String, String>` | Select dependency from list |
| `prompt_clean()` | `() -> Result<(bool, bool), String>` | Select what to clean: Cache / Types / Both |

**Prompt flow for `prompt_add_resource()`** (`interactive.rs:21-82`):

1. `Select` source type: release (0), cluster (1), git (2), file (3)
2. `Input` dependency name
3. Source-specific prompts:
   - **release**: `Input` version (e.g., "1.35")
   - **cluster**: `Input` cluster name (optional, `allow_empty=true`)
   - **git**: `Input` repo URL, tag, path to CRDs
   - **file**: `Input` path to YAML file/directory

**Prompt flow for `prompt_add_chart()`** (`interactive.rs:84-154`):

1. `Select` source type: registry (0), artifacthub (1), git (2), file (3)
2. `Input` chart name
3. Source-specific prompts:
   - **registry**: `Input` repo URL, chart name in repo, version
   - **artifacthub**: `Input` package name (e.g., "bitnami/postgresql"), version
   - **git**: `Input` repo URL, tag, path to chart
   - **file**: `Input` path to `values.schema.json`

### M17 Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Add `toml_edit` to workspace deps |
| `crates/husako-config/Cargo.toml` | Add `toml_edit` |
| `crates/husako-config/src/lib.rs` | Export `edit` module |
| `crates/husako-config/src/edit.rs` | **NEW**: TOML write-back |
| `crates/husako-cli/src/main.rs` | Add `Add`, `Remove` variants + handlers |
| `crates/husako-cli/src/interactive.rs` | **NEW**: dialoguer prompts |
| `crates/husako-core/src/lib.rs` | Add `add_dependency()`, `remove_dependency()` + types |

### M17 Tests (17 tests)

**edit.rs** (12 tests):

| Test | Location | What it verifies |
|------|----------|------------------|
| `add_resource_release` | `edit.rs:189` | Adds release resource, creates `[resources]` section |
| `add_chart_registry` | `edit.rs:209` | Adds registry chart, creates `[charts]` section |
| `remove_resource_existing` | `edit.rs:230` | Removes existing resource entry |
| `remove_resource_missing` | `edit.rs:242` | Returns false for nonexistent entry |
| `remove_chart_existing` | `edit.rs:250` | Removes existing chart entry |
| `update_resource_version` | `edit.rs:262` | Updates `version` field in resource |
| `update_chart_version` | `edit.rs:279` | Updates `version` field in chart |
| `update_git_tag` | `edit.rs:296` | Updates `tag` field (not `version`) for git source |
| `preserve_comments` | `edit.rs:313` | Comments survive load/edit/save cycle |
| `round_trip` | `edit.rs:328` | Unmodified document round-trips identically |
| `load_document_missing` | `edit.rs:339` | Returns error for missing config |
| `load_and_save_round_trip` | `edit.rs:346` | Save produces identical content to input |

**core/lib.rs** (5 tests):

| Test | Location | What it verifies |
|------|----------|------------------|
| `add_resource_creates_entry` | `core/lib.rs:2240` | End-to-end resource addition |
| `add_chart_creates_entry` | `core/lib.rs:2259` | End-to-end chart addition |
| `remove_resource_from_config` | `core/lib.rs:2279` | End-to-end resource removal |
| `remove_chart_from_config` | `core/lib.rs:2295` | End-to-end chart removal |
| `remove_nonexistent_returns_error` | `core/lib.rs:2308` | Error for missing dependency |

---

## M18: `husako outdated`

**Status**: Complete

**Goal**: Check which dependencies have newer versions available. Analogous to `npm outdated` or `cargo outdated`.

### Version Discovery (`crates/husako-core/src/version_check.rs`)

Each source type uses a different discovery mechanism:

| Source | Discovery Method | Returns |
|--------|-----------------|---------|
| `release` | GitHub API: `GET /repos/kubernetes/kubernetes/tags?per_page=100` | `"1.36"` (major.minor only) |
| `registry` | Fetch `{repo}/index.yaml`, parse YAML, find chart entries | `"4.12.1"` (full semver) |
| `artifacthub` | `GET https://artifacthub.io/api/v1/packages/helm/{package}` | `"16.5.1"` (from `version` field) |
| `git` | `git ls-remote --tags --sort=-v:refname {repo}` | `"v1.18.0"` (highest stable tag) |
| `file` / `cluster` | Skipped (no version concept) | -- |

**Public API**:

```rust
/// Discover the latest stable Kubernetes release version from GitHub API.
pub fn discover_latest_release() -> Result<String, HusakoError>;

/// Discover the latest version from a Helm chart registry's index.yaml.
pub fn discover_latest_registry(repo: &str, chart: &str) -> Result<String, HusakoError>;

/// Discover the latest version from ArtifactHub API.
pub fn discover_latest_artifacthub(package: &str) -> Result<String, HusakoError>;

/// Discover the latest tag from a git repository using `git ls-remote --tags`.
pub fn discover_latest_git_tag(repo: &str) -> Result<Option<String>, HusakoError>;

/// Compare two version strings for equivalence.
pub fn versions_match(current: &str, latest: &str) -> bool;
```

**Implementation details**:

- **`discover_latest_release()`** (`version_check.rs:4-41`):
  - Fetches 100 tags from GitHub API
  - Filters: skips pre-release tags containing `-` (alpha, beta, rc)
  - Parses with `semver::Version`, tracks highest
  - Returns `major.minor` format only (e.g., "1.35" not "1.35.0") -- matches husako config convention

- **`discover_latest_registry()`** (`version_check.rs:44-87`):
  - Fetches `{repo}/index.yaml` (standard Helm repository format)
  - Parses YAML, navigates `entries[chart]` (array of version objects)
  - Filters stable versions: `v.pre.is_empty()`
  - Returns full semver string (e.g., "4.12.1")

- **`discover_latest_artifacthub()`** (`version_check.rs:90-117`):
  - HTTP GET to ArtifactHub REST API
  - Extracts `data["version"]` field -- ArtifactHub returns latest version by default when no version query param is specified

- **`discover_latest_git_tag()`** (`version_check.rs:120-156`):
  - Runs `git ls-remote --tags --sort=-v:refname {repo}`
  - Parses tab-separated output, strips `refs/tags/` prefix and `^{}` suffix
  - Filters stable semver (no pre-release), returns highest
  - Returns `Option<String>` -- `None` if no semver tags found

- **`versions_match()`** (`version_check.rs:162-177`):
  - Exact match: `current == latest`
  - Prefix match: if `current` is major.minor only (e.g., "1.35"), matches if `latest` starts with it (e.g., "1.35.0", "1.35.1")
  - Strips `v` prefix for comparison

All discovery functions use `reqwest::blocking::Client` with `user_agent("husako")`.

### `husako outdated` Command

**CLI definition**:

```rust
/// Check for outdated dependencies
Outdated,
```

**Core API**:

```rust
pub struct OutdatedEntry {
    pub name: String,
    pub kind: &'static str,        // "resource" or "chart"
    pub source_type: &'static str, // "release", "git", "registry", "artifacthub"
    pub current: String,
    pub latest: Option<String>,     // None if discovery failed
    pub up_to_date: bool,
}

pub fn check_outdated(
    project_root: &Path,
    progress: &dyn ProgressReporter,
) -> Result<Vec<OutdatedEntry>, HusakoError>;
```

**Implementation** (`husako-core/src/lib.rs:799-983`):

1. Load config
2. For each resource (sorted by name):
   - Start progress task: "Checking {name}..."
   - Dispatch discovery by source type
   - On success: compare with `versions_match()`, finish with ok/err
   - On network failure: `finish_err()`, set `latest: None`, continue (don't abort)
3. Same for each chart
4. Return all entries

**Error handling**: Network errors are per-entry, not fatal. Failed entries show "?" in the Latest column.

**Output**:

```
$ husako outdated
Name             Kind       Source       Current    Latest
kubernetes       resource   release      1.35       1.36
cert-manager     resource   git          v1.17.2    v1.18.0
ingress-nginx    chart      registry     4.12.0     4.12.0     ✓
postgresql       chart      artifacthub  16.4.0     16.5.1
```

### M18 Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Add `semver` to workspace deps |
| `crates/husako-core/Cargo.toml` | Add `semver`, `reqwest`, `serde`, `serde_yaml_ng` |
| `crates/husako-core/src/version_check.rs` | **NEW**: Version discovery |
| `crates/husako-core/src/lib.rs` | Add `check_outdated()`, export `version_check` |
| `crates/husako-cli/src/main.rs` | Add `Outdated` variant + handler |

### M18 Tests (4 tests)

| Test | Location | What it verifies |
|------|----------|------------------|
| `versions_match_exact` | `version_check.rs:184` | Exact string match |
| `versions_match_prefix` | `version_check.rs:190` | "1.35" matches "1.35.0" and "1.35.1" |
| `versions_no_match` | `version_check.rs:196` | "1.35" does not match "1.36" |
| `versions_match_v_prefix` | `version_check.rs:202` | `v` prefix handling |

---

## M19: `husako update`

**Status**: Complete

**Goal**: Update versioned dependencies to their latest versions and auto-regenerate types. Combines M17's TOML write-back with M18's version discovery.

### `husako update` Command

**CLI definition**:

```rust
/// Update dependencies to latest versions
Update {
    /// Update a specific dependency by name
    name: Option<String>,
    /// Update only resources
    #[arg(long)]
    resources_only: bool,
    /// Update only charts
    #[arg(long)]
    charts_only: bool,
    /// Show what would be updated without making changes
    #[arg(long)]
    dry_run: bool,
},
```

**Core API**:

```rust
pub struct UpdateOptions {
    pub project_root: PathBuf,
    pub name: Option<String>,
    pub resources_only: bool,
    pub charts_only: bool,
    pub dry_run: bool,
}

pub struct UpdatedEntry {
    pub name: String,
    pub kind: String,
    pub old_version: String,
    pub new_version: String,
}

pub struct UpdateResult {
    pub updated: Vec<UpdatedEntry>,
    pub skipped: Vec<String>,
    pub failed: Vec<(String, String)>,
}

pub fn update_dependencies(
    options: &UpdateOptions,
    progress: &dyn ProgressReporter,
) -> Result<UpdateResult, HusakoError>;
```

**Implementation** (`husako-core/src/lib.rs:1011-1111`):

1. Call `check_outdated()` to get all entries with latest versions
2. Filter entries by:
   - `options.name` -- if set, only update the named dependency
   - `options.resources_only` / `options.charts_only` -- filter by kind
3. **Lazy-load TOML document**: only if at least one entry needs updating (avoids unnecessary file I/O)
4. For each filtered entry:
   - Skip if `up_to_date == true` -> add to `skipped`
   - Skip if `latest` is `None` -> add to `failed`
   - If `dry_run`: add to `updated` without writing
   - Otherwise: call `edit::update_resource_version()` or `edit::update_chart_version()`
5. Save TOML document if any changes were made
6. **Auto-regenerate types**: if any updates occurred and not `dry_run`, call `generate()` with a progress task

**Design decision**: Auto-regeneration after update ensures types stay in sync with config. This mirrors `npm install` behavior where the lock file and `node_modules` stay consistent.

**Output**:

```
$ husako update
Checking for updates...
Updated kubernetes: 1.35 -> 1.36 (resource)
Updated ingress-nginx: 4.12.0 -> 4.12.1 (chart)
Skipped: postgresql (up to date)
Regenerating types...
Done.

$ husako update ingress-nginx
Updated ingress-nginx: 4.12.0 -> 4.12.1 (chart)
Regenerating types...
Done.

$ husako update --dry-run
Would update kubernetes: 1.35 -> 1.36 (resource)
Would update ingress-nginx: 4.12.0 -> 4.12.1 (chart)
```

### M19 Files Changed

| File | Change |
|------|--------|
| `crates/husako-core/src/lib.rs` | Add `update_dependencies()` + types |
| `crates/husako-cli/src/main.rs` | Add `Update` variant + handler |

### M19 Tests

No dedicated M19 tests; `update_dependencies()` is tested implicitly through the `check_outdated()` and `edit::update_*_version()` tests. Manual E2E verification covers the full flow.

---

## M20: `husako info` + `husako debug` + `husako validate`

**Status**: Complete

**Goal**: Diagnostic, inspection, and validation commands for project health and CI integration.

### `husako info`

Project summary or detailed dependency information.

**CLI definition**:

```rust
/// Show project summary or dependency details
Info {
    /// Dependency name (omit for project summary)
    name: Option<String>,
},
```

**Core API**:

```rust
pub struct ProjectSummary {
    pub project_root: PathBuf,
    pub config_valid: bool,
    pub resources: Vec<DependencyInfo>,
    pub charts: Vec<DependencyInfo>,
    pub cache_size: u64,
    pub type_file_count: usize,
    pub types_size: u64,
}

pub struct DependencyDetail {
    pub info: DependencyInfo,
    pub cache_path: Option<PathBuf>,
    pub cache_size: u64,
    pub type_files: Vec<(PathBuf, u64)>,
    pub schema_property_count: Option<(usize, usize)>,  // (total, top-level) for charts
    pub group_versions: Vec<(String, Vec<String>)>,      // (gv, [kinds]) for resources
}

pub fn project_summary(project_root: &Path) -> Result<ProjectSummary, HusakoError>;
pub fn dependency_detail(project_root: &Path, name: &str) -> Result<DependencyDetail, HusakoError>;
```

**Implementation -- `project_summary()`** (`husako-core/src/lib.rs:1126-1159`):

1. Load config, record `config_valid`
2. Call `list_dependencies()` for resource/chart lists
3. Compute `cache_size` via `dir_size(.husako/cache/)`
4. Compute `type_file_count` and `types_size` via `count_files_and_size(.husako/types/)`

**Implementation -- `dependency_detail()`** (`husako-core/src/lib.rs:1171-1221`):

1. Load config, error if missing
2. Search resources first (return early if found), then charts
3. For resources:
   - Map to `DependencyInfo` via `resource_info()`
   - List type files from `.husako/types/k8s/` via `list_type_files()`
   - Read group-versions via `read_group_versions()` -- parses `.d.ts` filenames, converts `__` separator to `/` (e.g., `apps__v1.d.ts` -> `apps/v1`)
   - Compute cache info via `resource_cache_info()` -- for Release source, path is `.husako/cache/release/v{version}.0/`
4. For charts:
   - Map to `DependencyInfo` via `chart_info()`
   - List type files from `.husako/types/helm/{name}.*`
   - Count schema properties via `read_chart_schema_props()` -- parses `.d.ts` to count property lines
5. Error if name not found in either section

**Helper functions**:

| Function | Location | Purpose |
|----------|----------|---------|
| `count_files_and_size()` | `lib.rs:1223-1242` | Recursive file count + total bytes |
| `list_type_files()` | `lib.rs:1244-1257` | List `.d.ts`/`.js` files in directory |
| `list_chart_type_files()` | `lib.rs:1259-1268` | List files matching `{chart_name}.*` |
| `read_group_versions()` | `lib.rs:1270-1293` | Parse `.d.ts` filenames -> group-version names |
| `resource_cache_info()` | `lib.rs:1295-1308` | Cache path for Release source |
| `read_chart_schema_props()` | `lib.rs:1317-1337` | Count properties in `.d.ts` (total vs top-level) |

**Output -- project summary**:

```
$ husako info
Project: /Users/syr/my-project
Config:  husako.toml (valid)

Resources (2):
  kubernetes       release      1.35
  cert-manager     git          v1.17.2

Charts (1):
  ingress-nginx    registry     4.12.0

Cache:   .husako/cache/ (23.4 MB)
Types:   .husako/types/ (45 files, 1.8 MB)
```

**Output -- dependency detail** (resource):

```
$ husako info kubernetes
kubernetes (resource)
  Source:  release
  Version: 1.35

  Cache:   .husako/cache/release/v1.35.0/ (16.5 MB)
  Types:   .husako/types/k8s/ (30 .d.ts + 30 .js)

  Group-Versions (30):
    v1              Pod, Service, ConfigMap, ...
    apps/v1         Deployment, StatefulSet, ...
    batch/v1        Job, CronJob
```

### `husako debug`

Health check for the project setup. Answers "why isn't my IDE working?"

**CLI definition**:

```rust
/// Check project health and diagnose issues
Debug,
```

**Core API**:

```rust
pub struct DebugReport {
    pub config_ok: Option<bool>,    // None if no config file, Some(true/false)
    pub types_exist: bool,
    pub type_file_count: usize,
    pub tsconfig_ok: bool,
    pub tsconfig_has_paths: bool,
    pub stale: bool,                // husako.toml newer than .husako/types/
    pub cache_size: u64,
    pub issues: Vec<String>,        // human-readable problems
    pub suggestions: Vec<String>,   // actionable fix suggestions
}

pub fn debug_project(project_root: &Path) -> Result<DebugReport, HusakoError>;
```

**Implementation** (`husako-core/src/lib.rs:1354-1448`):

5 checks performed sequentially:

1. **Config check**: Load and parse `husako.toml`. If missing -> `config_ok = None`, add issue. If parse error -> `config_ok = Some(false)`, add issue.
2. **Types check**: Check `.husako/types/` exists and count files. If missing or empty -> add issue with suggestion to run `husako generate`.
3. **tsconfig check**: Parse `tsconfig.json` (using `strip_jsonc()` for JSONC support). Check for `compilerOptions.paths.husako` and `compilerOptions.paths["k8s/*"]` using JSON pointer: `/compilerOptions/paths/husako` and `/compilerOptions/paths/k8s~1*` (RFC 6901 `~1` encodes `/`).
4. **Staleness check**: Compare `husako.toml` modification time vs `.husako/types/` modification time. If config is newer -> `stale = true`, suggest regeneration.
5. **Cache size**: Compute total cache size for reporting.

**Output**:

```
$ husako debug
✓ husako.toml found and valid
✓ .husako/types/ exists (45 type files)
✓ tsconfig.json has husako path mappings
✗ Types may be stale (husako.toml newer than .husako/types/)
  -> Run 'husako generate' to update

✓ .husako/cache/ exists (23.4 MB)
```

### `husako validate`

Compile and validate TypeScript files without rendering YAML output. Designed for CI pipelines.

**CLI definition**:

```rust
/// Validate TypeScript without rendering output
Validate {
    /// TypeScript entry file or alias
    file: String,
},
```

**Core API**:

```rust
pub struct ValidateResult {
    pub resource_count: usize,
    pub validation_errors: Vec<String>,
}

pub fn validate_file(
    source: &str,
    filename: &str,
    options: &RenderOptions,
) -> Result<ValidateResult, HusakoError>;
```

**Implementation** (`husako-core/src/lib.rs:1458-1507`):

Reuses the existing `render()` pipeline but skips YAML emission:

1. **Compile**: `husako_compile_oxc::compile(source, filename)` -> JS
2. **Execute**: `husako_runtime_qjs::execute(js, options)` -> `serde_json::Value`
   - Compilation and runtime errors propagate as `HusakoError` (hard failures)
3. **Count resources**: array length or 1 if single object
4. **Validate**: `validate::validate(value, schema_store)` -> collect errors as strings
   - Validation errors are **collected, not thrown** -- returned in `ValidateResult.validation_errors`

**Design decision**: Compilation and runtime errors are hard failures (the user's code is broken). Validation errors are soft results (the code runs but produces invalid manifests). This distinction lets CI pipelines differentiate "can't compile" from "produces bad YAML".

**Output**:

```
$ husako validate env/dev.ts
✓ env/dev.ts: 3 resources, 0 validation errors

$ husako validate env/broken.ts
error: compile error in env/broken.ts:15
  TypeError: deployment is not a function
```

### M20 Files Changed

| File | Change |
|------|--------|
| `crates/husako-core/src/lib.rs` | Add `project_summary()`, `dependency_detail()`, `debug_project()`, `validate_file()` + types |
| `crates/husako-cli/src/main.rs` | Add `Info`, `Debug`, `Validate` variants + handlers |

### M20 Tests (11 tests)

| Test | Location | What it verifies |
|------|----------|------------------|
| `project_summary_empty` | `core/lib.rs:2319` | Empty project returns zero counts |
| `project_summary_with_deps` | `core/lib.rs:2330` | Deps listed with correct metadata |
| `debug_missing_config` | `core/lib.rs:2343` | Issues list includes missing config |
| `debug_valid_project` | `core/lib.rs:2353` | All checks pass on valid project |
| `debug_missing_types` | `core/lib.rs:2374` | Issues list includes missing types |
| `validate_valid_ts` | `core/lib.rs:2384` | Valid TS returns resource count, no errors |
| `validate_compile_error` | `core/lib.rs:2396` | Compile error propagates |
| `validate_runtime_error` | `core/lib.rs:2404` | Runtime error propagates |
| `dependency_detail_not_found` | `core/lib.rs:2411` | Error for unknown dependency |
| `dependency_detail_resource` | `core/lib.rs:2420` | Resource detail has correct metadata |
| `dependency_detail_chart` | `core/lib.rs:2435` | Chart detail has correct metadata |

---

## M21: Progress Bars

**Status**: Complete

**Goal**: Visual feedback for network operations and long-running tasks using terminal spinners.

### Design: Decoupled Callback Trait

Core defines the trait; CLI implements with `indicatif`; tests use no-op. This avoids pulling `indicatif` into the core library and keeps tests fast and deterministic.

**`crates/husako-core/src/progress.rs`**:

```rust
/// Trait for reporting progress of long-running operations.
pub trait ProgressReporter: Send + Sync {
    fn start_task(&self, message: &str) -> Box<dyn TaskHandle>;
}

/// Handle for an in-progress task.
pub trait TaskHandle: Send + Sync {
    fn set_message(&self, message: &str);
    fn finish_ok(&self, message: &str);
    fn finish_err(&self, message: &str);
}

/// No-op reporter for tests and non-interactive use.
pub struct SilentProgress;

impl ProgressReporter for SilentProgress {
    fn start_task(&self, _message: &str) -> Box<dyn TaskHandle> {
        Box::new(SilentTask)
    }
}
```

**`crates/husako-cli/src/progress.rs`**:

```rust
pub struct IndicatifReporter;

impl ProgressReporter for IndicatifReporter {
    fn start_task(&self, message: &str) -> Box<dyn TaskHandle> {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
        );
        pb.set_message(message.to_string());
        pb.enable_steady_tick(Duration::from_millis(80));
        Box::new(IndicatifTaskHandle { pb: Arc::new(pb) })
    }
}
```

**Spinner completion**:
- `finish_ok()`: replaces spinner with `✓ {message}` (checkmark)
- `finish_err()`: replaces spinner with `✗ {message}` (X mark)

### Signature Changes

Functions that perform network I/O now accept a progress reporter:

```rust
// husako-core/src/lib.rs
pub fn generate(options: &GenerateOptions, progress: &dyn ProgressReporter) -> ...;

// husako-core/src/schema_source.rs
pub fn resolve_all(
    config: &HusakoConfig,
    cache_dir: &Path,
    progress: &dyn ProgressReporter,
) -> Result<HashMap<String, Value>, HusakoError>;

// husako-core/src/lib.rs
pub fn check_outdated(project_root: &Path, progress: &dyn ProgressReporter) -> ...;
pub fn update_dependencies(options: &UpdateOptions, progress: &dyn ProgressReporter) -> ...;
```

**Test compatibility**: All existing tests pass `&SilentProgress` -- zero overhead, no test changes needed.

### Where Progress Is Shown

| Operation | Message | Completion |
|-----------|---------|------------|
| `generate` -- fetch release specs | "Fetching kubernetes v1.35.0..." | "✓ kubernetes: 42 group-versions" |
| `generate` -- clone git repo | "Cloning cert-manager (v1.17.2)..." | "✓ cert-manager: 5 CRDs" |
| `generate` -- fetch chart schema | "Fetching ingress-nginx chart schema..." | "✓ ingress-nginx: values.schema.json" |
| `generate` -- type generation | "Generating types..." | "✓ Generated types in .husako/types/" |
| `outdated` -- per-entry check | "Checking kubernetes..." | "✓ kubernetes: 1.35 -> 1.36" |
| `update` -- version check | "Checking for updates..." | "✓ Found N updates" |
| `update` -- regenerate | "Regenerating types..." | "✓ Types updated" |

**UX**:

```
$ husako generate
⠋ Fetching kubernetes v1.35.0...
✓ kubernetes: 42 group-versions
⠋ Fetching ingress-nginx chart schema...
✓ ingress-nginx: values.schema.json
✓ Generated types in .husako/types/
✓ Updated tsconfig.json
```

### M21 Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Add `indicatif` to workspace deps |
| `crates/husako-core/src/progress.rs` | **NEW**: trait + `SilentProgress` |
| `crates/husako-core/src/lib.rs` | Export `progress`, update `generate()` signature |
| `crates/husako-core/src/schema_source.rs` | Accept `&dyn ProgressReporter` |
| `crates/husako-cli/Cargo.toml` | Add `indicatif` |
| `crates/husako-cli/src/progress.rs` | **NEW**: `IndicatifReporter` |
| `crates/husako-cli/src/main.rs` | Create reporter, pass to functions |

### M21 Tests (2 tests)

| Test | Location | What it verifies |
|------|----------|------------------|
| `silent_progress_no_ops` | `progress.rs:35` | SilentProgress start + finish_ok works |
| `silent_progress_err` | `progress.rs:43` | SilentProgress start + finish_err works |

---

## Complete Test Summary

**46 new tests** added across M16-M21, bringing the total from 319 to **365**.

| Location | Count | Milestones |
|----------|-------|------------|
| `husako-core/src/lib.rs` | 28 | M16 (15), M17 (5), M20 (8) |
| `husako-config/src/edit.rs` | 12 | M17 |
| `husako-core/src/version_check.rs` | 4 | M18 |
| `husako-core/src/progress.rs` | 2 | M21 |

## Verification

```bash
# Full verification suite
cargo build && cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check

# Manual E2E per milestone
husako init                                                        # M16
husako clean --all                                                 # M16
husako ls                                                          # M16
husako add kubernetes --resource --source release --version 1.35   # M17
husako rm kubernetes                                               # M17
husako add                                                         # M17 (interactive)
husako outdated                                                    # M18
husako update --dry-run                                            # M19
husako update                                                      # M19
husako info                                                        # M20
husako info kubernetes                                             # M20
husako debug                                                       # M20
husako validate entry.ts                                           # M20
husako generate                                                    # M21 (verify spinners)
```
