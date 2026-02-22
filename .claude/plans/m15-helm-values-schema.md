# M15: Helm Values Schema Type Generation

## Context

FluxCD HelmRelease CRD has a `values` field that accepts any JSON — no type safety. Helm charts can ship a `values.schema.json` (JSON Schema) that describes the valid values. This milestone extends husako's type generation to produce TypeScript interfaces from Helm values schemas, so users get autocomplete and validation when authoring HelmRelease resources.

**Config changes**: `[schemas]` is renamed to `[resources]` (breaking, pre-1.0). A new `[charts]` section is added with 4 source types.

**Import prefix**: `helm/*` — separate from `k8s/*`, with its own resolver and output directory. (Config section is `[charts]`, import prefix is `helm/*`, matching the pattern where `[resources]` maps to `k8s/*`.)

## Config Format

```toml
[resources]
kubernetes = { source = "release", version = "1.35" }
cert-manager = { source = "git", repo = "...", tag = "v1.17.2", path = "deploy/crds" }

[charts]
ingress-nginx = { source = "registry", repo = "https://kubernetes.github.io/ingress-nginx", chart = "ingress-nginx", version = "4.12.0" }
postgresql = { source = "artifacthub", package = "bitnami/postgresql", version = "16.4.0" }
my-chart = { source = "file", path = "./schemas/my-chart-values.schema.json" }
my-other = { source = "git", repo = "https://github.com/...", tag = "v1.0.0", path = "charts/my-chart" }
```

## User-Facing TypeScript

```typescript
import { values, controller } from "helm/ingress-nginx";
import { helmRelease, helmChartTemplate, crossNamespaceObjectReference } from "k8s/helm.toolkit.fluxcd.io/v2";
import { name, build } from "husako";

const release = helmRelease()
  .metadata(name("my-release"))
  .chart(
    helmChartTemplate()
      .chart("ingress-nginx")
      .version("4.12.0")
      .sourceRef(
        crossNamespaceObjectReference()
          .kind("HelmRepository")
          .name("ingress-nginx")
      )
  )
  .interval("10m")
  .values(
    values()
      .controller(
        controller()
          .replicaCount(2)
          .image({ repository: "nginx", tag: "1.25" }) // plain object — Image has no Ref properties
      )
  );

build([release]);
```

Chart values follow the **exact same** code generation rules as k8s types (builder-spec Section 8):
- Nested `object` properties in JSON Schema are extracted into separate named schemas (creating `Ref` relationships), same as CRD conversion
- Schemas with `Ref`/`Array(Ref)` properties → `_SchemaBuilder` subclass with factory function
- Schemas with only primitive properties → plain interface, used as object literals (builder-spec Section 1: "Plain objects are allowed only as leaf values where no builder exists")

## Output Structure

```
.husako/types/
├── helm/
│   ├── ingress-nginx.d.ts     # Interfaces + builder classes
│   ├── ingress-nginx.js       # Builder implementations (real, not stubs)
│   └── postgresql.d.ts
├── k8s/                        # Existing
└── tsconfig.json               # Updated with helm/* path
```

Both `.d.ts` and `.js` files are generated per chart, identical pattern to `k8s/*` modules. Objects with complex properties get `_SchemaBuilder` subclasses; simple scalars get plain property methods.

---

## Phases

### Phase 1 (M15a): Foundation — Config + File Source + Type Generation

**Goal**: End-to-end flow with local `values.schema.json` files.

#### 1. Config (`crates/husako-config/src/lib.rs`)

- Rename `schemas` field to `resources` with `#[serde(alias = "schemas")]` for backward compat
- Print deprecation warning to stderr when `[schemas]` is detected (check raw TOML string before parse)
- Add `charts: HashMap<String, ChartSource>` field
- Add `ChartSource` tagged enum:

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "source")]
pub enum ChartSource {
    #[serde(rename = "registry")]
    Registry { repo: String, chart: String, version: String },
    #[serde(rename = "artifacthub")]
    ArtifactHub { package: String, version: String },
    #[serde(rename = "file")]
    File { path: String },
    #[serde(rename = "git")]
    Git { repo: String, tag: String, path: String },
}
```

- Update `validate()`: check chart file paths are relative
- Update all existing tests referencing `config.schemas` → `config.resources`

#### 2. New crate: `husako-helm`

Create `crates/husako-helm/` with:

- `Cargo.toml`: depends on `reqwest`, `serde`, `serde_json`, `serde_yaml_ng`, `thiserror` (all workspace), plus new `flate2` and `tar`
- `src/lib.rs`: error type `HelmError`, public dispatch function
- `src/file.rs`: read local `values.schema.json`, validate it's valid JSON Schema

#### 3. JSON Schema → TypeScript + JS (`crates/husako-dts/src/json_schema.rs`)

New module that converts JSON Schema to `.d.ts` and `.js` using the same builder-spec patterns as k8s types. JSON Schema differences from OpenAPI:

- `$ref` uses `#/$defs/` or `#/definitions/` (not `#/components/schemas/`)
- `enum` on strings → string literal union
- `oneOf`/`anyOf` → TypeScript union
- `allOf` → intersection type

