# Milestone 3: Runtime SDK (Builders, Fragments, Merge)

**Status**: Completed
**Commits**: `f02f7bb`, `8437be4`

## Goal

Implement the husako runtime SDK providing chainable, immutable builder/fragment APIs in JavaScript, enabling the canonical authoring style from the plan.

## Deliverables

### Fragment Builders (`"husako"` module)

- `name(v)` -> MetadataFragment
- `namespace(v)` -> MetadataFragment
- `label(k, v)` -> MetadataFragment
- `annotation(k, v)` -> MetadataFragment
- `merge(fragments[])` -> merged fragment (deep-merge labels/annotations, last-wins scalars)

### Resource Builders (`"husako"` module)

- `cpu(v)` -> ResourceListFragment (normalizes: 0.5 -> "500m", integer -> string)
- `memory(v)` -> ResourceListFragment (normalizes: number -> "NGi", string validated)
- `requests(rl)` -> ResourceRequirementsFragment
- `limits(rl)` -> ResourceRequirementsFragment
- `requests()` and `limits()` are chainable on ResourceRequirementsFragment

### K8s Resource Builders

- `"k8s/apps/v1"`: `Deployment` class
- `"k8s/core/v1"`: `Namespace`, `Service`, `ConfigMap` classes
- All extend `_ResourceBuilder` base class with `.metadata()`, `.resources()`, `.toJSON()`

## Architecture Decisions

### Immutable Fragments (Copy-on-Write)

Every fragment method returns a **new object** to ensure safe reuse:

```js
const base = label("env", "dev");
const a = base.label("team", "a");
const b = base.label("team", "b");
// base, a, b are independent — no mutation
```

Implementation: each method spreads existing data into a new object, then applies the change.

### Merge Semantics

- **Scalars** (name, namespace): last argument wins
- **Maps** (labels, annotations): deep-merge by key, last wins on conflicts
- **Arrays**: replace (no implicit concat)

### Quantity Normalization Policy

- `cpu(0.5)` -> `"500m"` (millicores for fractionals)
- `cpu(1)` -> `"1"` (integer as-is)
- `memory(4)` -> `"4Gi"` (numeric memory defaults to Gi)
- `memory("2Gi")` -> `"2Gi"` (string passed through)

### Builder Base Class (`_ResourceBuilder`)

```js
class _ResourceBuilder {
  constructor(apiVersion, kind) { ... }
  metadata(fragment) { ... }     // merges MetadataFragment
  resources(...fragments) { ... } // merges ResourceRequirementsFragments
  toJSON() { ... }               // produces the final plain object
}
```

All K8s resource classes inherit from this and just set apiVersion/kind.

### Module Registration

Builtin modules are registered in the QuickJS runtime loader:
- `"husako"` -> `husako.js` (SDK implementation)
- `"k8s/apps/v1"` -> `k8s_apps_v1.js`
- `"k8s/core/v1"` -> `k8s_core_v1.js`

The JS source is embedded via `include_str!()` in `husako-sdk`.

## Files Created/Modified

```
crates/husako-sdk/src/js/husako.js       # Full SDK: fragments, merge, cpu, memory, etc.
crates/husako-sdk/src/js/husako_base.js  # _ResourceBuilder base class
crates/husako-sdk/src/js/k8s_apps_v1.js  # Deployment class
crates/husako-sdk/src/js/k8s_core_v1.js  # Namespace, Service, ConfigMap classes
crates/husako-sdk/src/lib.rs             # include_str! constants
crates/husako-sdk/src/dts/husako.d.ts    # TypeScript declarations for "husako"
crates/husako-runtime-qjs/src/lib.rs     # Module loader updated for new builtins
examples/canonical.ts                    # Canonical example from the plan
```

## Tests

### Integration Tests

- `render_canonical` — canonical example renders with all builder features
- `render_canonical_snapshot` — snapshot of canonical YAML output
- `metadata_fragment_reuse` — fragments are immutable (base, a, b independent)
- `merge_labels_deep` — merge combines multiple label fragments
- `cpu_normalization` — `cpu(0.5)` -> `500m`
- `memory_normalization` — `memory(4)` -> `4Gi`
- `k8s_core_v1_namespace` — Namespace builder works
- `backward_compat_plain_objects` — plain object literals still work

## Acceptance Criteria

- [x] Canonical TS example renders correct YAML
- [x] Fragments are immutable and reusable
- [x] merge() deep-merges labels, last-wins scalars
- [x] cpu/memory normalization works
- [x] Deployment, Namespace, Service, ConfigMap builders work
- [x] requests()/limits() chainable on ResourceRequirementsFragment
- [x] Plain object literals still work (backward compat)
