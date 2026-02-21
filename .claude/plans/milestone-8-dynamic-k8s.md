# Milestone 8: Dynamic K8s Resources + CRDs

**Status**: Completed

## Goal

Eliminate all hardcoded K8s resource types and generate both runtime JS modules and rich `.d.ts` types from OpenAPI schemas during `husako init`. This enables support for all Kubernetes built-in resources and CRDs without code changes.

## Pipeline Change

```
husako init:
  fetch OpenAPI → generate .d.ts only (4 hardcoded builders)     # old (M5)
  fetch OpenAPI → generate .d.ts + .js for ALL resources/CRDs    # new (M8)

husako render:
  k8s/* → resolve from embedded JS (4 resources)                 # old
  k8s/* → resolve from .husako/types/k8s/*.js (all resources)    # new
```

## Hardcoded Locations Removed (4)

1. `husako-sdk/src/lib.rs` — `K8S_APPS_V1`, `K8S_CORE_V1` constants
2. `husako-runtime-qjs/src/lib.rs` — `BuiltinResolver`/`BuiltinLoader` entries for `k8s/apps/v1`, `k8s/core/v1`
3. `husako-core/src/lib.rs` — `registered_kinds()` function (4 fixed entries)
4. `husako-sdk/src/js/k8s_apps_v1.js`, `k8s_core_v1.js` — hardcoded runtime JS modules

## Deliverables

### 1. Generic Builder Base Class (`husako_base.js`)

New methods on `_ResourceBuilder`:

- `_copy()` — internal clone helper, returns a new instance of the same constructor with all fields copied
- `.spec(value)` — generic spec setter, takes a plain object
- `.set(key, value)` — set arbitrary top-level field (`data`, `rules`, `roleRef`, etc.)
- `_render()` — if `_spec` is set, use it directly; otherwise fall back to `_resources` Deployment structure for backward compat; also emit `_extra` fields

All chainable methods return new instances via `_copy()` (immutable builder pattern).

Preserved unchanged:
- `.metadata(fragment)` — same behavior
- `.resources(...fragments)` — backward compat for Deployment-like resources

### 2. Runtime JS Generation

During `husako init`, `.js` modules are generated alongside each `.d.ts` per group-version. Every schema with `x-kubernetes-group-version-kind` gets a builder class.

Generated format (e.g. `.husako/types/k8s/apps/v1.js`):

```javascript
import { _ResourceBuilder } from "husako/_base";

export class Deployment extends _ResourceBuilder {
  constructor() { super("apps/v1", "Deployment"); }
}
export class StatefulSet extends _ResourceBuilder {
  constructor() { super("apps/v1", "StatefulSet"); }
}
```

### 3. Rich `.d.ts` Builder Classes

Each builder class includes a typed `.spec()` override when the schema has a `spec` property with a `$ref` type. OpenAPI descriptions flow through as JSDoc comments.

```typescript
/** Deployment enables declarative updates for Pods and ReplicaSets. */
export class Deployment extends _ResourceBuilder {
  constructor();
  /** Set the resource specification. */
  spec(value: DeploymentSpec): this;
}
```

Resources without a spec property (e.g. `Namespace`) only get the base class methods.

### 4. Dynamic Module Loader (`HusakoK8sResolver`)

Three-tier module resolution:

1. `BuiltinResolver` — `husako`, `husako/_base` (embedded, always available)
2. `HusakoK8sResolver` — `k8s/*` → `.husako/types/k8s/{group}/{version}.js` (generated files)
3. `HusakoFileResolver` — `./`, `../` (user relative imports)

Error messages:
- No `generated_types_dir` → `"k8s modules require 'husako init' to be run first"`
- File not found → `"module 'k8s/apps/v1' not found. Run 'husako init' to generate k8s modules"`

### 5. CRD Support

CRDs in OpenAPI have `x-kubernetes-group-version-kind` but schema names don't match `io.k8s.api.*`. Fix: reclassify `SchemaLocation::Other` schemas that have GVK into their proper group-version.

```typescript
import { Deployment } from "k8s/apps/v1";              // built-in
import { Namespace } from "k8s/core/v1";                // core group
import { Cluster } from "k8s/postgresql.cnpg.io/v1";   // CRD
```

## Architecture Decisions

### `husako init` Required (Breaking Change)