**Same as k8s**: Objects with complex (nested object/array) properties get `_SchemaBuilder` subclasses with per-property chainable methods and factory functions. The root schema becomes a `Values` builder. Follows builder-spec Section 8 heuristics.

Public API:

```rust
/// Returns (dts_content, js_content)
pub fn generate_chart_types(
    chart_name: &str,
    schema: &serde_json::Value,
) -> Result<(String, String), DtsError>;
```

Output `.d.ts`:

```typescript
import { _SchemaBuilder } from "husako/_base";

export interface ValuesSpec {
  replicaCount?: number;
  image?: ImageSpec;
}

export interface ImageSpec {
  repository?: string;
  tag?: string;
}

export class Values extends _SchemaBuilder {
  replicaCount(value: number): Values;
  image(value: ImageSpec | Image): Values;
}

export class Image extends _SchemaBuilder {
  repository(value: string): Image;
  tag(value: string): Image;
}

export function values(): Values;
export function image(): Image;
```

Output `.js`:

```javascript
import { _SchemaBuilder } from "husako/_base";

export class Values extends _SchemaBuilder {
  replicaCount(v) { return this._set("replicaCount", v); }
  image(v) { return this._set("image", v); }
}

export class Image extends _SchemaBuilder {
  repository(v) { return this._set("repository", v); }
  tag(v) { return this._set("tag", v); }
}

export function values() { return new Values(); }
export function image() { return new Image(); }
```

**Naming**: Root → `Values`/`values()`. Nested objects → PascalCase from property name (e.g., `controller` → `Controller`/`controller()`). `$defs` keys used as-is.

#### 4. Chart source orchestrator (`crates/husako-core/src/chart_source.rs`)

New module (parallel to `schema_source.rs`):

```rust
pub fn resolve_all(
    config: &HusakoConfig,
    project_root: &Path,
    cache_dir: &Path,
) -> Result<HashMap<String, serde_json::Value>, HusakoError>;
```

Returns `chart_name → JSON Schema value`. Phase 1 only dispatches `File` variant.

#### 5. Wire into generate (`crates/husako-core/src/lib.rs`)

After the k8s type generation block (line ~206), add chart type generation:

- Call `chart_source::resolve_all()` for `[charts]` entries
- Call `husako_dts::generate_chart_types()` for each schema → returns `(dts, js)` tuple
- Write `.d.ts` + `.js` files to `.husako/types/helm/`
- Update `write_tsconfig()` to include `"helm/*": [".husako/types/helm/*"]` in paths
- Update `config.schemas` reference at line 185 to `config.resources`

#### 6. Resolver (`crates/husako-runtime-qjs/src/resolver.rs`)

Extend `HusakoK8sResolver` to also handle `helm/*`:

- Check `name.starts_with("helm/")` in addition to `"k8s/"`
- Resolve to `.js` files in `.husako/types/helm/` (same pattern as k8s)

#### 7. Error handling (`crates/husako-cli/src/main.rs`)

- Add `HusakoError::Chart` variant wrapping `husako_helm::HelmError`
- Map to exit code 6 (same category as OpenAPI fetch errors)

#### 8. Workspace (`Cargo.toml`)

- Add `husako-helm` to workspace members
- Add `flate2 = "1"` and `tar = "0.4"` to workspace dependencies

---

### Phase 2 (M15b): Network Sources — Registry + ArtifactHub

#### 1. HTTP Registry (`crates/husako-helm/src/registry.rs`)

Flow:

1. Check cache: `.husako/cache/helm/registry/{hash(repo,chart)}/{version}.json`
2. `GET {repo}/index.yaml` → parse YAML
3. Find `entries[chart]` → match version → get archive URL
4. `GET` the `.tgz` archive
5. `GzDecoder` → `tar::Archive` → find `{chart}/values.schema.json`
6. Parse JSON, cache, return

Edge cases:

- Chart/version not found → clear error
- No `values.schema.json` in archive → error: "chart does not include values.schema.json"
- `oci://` prefix → error: "OCI registries require `helm` CLI; use `source = \"artifacthub\"` or `source = \"file\"` instead"

#### 2. ArtifactHub (`crates/husako-helm/src/artifacthub.rs`)

Flow:

1. Check cache: `.husako/cache/helm/artifacthub/{hash(package)}/{version}.json`
2. Parse `package` as `"{repo}/{chart}"`
3. `GET https://artifacthub.io/api/v1/packages/helm/{repo}/{chart}/{version}`
4. Parse response, extract `values_schema` field (it's the JSON Schema)
5. If absent, try: `GET https://artifacthub.io/api/v1/packages/{package_id}/{version}/values-schema`
6. Cache and return

#### 3. Wire into dispatch (`crates/husako-helm/src/lib.rs`)

Update the dispatch function to handle `Registry` and `ArtifactHub` variants.

---

### Phase 3 (M15c): Git Source + Polish

#### 1. Git source (`crates/husako-helm/src/git.rs`)

Reuse pattern from `husako-core/schema_source.rs::resolve_git()`:

1. Check cache: `.husako/cache/helm/git/{hash(repo,tag,path)}.json`
2. `git clone --depth 1 --branch {tag} {repo}` into temp dir
3. Read `{temp}/{path}/values.schema.json` (or `{path}` directly if it ends in `.json`)
4. Parse, cache, return

#### 2. Template updates (`crates/husako-sdk/src/lib.rs`)

Add commented-out `[charts]` example to template TOML files. Update `[schemas]` → `[resources]` in all templates.

#### 3. Documentation

Update `CLAUDE.md` architecture section and `.claude/PLAN.md`.

---

## Files Summary

### New Files

| File                                     | Purpose                        |
| ---------------------------------------- | ------------------------------ |
| `crates/husako-helm/Cargo.toml`          | Crate manifest                 |
| `crates/husako-helm/src/lib.rs`          | Error types, public dispatch   |
| `crates/husako-helm/src/file.rs`         | Local file source              |
| `crates/husako-helm/src/registry.rs`     | HTTP Helm repo (M15b)          |
| `crates/husako-helm/src/artifacthub.rs`  | ArtifactHub API (M15b)         |
| `crates/husako-helm/src/git.rs`          | Git clone source (M15c)        |
| `crates/husako-dts/src/json_schema.rs`   | JSON Schema → .d.ts + .js (builders) |
| `crates/husako-core/src/chart_source.rs` | Chart source orchestrator      |

### Modified Files

| File                                        | Changes                                                               |
| ------------------------------------------- | --------------------------------------------------------------------- |
| `Cargo.toml`                                | Add `husako-helm` member, `flate2`, `tar` deps                        |
| `crates/husako-config/src/lib.rs`           | `ChartSource` enum, `charts` field, `schemas` → `resources` alias     |
| `crates/husako-core/Cargo.toml`             | Add `husako-helm` dependency                                          |
| `crates/husako-core/src/lib.rs`             | Wire chart generation, update tsconfig paths, `schemas` → `resources` |
| `crates/husako-dts/src/lib.rs`              | Export `json_schema` module and `generate_chart_types()`              |
| `crates/husako-runtime-qjs/src/resolver.rs` | Handle `helm/*` prefix                                                |
| `crates/husako-cli/src/main.rs`             | `ChartError` exit code mapping                                        |
| `crates/husako-sdk/src/lib.rs`              | Template TOML updates (M15c)                                          |

---

## Verification

```bash
# Build
cargo build

# All tests (should still pass 270 existing + ~30 new)
cargo test --workspace --all-features

# Lint
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Format
cargo fmt --all --check

# Manual E2E: file source
mkdir /tmp/chart-test && cd /tmp/chart-test
cat > husako.toml << 'EOF'
[charts]
my-chart = { source = "file", path = "./values.schema.json" }
EOF
cat > values.schema.json << 'EOF'
{
  "type": "object",
  "properties": {
    "replicaCount": { "type": "integer", "default": 1 },
    "image": {
      "type": "object",
      "properties": {
        "repository": { "type": "string" },
        "tag": { "type": "string" }
      }
    }
  }
}
EOF
husako generate
cat .husako/types/helm/my-chart.d.ts
# Expected: ValuesSpec interface + Values builder class + values() factory
cat .husako/types/helm/my-chart.js
# Expected: Values _SchemaBuilder subclass with .replicaCount()/.image() methods

# Verify tsconfig includes helm path
cat tsconfig.json | grep "helm"

# Manual E2E: backward compat ([schemas] still works)
cat > husako.toml << 'EOF'
[schemas]
kubernetes = { source = "release", version = "1.35" }
EOF
husako generate
# Should work with deprecation warning on stderr

# Manual E2E: ArtifactHub (M15b)
cat > husako.toml << 'EOF'
[charts]
ingress-nginx = { source = "artifacthub", package = "ingress-nginx/ingress-nginx", version = "4.12.0" }
EOF
husako generate
cat .husako/types/helm/ingress-nginx.d.ts
```
