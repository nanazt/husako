# Architecture Deep-Dive

Implementation details not covered by CLAUDE.md, dsl-spec.md, or cli-design.md.

## 1. Schema Classification & Code Generation

### Location classification (`husako-dts/src/schema.rs`)

Every OpenAPI schema name is classified into one of three locations:

1. **Common** — name starts with `io.k8s.apimachinery.` → emitted to `_common.d.ts`/`_common.js`
2. **GroupVersion** — name starts with `io.k8s.api.` → strip prefix, split into `[group, version, Type]`, emitted to per-module files (e.g., `apps/v1.d.ts`)
3. **Other** — everything else (CRDs, non-standard schemas)

### CRD reclassification

After initial classification, schemas with `location == Other` AND a populated `gvk` field (from `x-kubernetes-group-version-kind`) are reclassified into `GroupVersion` using the GVK's group (defaulting to `"core"` if empty) and version. This ensures CRDs whose schema names (e.g., `io.cnpg.postgresql.v1.Cluster`) don't match the `io.k8s.api.*` pattern still land in the correct module.

### Type mapping decision order (`ts_type_from_schema`)

1. `$ref` → `TsType::Ref(short_name)` (last segment after `#/components/schemas/`)
2. `x-kubernetes-int-or-string: true` → `TsType::IntOrString`
3. `type` field:
   - `"string"` → `String`
   - `"integer"` | `"number"` → `Number`
   - `"boolean"` → `Boolean`
   - `"array"` → `Array(items_type)` (recurse into `items`)
   - `"object"` + `additionalProperties` → `Map(val_type)`
   - `"object"` + `properties` only → `Any` (inline objects handled by extraction, not this level)
   - `"object"` with neither → `Map(Any)`
4. No recognized type → `Any`

### Builder generation heuristic

A schema gets a `_SchemaBuilder` subclass when it has at least one property whose type is `Ref(_)` or `Array(Ref(_))`. Schemas with only primitives, maps, or arrays of primitives get plain interfaces.

Properties skipped during builder method generation: `status`, `apiVersion`, `kind`, `metadata`.

### Pod template shortcut

When a schema has a `template` property referencing `PodTemplateSpec`, the builder gains `.containers()` and `.initContainers()` deep-path methods via `_setDeep`.

### DTS emission order per group-version

1. Common type imports (from `_common`)
2. Spec interfaces for builder-bearing schemas
3. `_ResourceBuilder` subclasses (schemas with GVK)
4. `_SchemaBuilder` subclasses (schemas without GVK but with complex properties)
5. Plain interfaces

A `.js` file is emitted only if the module has at least one GVK schema or one schema meeting the builder heuristic.

## 2. CRD YAML → OpenAPI Conversion

**Module:** `husako-openapi/src/crd.rs`

### Algorithm

1. Parse multi-document YAML (`serde_yaml_ng::Deserializer`).
2. Filter documents by `apiVersion == "apiextensions.k8s.io/v1"` AND `kind == "CustomResourceDefinition"`. Non-CRDs are silently skipped.
3. Error if zero CRDs found.
4. For each CRD, extract `spec.group`, `spec.names.kind`, `spec.versions[]`.
5. Compute prefix via `reverse_domain(group)` — `"cert-manager.io"` → `"io.cert-manager"`.
6. For each version with `schema.openAPIV3Schema`:
   - Base name: `"{prefix}.{version}"` (e.g., `"io.cert-manager.v1"`)
   - Extract nested schemas recursively
   - Build resource schema with injected `apiVersion`, `kind`, `metadata` ($ref to ObjectMeta), `x-kubernetes-group-version-kind`

### Nested schema extraction

Inline CRD schemas are converted to `$ref` graphs:

- A property is extractable if it has `type == "object"` AND a `properties` key (not just `additionalProperties`).
- Extracted schemas get PascalCase names: property `issuerRef` on `Certificate` → `CertificateIssuerRef`.
- The original property is replaced with a `$ref`, preserving its `description`.
- Arrays with extractable `items` are also extracted.
- Recursion continues into extracted schemas.

### Naming conventions

- Top-level: `{base}.{Kind}` (e.g., `io.cert-manager.v1.Certificate`)
- Spec: `{base}.{Kind}Spec`
- Nested: `{base}.{Context}{PascalCase(propName)}`
- `to_pascal_case`: splits on `_` and `-`, capitalizes first letter, preserves camelCase within segments

### Domain reversal

`reverse_domain` splits on `.` and reverses: `"postgresql.cnpg.io"` → `"io.cnpg.postgresql"`.

Output is wrapped in `{"components": {"schemas": {...}}}` matching OpenAPI v3 format.

## 3. Validation Engine