`k8s/*` imports require `husako init` to have been run. No embedded fallback for built-in resources. Clear error message guides users.

### Backward Compatibility for `.resources()`

When `.spec()` is not called, `_render()` produces the Deployment-like nested structure (`spec.template.spec.containers[0].resources`). Existing usage continues to work identically.

### `_ResourceBuilder` Base Stays Embedded

`husako.js` and `husako_base.js` remain as embedded builtins (`include_str!`). Only `k8s/*` modules become dynamically loaded from generated files.

## Files Deleted

```
crates/husako-sdk/src/js/k8s_apps_v1.js    # replaced by generated files
crates/husako-sdk/src/js/k8s_core_v1.js    # replaced by generated files
```

## Files Created

```
crates/husako-dts/src/snapshots/..emit_builder_class_without_spec.snap
crates/husako-dts/src/snapshots/..emit_group_version_js_snapshot.snap
crates/husako-dts/src/snapshots/..apps_v1_js.snap
```

## Files Modified

```
crates/husako-sdk/src/js/husako_base.js      # _copy(), .spec(), .set(), _render() update
crates/husako-sdk/src/dts/husako_base.d.ts   # spec(), set() declarations
crates/husako-sdk/src/lib.rs                 # remove K8S_APPS_V1, K8S_CORE_V1
crates/husako-dts/src/emitter.rs             # emit_group_version_js(), find_spec_type(), typed .spec()
crates/husako-dts/src/lib.rs                 # remove RegisteredKind, CRD reclassification, .js generation
crates/husako-runtime-qjs/src/resolver.rs    # HusakoK8sResolver
crates/husako-runtime-qjs/src/lib.rs         # generated_types_dir in ExecuteOptions, rewire resolvers
crates/husako-core/src/lib.rs                # remove registered_kinds(), pass generated_types_dir
crates/husako-cli/tests/integration.rs       # new tests, write_k8s_modules helper
```

## Tests

### Unit Tests — `husako_base.js` (via `husako-runtime-qjs`)

- `spec_generic_setter` — `.spec({ replicas: 3 })` produces `spec: { replicas: 3 }`
- `set_generic_top_level` — `.set("data", { key: "val" })` produces `data: { key: "val" }`
- `spec_overrides_resources` — calling both `.spec()` and `.resources()` uses `.spec()`
- Backward compat — existing `.resources()` tests pass unchanged

### Unit Tests — `HusakoK8sResolver` (5 tests)

- Resolves `k8s/apps/v1` → `.husako/types/k8s/apps/v1.js` when file exists
- Error when file doesn't exist
- Error when `generated_types_dir` is `None`
- Ignores non-`k8s/` imports (falls through)
- CRD paths: `k8s/postgresql.cnpg.io/v1` resolves correctly

### Unit Tests — `emitter.rs`

- `emit_group_version_js_snapshot` — JS module output
- `emit_builder_class_without_spec` — Namespace-like resource without typed `.spec()`
- Updated snapshots for typed `.spec()` override

### Unit Tests — `husako-dts/src/lib.rs`

- `.js` files generated alongside `.d.ts`
- CRD schemas reclassified by GVK into proper group-version

### Integration Tests (5 new)

- `render_k8s_import_without_init` — `k8s/apps/v1` without init → exit 4 + helpful error
- `init_generates_js_modules` — after init, `.husako/types/k8s/apps/v1.js` exists with `class Deployment`
- `render_with_generated_modules` — init then render with `k8s/apps/v1` import → exit 0
- `generic_spec_setter` — non-Deployment resource using `.spec()` → correct YAML
- `generic_set_configmap` — ConfigMap using `.set("data", ...)` → correct YAML

## Acceptance Criteria

- [x] All K8s resources with GVK get runtime `.js` builders (not just 4 hardcoded)
- [x] CRDs with GVK get `.js` + `.d.ts` builders automatically
- [x] Generated `.d.ts` has typed `.spec()` with OpenAPI descriptions in JSDoc
- [x] `k8s/*` imports require `husako init` — clear error without it
- [x] `HusakoK8sResolver` resolves to `.husako/types/k8s/*.js`
- [x] `.spec()` and `.set()` work for generic resources
- [x] `.resources()` backward compat — existing tests pass unchanged
- [x] No hardcoded resource types remain in codebase
- [x] All 183 workspace tests pass
- [x] clippy clean, fmt clean
