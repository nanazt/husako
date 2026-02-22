# Milestone 5: Type Generation + `husako init`

**Status**: Completed
**Commit**: `ae9db93`

## Goal

Generate TypeScript `.d.ts` type definitions from OpenAPI specs so users get full autocomplete in their editor. Provide `husako init` command to bootstrap a project.

## Deliverables

### Type Generation (`husako-dts`)

- Parse OpenAPI component schemas into structured `SchemaInfo`
- Classify schemas: Common (`io.k8s.apimachinery.*`), GroupVersion (`io.k8s.api.*`), Other
- Generate `.d.ts` files:
  - `k8s/_common.d.ts` — shared types (ObjectMeta, LabelSelector, etc.)
  - `k8s/<group>/<version>.d.ts` — per-group-version interfaces + builder classes
- Generate `husako/_base.d.ts` — `_ResourceBuilder` base class types
- Builder classes extend `_ResourceBuilder` for registered kinds

### `husako init` Command

- Write static `husako.d.ts` (SDK types)
- Write static `husako/_base.d.ts` (base builder types)
- Fetch OpenAPI specs (if `--api-server` or `--spec-dir` provided)
- Generate k8s `.d.ts` files from specs
- Write/update `tsconfig.json` with husako path mappings
- `--skip-k8s` flag to skip k8s type generation

## Architecture Decisions

### Schema Parsing (`husako-dts/src/schema.rs`)

```rust
pub struct SchemaInfo {
    pub full_name: String,       // io.k8s.api.apps.v1.Deployment
    pub ts_name: String,         // Deployment
    pub location: SchemaLocation, // Common | GroupVersion | Other
    pub properties: Vec<PropertyInfo>,
    pub gvk: Option<GroupVersionKind>,
}

pub enum TsType {
    String, Number, Boolean, IntOrString,
    Array(Box<TsType>), Map(Box<TsType>),
    Ref(String), Any,
}
```

### Type Emission (`husako-dts/src/emitter.rs`)

- Common types -> `_common.d.ts` (interfaces only)
- GV types -> `<group>/<version>.d.ts`:
  - Interfaces for all schemas in that group-version
  - Import `_common` types
  - Builder `class` for registered kinds (extends `_ResourceBuilder`)
  - Non-registered kinds: interface only (no builder)

### Registered Kinds

Hardcoded list in `husako-core` of kinds that have runtime builder classes:
- `apps/v1:Deployment`
- `v1:Namespace`
- `v1:Service`
- `v1:ConfigMap`

Only these get `class Foo extends _ResourceBuilder<FooSpec>` in the generated `.d.ts`.

### tsconfig.json Management

- If `tsconfig.json` exists: parse, merge husako paths into `compilerOptions.paths`
- If not: create new with husako paths + sensible defaults
- Preserves all existing user config

### Path Mappings

```json
{
  "husako": [".husako/types/husako.d.ts"],
  "husako/_base": [".husako/types/husako/_base.d.ts"],
  "k8s/*": [".husako/types/k8s/*"]
}
```

## Files Created/Modified

```
crates/husako-dts/src/lib.rs        # generate() orchestration
crates/husako-dts/src/schema.rs     # OpenAPI schema parsing
crates/husako-dts/src/emitter.rs    # .d.ts code generation
crates/husako-sdk/src/dts/husako.d.ts      # Static SDK type declarations
crates/husako-sdk/src/dts/husako_base.d.ts # Static base builder types
crates/husako-core/src/lib.rs       # init() function, registered_kinds()
crates/husako-cli/src/main.rs       # Init subcommand
```

## Tests

### Unit Tests (husako-dts)

- Schema parsing: properties, required fields, types, refs
- Schema classification: Common vs GroupVersion vs Other
- TsType mapping: string, integer, boolean, array, map, ref, int-or-string
- GVK extraction from `x-kubernetes-group-version-kind`
- Snapshot tests: `_common.d.ts`, `apps/v1.d.ts`

### Unit Tests (husako-core)

- `init_skip_k8s_writes_static_dts` — skip-k8s writes husako.d.ts only
- `init_updates_existing_tsconfig` — merges into existing tsconfig

### Integration Tests

- `init --skip-k8s` -> static files + tsconfig
- `init --spec-dir` -> generates k8s types
- `init` updates existing tsconfig without losing user config

## Acceptance Criteria

- [x] `husako init --spec-dir` generates .d.ts files
- [x] Generated .d.ts has correct interfaces for schemas
- [x] Builder classes generated for registered kinds
- [x] `_common.d.ts` has shared types
- [x] tsconfig.json created/updated with paths
- [x] `--skip-k8s` writes only static types
- [x] Existing tsconfig preserved and merged