**Module:** `husako-core/src/validate.rs`

### Walk order per node (`validate_value`)

1. **Depth guard** — return if `depth > 64`
2. **Null skip** — `null` treated as "not set"
3. **`$ref` resolution** — resolve and recurse (depth + 1), return
4. **`allOf`** — validate against each sub-schema (depth + 1 per sub), return
5. **`x-kubernetes-int-or-string`** — accept Number or String, reject else, return
6. **Format dispatch** — if `format == "quantity"`, delegate to `validate_quantity()`, return
7. **Type check** — verify value matches schema's `type`, return on mismatch
8. **Enum check** — string membership in `enum` array, return on mismatch
9. **Numeric bounds** — `minimum`/`maximum` (additive, both can error)
10. **Pattern check** — `regex_lite::Regex`, skip silently on compile failure
11. **Recurse** — object properties + additionalProperties, array items

### Not handled

`oneOf`, `anyOf`, `x-kubernetes-validation` (CEL), unknown field rejection.

### Schema store

- Version `2` required (both producer and consumer). `from_json()` returns `None` for any other version.
- GVK index key format: `"{apiVersion}:{kind}"` (e.g., `"apps/v1:Deployment"`).

### Fallback chain

1. SchemaStore available + GVK matches → full schema validation
2. SchemaStore available + GVK not found → quantity heuristic only
3. No SchemaStore → quantity heuristic only

The heuristic checks `resources.requests/*` and `resources.limits/*` paths.

## 4. JSON Schema Codegen for Helm

**Module:** `husako-dts/src/json_schema.rs`

Differences from the OpenAPI codegen path (`husako-dts/src/lib.rs`):

| Aspect | OpenAPI path | JSON Schema path |
|--------|-------------|-----------------|
| `$ref` prefix | `#/components/schemas/` | `#/$defs/` or `#/definitions/` |
| `enum` on strings | preserves values | simplified to `TsType::String` |
| `oneOf`/`anyOf` | not handled | collapsed to `TsType::Any` |
| `additionalProperties: true` (boolean) | not handled | `Map(Any)` |
| Inline objects with properties | left as `Any` (CRD converter handles extraction) | extracted during codegen into named `Ref` schemas |
| PascalCase splitting | `_` and `-` | `_`, `-`, and `.` |

### Dual type resolution

- `resolve_json_schema_type()` — for builder classes, produces `Ref("Image")`
- `resolve_json_schema_type_for_spec()` — for Spec interfaces, produces `Ref("ImageSpec")` when a matching `*Spec` schema exists

### Output structure

- Root always becomes `Values`/`ValuesSpec`/`values()`.
- Nested objects produce `{Name}Spec` interface + `{Name}` builder.
- Factory functions use first-char-lowercase: `Values` → `values()`.

### Emission order

1. `_SchemaBuilder` import (if builders exist)
2. Spec interfaces
3. Builder classes
4. Plain interfaces

## 5. Version Discovery

**Module:** `husako-core/src/version_check.rs`

### Per-source discovery

**`release`** — GitHub API `kubernetes/kubernetes` tags:
- Filter: strip `v` prefix, skip tags with `-` (pre-release), parse `semver::Version`
- Returns `"{major}.{minor}"` format (no patch) — matches config's version format
- Pagination: fetches 100 tags per page

**`registry`** — Helm repo `index.yaml`:
- HTTP GET `{repo}/index.yaml`, navigate `entries[chart]`
- Filter: parseable semver with empty pre-release
- Returns full semver (e.g., `"4.12.1"`)

**`artifacthub`** — REST API:
- Endpoint: `https://artifacthub.io/api/v1/packages/helm/{package}`
- `available_versions` array, filter `prerelease == true` OR non-empty pre-release segment
- 10-second request timeout

**`git`** — `git ls-remote --tags`:
- Parse tab-separated output, strip `refs/tags/` and `^{}` suffix
- Strip `v` for semver parsing, filter stable
- Returns original tag string (preserving `v` prefix)

**`file`** — skipped (no version concept).

### Version comparison (`versions_match`)

1. Exact string match
2. Strip `v` prefix from both
3. If current has ≤1 dot (major.minor format): prefix match (`"1.35"` matches `"1.35.0"`)
4. Otherwise: exact match after `v` stripping

### Search pagination

ArtifactHub search requests `PAGE_SIZE + 1` (21) results to detect `has_more`, truncates to 20 before returning.

## 6. TOML Write-Back

**Module:** `husako-config/src/edit.rs`

Uses `toml_edit::DocumentMut` (CST-based editing) to preserve comments, whitespace, and formatting across edits.

### Entry format

All dependency entries use inline tables:
```toml
kubernetes = { source = "release", version = "1.35" }
```

