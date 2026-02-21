# Milestone 10 — Generic Builder Pattern Generation

Date: 2026-02-22 (Asia/Seoul)
Status: **Completed**

---

## Goal

Replace raw `.spec({...})` object literals with **per-property chainable methods** on resource builders, and generate standalone **schema builder classes** for intermediate types (Container, PodSpec, etc.) that have sufficiently complex properties.

---

## Key Design: Generic `_SchemaBuilder` base class

Instead of hardcoding spec property promotion, introduce a generic `_SchemaBuilder` base class in the SDK. The emitter generates subclasses for any schema with sufficiently complex properties.

```javascript
// husako_base.js — new generic builder for non-resource schemas
export class _SchemaBuilder {
  constructor(init) { this._props = init ? Object.assign({}, init) : {}; }
  _copy() { const n = new this.constructor(); n._props = Object.assign({}, this._props); return n; }
  _set(key, value) { const n = this._copy(); n._props[key] = value; return n; }
  _toJSON() { return _resolveFragments(this._props); }
}
```

The emitter generates per-property chainable methods for both:
- **Resource builders** (`_ResourceBuilder` subclasses) — spec properties promoted to resource level via `_setSpec()`
- **Schema builders** (`_SchemaBuilder` subclasses) — standalone builders for intermediate types via `_set()`

Same generation logic, two targets.

---

## Generation heuristic: which schemas get builders?

Not every schema needs a builder. `LabelSelector` with just `matchLabels: Record<string, string>` is fine as a plain object. A schema benefits from a builder when it has **deep nesting** — properties that reference other complex types.

**Rule: Generate a builder for any schema that has at least one property with a `Ref` or `Array(Ref)` type.**

This captures: Container (refs ResourceRequirements, Probe, etc.), PodSpec (refs Container[], Volume[]), PodTemplateSpec (refs PodSpec), DeploymentSpec (refs PodTemplateSpec, LabelSelector), ServiceSpec (refs ServicePort[]).

This skips: LabelSelector (only map<string,string>), ObjectMeta (only primitives+maps), simple quantity types.

---

## Target API

```typescript
import { Deployment } from "k8s/apps/v1";
import { Container } from "k8s/core/v1";
import { name, cpu, memory, requests, limits } from "husako";

const c = new Container()
  .name("nginx")
  .image("nginx:1.25")
  .ports([{ containerPort: 80 }])
  .resources(requests(cpu("250m")).limits(cpu("500m")));

const nginx = new Deployment()
  .metadata(name("nginx").label("app", "nginx"))
  .replicas(3)
  .selector({ matchLabels: { app: "nginx" } })   // plain object — readable enough
  .containers([c]);
```

- `Deployment` extends `_ResourceBuilder`, gets spec property methods (`.replicas()`, `.selector()`, `.template()`, etc.)
- `Container` extends `_SchemaBuilder`, gets property methods (`.name()`, `.image()`, `.ports()`, `.env()`, etc.)
- `selector` takes a plain object because `LabelSelector` is simple enough — no builder generated
- `.containers()` is a deep-path shortcut generated on workload resources

---

## Phases

### Phase 1: `_SchemaBuilder` base class + `_resolveFragments` + `_setSpec`/`_setDeep` on `_ResourceBuilder`

**Files:**
- `crates/husako-sdk/src/js/husako_base.js` — add `_SchemaBuilder`, `_resolveFragments()`, `_mergeDeep()`, add `_specParts`/`_setSpec()`/`_setDeep()` to `_ResourceBuilder`, update `_render()` and `_copy()`
- `crates/husako-sdk/src/dts/husako_base.d.ts` — add `_SchemaBuilder` declaration

`_ResourceBuilder` changes:
- New field `_specParts` (accumulated via `_setSpec`)
- `_setSpec(key, value)` — sets one spec property, clears whole-object `_spec`
- `_setDeep(path, value)` — sets nested spec path (for `.containers()` shortcut)
- `.spec({...})` clears `_specParts` (mutual exclusion: last-called family wins)
- `_render()` resolves `_specParts` via `_resolveFragments()` (recursive `_toJSON()` calls)
- Render precedence: `_spec` > `_specParts` > `_resources` (legacy)

### Phase 2: Generic builder generation in emitter

**Files:**
- `crates/husako-dts/src/emitter.rs` — new functions + modify existing
- `crates/husako-dts/src/schema.rs` — add `has_complex_property()` helper

