# M23: Search, Autocomplete & Interactive Polish

**Status**: Complete

## M23a: ArtifactHub Search

Added `search_artifacthub(query, offset)` to `version_check.rs` with paginated results. Interactive flow in `husako add --chart` offers "Search ArtifactHub" or "Enter manually". Search results are displayed using a custom `search_select` widget with infinite scroll — auto-loads more results as the cursor approaches the bottom. Selected item is highlighted with cyan bold text and `>` prefix. Navigation does not wrap around.

### Files

| File | Action |
|------|--------|
| `crates/husako-core/src/version_check.rs` | Added `ArtifactHubPackage`, `ArtifactHubSearchResult`, `search_artifacthub()` |
| `crates/husako-cli/src/search_select.rs` | Created custom scrollable selector with infinite scroll, cyan highlighting, no wrap-around |
| `crates/husako-cli/src/interactive.rs` | Added `prompt_artifacthub_chart()` with search flow using `search_select::run()` |

## M23b: Smart Version Selection

Added `discover_recent_releases(limit)` and `discover_registry_versions(repo, chart, limit)` to `version_check.rs`. Interactive prompts show version lists with "(latest)" tag and "Enter manually" fallback.

### Files

| File | Action |
|------|--------|
| `crates/husako-core/src/version_check.rs` | Added `discover_recent_releases()`, `discover_registry_versions()` |
| `crates/husako-cli/src/interactive.rs` | Added `prompt_release_version()`, `prompt_registry_version()` |

## M23c: FuzzySelect, Validation & Confirmations

- **FuzzySelect**: `prompt_remove()` uses `FuzzySelect` when >5 items
- **Input validation**: `validate_name` (lowercase+digits+hyphens), `validate_url` (https/http prefix), `validate_non_empty`
- **Confirmations**: `confirm()` helper using `dialoguer::Confirm`
  - `husako clean`: confirms before removing
  - `husako remove <name>`: confirms in CLI mode (not interactive)
- **`--yes`/`-y` flag**: Global clap flag to skip confirmations

### Files

| File | Action |
|------|--------|
| `crates/husako-cli/src/interactive.rs` | FuzzySelect, validators, `confirm()` |
| `crates/husako-cli/src/main.rs` | `--yes` flag, confirmation calls |
| `Cargo.toml` | Added `fuzzy-select` feature to dialoguer |

## New Tests (11 total)

- `artifacthub_package_deserialize` — JSON deserialization
- `artifacthub_package_missing_description` — Optional field handling
- `artifacthub_has_more_detection` — Pagination overflow detection
- `artifacthub_display_formatting` — Package ID and description truncation
- `search_select::constants_are_valid` — Widget constant invariants
- `validate_name_accepts_valid` — Valid name patterns
- `validate_name_rejects_invalid` — Invalid name patterns
- `validate_url_accepts_valid` — Valid URL patterns
- `validate_url_rejects_invalid` — Invalid URL patterns
- `validate_non_empty_works` — Empty/whitespace rejection
- `style::helpers_return_non_empty` — Style helper validation