Constructed via `source_to_inline_table()` (resources) and `chart_source_to_inline_table()` (charts).

### Version update logic (`update_version_in_item`)

Tries `"version"` key first, then `"tag"` key (for git sources). Works on both inline tables and standard tables.

### Lazy loading

The TOML document is only loaded from disk if at least one entry needs updating, avoiding unnecessary I/O for up-to-date projects.

### Remove operations

`table.remove(name)` on the section's table-like interface. `remove_dependency` tries resources first, then charts — first match wins.

## 7. Cache Structure

```
.husako/
├── cache/
│   ├── release/{tag}/              # e.g., v1.35.0/
│   │   ├── _manifest.json          # [(discovery_key, filename)] pairs
│   │   └── apis__apps__v1_openapi.json
│   ├── git/{hash}/                 # djb2 hash of repo URL (16-char hex)
│   │   └── {tag}/
│   │       └── apis__cert-manager.io__v1.json
│   └── helm/
│       ├── registry/{hash}/        # djb2("{repo}/{chart}")
│       │   └── {version}.json
│       ├── artifacthub/{hash}/     # djb2(package)
│       │   └── {version}.json
│       └── git/{hash}/             # djb2("{repo}/{path}")
│           └── {tag}.json
└── types/
    ├── k8s/                        # generated .d.ts + .js per group-version
    └── helm/                       # generated .d.ts + .js per chart

husako.lock                         # project-root lock file (alongside husako.toml, NOT inside .husako/)
```

### Hash function

djb2: `hash = hash.wrapping_mul(33).wrapping_add(byte)` from seed 5381, formatted as 16-char zero-padded hex. Used identically in `husako-helm` and `husako-core/schema_source.rs`.

### Release cache specifics

- Cache key is the git tag, making it deterministic for pinned versions.
- Discovery paths use `__` as separator: `apis/apps/v1` → `apis__apps__v1_openapi.json`.
- `_manifest.json` enables fast reload; directory scanning is the fallback.

### Invalidation model

No explicit invalidation. Pinned sources (release+version, git+tag, chart+version) are deterministic by design. File sources are not cached (intentionally mutable).

## 8. Lock File (Incremental Type Generation)

`husako.lock` is written to the **project root** (alongside `husako.toml`) by
`husako_core::generate()` after every successful run. Lock write failure is non-fatal —
types are already written; only a warning is printed.

**Structs:** `husako-config/src/lock.rs`
- `HusakoLock { format_version, husako_version, resources, charts, plugins }` — all maps are `BTreeMap` for deterministic TOML output
- `ResourceLockEntry`, `ChartLockEntry`, `PluginLockEntry` — `#[serde(tag = "source")]` enums matching `SchemaSource`/`ChartSource`/`PluginSource` field shapes

**Load/save:** `husako_config::load_lock(root)` / `husako_config::save_lock(root, lock)`

**Skip decision module:** `husako-core/src/lock_check.rs`
- `should_skip_k8s(config, lock, husako_version, types_dir, project_root)` — all resources unchanged?
- `should_skip_chart(name, source, lock, husako_version, types_dir, project_root)` — per chart
- `should_skip_plugin(name, source, lock, plugins_dir, project_root)` — per plugin

**Skip criteria (summary):**

| Entry | Identity key | Extra check |
|---|---|---|
| `release` resource | `version` | `types/k8s/` exists |
| `git` resource | `repo + tag + path` | `types/k8s/` exists |
| `file` resource | `path + content_hash` | `types/k8s/` exists |
| chart (any) | source-specific version/tag | `types/helm/{name}.d.ts` exists |
| `git` plugin | `url + path` | installed dir + plugin_version match |
| `path` plugin | `path + content_hash` | installed dir + plugin_version match |

K8s resources are all-or-nothing: if ANY resource fails its skip check, ALL k8s
types are regenerated (because `resolve_all()` merges them before codegen).

**`GenerateOptions` additions:**
- `husako_version: String` — set by CLI from `env!("CARGO_PKG_VERSION")`
- `no_incremental: bool` — skip all lock checks (lock still written at end)

**`--skip-k8s` interaction:** When `skip_k8s` is true, preserve existing resource entries
from the old lock verbatim in `new_lock`. This ensures the next `husako gen` without
`--skip-k8s` still benefits from incremental skip.

**File hashing:** `lock_check::hash_file(path)` / `hash_dir(path)` — djb2 over
content bytes (single file) or sorted relative-path+content bytes (directory). Same
djb2 algorithm as `husako-helm::cache_hash`.

## 9. Plugin System

**Spec:** `.claude/plugin-spec.md` (authoritative reference)

### Storage layout