New emitter functions:
- `should_generate_builder(schema) -> bool` — checks if schema has Ref/Array(Ref) properties and no GVK
- `find_spec_schema(schema, all_schemas) -> Option<&SchemaInfo>` — finds the Spec type for a GVK resource
- `has_pod_template(schema) -> bool` — checks if spec has `template: Ref(PodTemplateSpec)`
- `emit_property_methods_dts(schema) -> String` — generates typed chainable method declarations for each property
- `emit_property_methods_js_set(schema) -> String` — generates `prop(v) { return this._set("prop", v); }` for schema builders
- `emit_property_methods_js_spec(schema) -> String` — generates `prop(v) { return this._setSpec("prop", v); }` for resource builders
- `emit_schema_builder_class(schema) -> String` — emits `_SchemaBuilder` subclass (`.d.ts`)
- `emit_schema_builder_js(schema) -> String` — emits `_SchemaBuilder` subclass (`.js`)

Modify existing:
- `emit_builder_class(schema, api_version, all_schemas)` — now also emits per-spec-property methods
- `emit_group_version_js(schemas)` — emits per-spec-property methods + schema builder classes + conditional `_SchemaBuilder` import
- `emit_group_version(schemas, common_names)` — emits schema builder class declarations

**Skip list for generated methods:** `status` (server-managed), `apiVersion`/`kind`/`metadata` (on resource builders, handled by `_ResourceBuilder` base)

For resources with GVK whose spec has a `template` property referencing PodTemplateSpec, also generate:
- `.containers(v)` → `_setDeep("template.spec.containers", v)` (`.js`)
- `.initContainers(v)` → `_setDeep("template.spec.initContainers", v)` (`.js`)

### Phase 3: Pass schema context through generation pipeline

**Files:**
- `crates/husako-dts/src/lib.rs` — modify `generate()` to pass `all_schemas` to emitter functions

Change: pass `all_schemas: &[&SchemaInfo]` (all schemas in the current group-version module) to `emit_builder_class()` and `emit_group_version_js()`.

Updated JS emission condition to also trigger for modules containing schema builders.

### Phase 4: Update templates, examples, and tests

**Files:**
- All template files in `crates/husako-sdk/src/templates/`
- `examples/canonical.ts`
- Integration test mock modules in `write_k8s_modules()`
- All insta snapshot files

Updated to use new builder style with per-property methods.

---

## Key files

| File | Change |
|------|--------|
| `crates/husako-sdk/src/js/husako_base.js` | Add `_SchemaBuilder`, `_resolveFragments`, `_mergeDeep`, `_specParts`, `_setSpec`, `_setDeep`, update `_render`/`_copy` |
| `crates/husako-sdk/src/dts/husako_base.d.ts` | Add `_SchemaBuilder` type declaration |
| `crates/husako-sdk/src/js/husako.js` | Add `_toJSON()` to `rrMethods` for fragment resolution |
| `crates/husako-dts/src/emitter.rs` | Generic builder generation, per-property methods, schema builder classes |
| `crates/husako-dts/src/schema.rs` | `has_complex_property()` + `is_complex_type()` helpers |
| `crates/husako-dts/src/lib.rs` | Pass all_schemas through pipeline, trigger JS emit for schema builders |
| `crates/husako-sdk/src/templates/**` | Updated to use per-property methods |
| `examples/canonical.ts` | Uses `.replicas()`, `.selector()`, `.template()`, `.containers()` |
| `crates/husako-cli/tests/integration.rs` | Updated mock modules + 6 new tests |

---

## Verification

1. `cargo test --workspace --all-features` — 209 tests pass (26 new)
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean
3. `cargo fmt --all --check` — clean
4. `cargo run -- render examples/basic.ts` — identical YAML
5. `cargo run -- render examples/canonical.ts` — identical YAML

---

## New integration tests added

1. `builder_spec_property_methods` — `.replicas()`, `.selector()`, `.template()` produce correct YAML
2. `builder_set_deep_merges` — `.containers()` merges into `template.spec` correctly
3. `builder_spec_overrides_spec_parts` — `.spec({...})` overrides per-property methods
4. `builder_spec_parts_override_resources` — per-property methods override `.resources()`
5. `builder_copy_on_write_isolation` — chaining creates independent copies
6. `builder_service_spec_properties` — Service builder `.type()`, `.selector()`, `.ports()` work