```
.husako/plugins/<name>/
├── plugin.toml           # Manifest
└── modules/              # Importable JS + .d.ts
    ├── index.js
    ├── index.d.ts
    └── sub.js
```

### Plugin manifest (`plugin.toml`)

Parsed by `husako-config`: `PluginManifest` struct with `PluginMeta` (name, version, description), `resources` (HashMap → `SchemaSource`), `charts` (HashMap → `ChartSource`), `modules` (HashMap → relative `.js` path).

### Configuration (`husako.toml`)

```toml
[plugins]
flux = { source = "git", url = "https://github.com/nanazt/husako-plugin-flux" }
my-plugin = { source = "path", path = "./plugins/my-plugin" }
```

`PluginSource` enum: `Git { url, path: Option<String> }` or `Path { path }`. Uses `#[serde(tag = "source")]` matching the `SchemaSource`/`ChartSource` pattern. When `path` is set on `Git`, sparse-checkout fetches only that subdirectory (for monorepo plugins).

### Install lifecycle (`husako-core/src/plugin.rs`)

1. `install_plugins()` iterates `config.plugins` in order
2. For each plugin: clean existing install → dispatch to `install_git()` (shallow clone, remove `.git/`) or `install_path()` (recursive copy)
3. Load `plugin.toml` manifest from installed directory
4. Return `Vec<InstalledPlugin>` for downstream use

### Preset merging

`merge_plugin_presets()` adds plugin resources/charts into the main config with namespaced keys `<plugin>:<name>` (e.g., `flux:flux-source`). Uses `entry().or_insert_with()` so user-defined entries take precedence.

### Module resolution chain (updated)

`BuiltinResolver` → **`PluginResolver`** → `HusakoK8sResolver` → `HusakoFileResolver`

`PluginResolver` maps import specifiers (e.g., `"flux"`, `"flux/helm"`) to absolute `.js` paths under `.husako/plugins/<name>/`. Built from `PluginManifest.modules`.

### tsconfig.json integration

`plugin_tsconfig_paths()` builds specifier → `.d.ts` path mappings (e.g., `"flux"` → `.husako/plugins/flux/modules/index.d.ts`). Paths are added alongside `k8s/*` and `helm/*` in the generated `tsconfig.json`.

### Runtime loading

`load_plugin_modules()` scans `.husako/plugins/` for directories with valid `plugin.toml`, collecting all module specifier → `.js` path mappings. Called by `render()` and `validate_file()` to populate `ExecuteOptions.plugin_modules`.

### Generate integration

1. Install plugins from `[plugins]` config
2. Clone config, merge plugin presets (resources + charts)
3. Generate k8s types (includes plugin CRD resources)
4. Generate chart types (includes plugin chart presets)
5. Write tsconfig with plugin module paths

### CLI commands

- `husako plugin add <name> --url <url>` / `--path <path>` — adds to `husako.toml`
- `husako plugin remove <name>` — removes from `husako.toml` + deletes `.husako/plugins/<name>/`
- `husako plugin list` — shows installed plugins from `.husako/plugins/`

## 10. Critical Invariants

1. **CRD reclassification must precede group-version partitioning** — otherwise CRD schemas stay in `Other` and are never emitted to module files.

2. **`_schema.json` version 2** — both the producer (`schema_store.rs`) and consumer (`validate.rs`) must agree on version 2. `from_json()` returns `None` for any other version.

3. **`HusakoK8sResolver` handles both `k8s/*` and `helm/*`** — despite the name, this single resolver maps both prefixes to `.husako/types/*.js` files.

4. **tsconfig.json paths must include `husako` and `k8s/*`** — optionally `helm/*` when charts are configured, plus plugin module specifiers when plugins are installed. Missing paths break IDE autocomplete.

5. **Schema filename ↔ discovery key** — uses `__` → `/` replacement (e.g., `apis__apps__v1` ↔ `apis/apps/v1`).

6. **Module resolution chain order** — `BuiltinResolver` → `PluginResolver` → `HusakoK8sResolver` → `HusakoFileResolver`. Each returns `Err` for unhandled imports, passing to the next resolver.

7. **Generate priority** — `--skip-k8s` → `--no-incremental` (bypass lock) → lock-file skip check → CLI flags → `husako.toml [resources]` → skip. Charts from `[charts]` are always generated when configured. Plugins are always installed first when `[plugins]` is configured.

8. **Render precedence** — `_spec` > `_specParts` > `_resources`. Calling `.spec({...})` clears `_specParts`.

9. **Dependency list sorting** — `list_dependencies()` and `check_outdated()` iterate in sorted order by name for deterministic output.

10. **Plugin preset namespace** — Plugin resources/charts are merged with `<plugin>:<name>` keys to avoid collisions with user-defined dependencies.
