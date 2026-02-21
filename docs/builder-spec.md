# Builder DSL Specification

This document defines the rules for the husako builder DSL. All user-facing code, examples, templates, and generated types must follow these rules. Implementation that violates any rule listed here is a bug.

---

## 1. Authoring Style

Users write Kubernetes resources using **builder chains**. No `new` keyword, no plain object literals for resource structure.

```typescript
import { deployment } from "k8s/apps/v1";
import { container } from "k8s/core/v1";
import { selector } from "k8s/_common";
import { metadata, cpu, memory, requests, limits, build } from "husako";

const nginx = deployment()
  .metadata(metadata().name("nginx").label("app", "nginx"))
  .replicas(3)
  .selector(selector().matchLabels({ app: "nginx" }))
  .containers([
    container()
      .name("nginx")
      .image("nginx:1.25")
      .resources(
        requests(cpu("250m").memory("128Mi"))
          .limits(cpu("500m").memory("256Mi"))
      )
  ]);

build([nginx]);
```

**Rules:**

- Every builder class exports a lowercase factory function: `Deployment` class → `deployment()` function.
- `metadata()` is the entry point for metadata chains. `metadata().name("x").label("k", "v")`.
- `name()`, `label()`, `namespace()`, `annotation()` remain as shorthand aliases for `metadata().name()` etc.
- Plain objects are allowed only as leaf values where no builder exists (e.g., `matchLabels`, `nodeSelector`).

---

## 2. Builder Hierarchy

Three layers of builders exist. Each has different responsibilities.

### _ResourceBuilder

Top-level Kubernetes resources with `apiVersion` and `kind` (schemas that carry `x-kubernetes-group-version-kind`).

| Method | Behavior |
|--------|----------|
| `metadata(fragment)` | Sets metadata. Accepts MetadataFragment or plain object. |
| `spec(value)` | Sets full spec object. Clears `_specParts`. |
| `_setSpec(key, value)` | Sets one spec property. Clears `_spec`. |
| `_setDeep(path, value)` | Sets nested spec path via deep merge. Clears `_spec`. |
| `set(key, value)` | Sets arbitrary top-level field outside spec. |
| `resources(...fragments)` | Sets container resource requirements from fragments. |
| `_render()` | Serializes to plain Kubernetes object. |

Generated per-spec-property methods (e.g., `.replicas()`, `.selector()`, `.template()`) call `_setSpec()` internally.

Deep-path shortcuts (e.g., `.containers()`, `.initContainers()`) call `_setDeep()` internally.

**Examples:** `Deployment`, `Service`, `Namespace`, `StatefulSet`, `DaemonSet`, `ConfigMap`

### _SchemaBuilder

Intermediate types that have complex nested properties but no GVK. Generated for schemas with at least one `Ref` or `Array(Ref)` property.

| Method | Behavior |
|--------|----------|
| `_set(key, value)` | Sets one property. Returns new instance. |
| `_toJSON()` | Resolves all nested fragments and returns plain object. |

Generated per-property methods (e.g., `.name()`, `.image()`, `.ports()`) call `_set()` internally.

**Examples:** `Container`, `PodSpec`, `PodTemplateSpec`, `DeploymentSpec`

### Fragment Builders

Hand-crafted builders in the `"husako"` module for common cross-cutting concerns.

| Fragment | Factory | Chainable methods |
|----------|---------|-------------------|
| MetadataFragment | `metadata()` | `.name(v)`, `.namespace(v)`, `.label(k, v)`, `.annotation(k, v)` |
| ResourceListFragment | `cpu(v)`, `memory(v)` | `.cpu(v)`, `.memory(v)` |
| ResourceRequirementsFragment | `requests(rl)`, `limits(rl)` | `.requests(rl)`, `.limits(rl)` |

---

## 3. Import Rules

| Source | Exports |
|--------|---------|
| `k8s/<group>/<version>` | Resource builder factories (`deployment`, `statefulSet`) + schema builder factories (`container`, `podSpec`) |
| `k8s/_common` | Common type builder factories (`selector`, `objectMeta`) for `io.k8s.apimachinery.*` schemas |
| `"husako"` | `metadata`, `cpu`, `memory`, `requests`, `limits`, `merge`, `build` |
| `"husako"` (aliases) | `name`, `label`, `namespace`, `annotation` — shorthand for `metadata().name()` etc. |

---

## 4. Copy-on-Write

Every chainable method returns a **new** builder instance. The original is never mutated.

```typescript
const base = deployment().metadata(metadata().name("base")).replicas(1);
const prod = base.replicas(3);   // base is unchanged, still replicas=1
const dev  = base.replicas(1);   // independent from prod
```

**Implementation:** each method calls `_copy()` which shallow-clones all instance fields into a new object of the same class.

---

## 5. Render Precedence

`_render()` builds the `spec` field using three mutually exclusive sources, checked in order:

| Priority | Source | Set by | Behavior |
|----------|--------|--------|----------|
| 1 | `_spec` | `.spec(obj)` | Full replacement. Ignores `_specParts`. |
| 2 | `_specParts` | `_setSpec()`, `_setDeep()` | Accumulated per-property. Merged with `_resources` if present. |
| 3 | `_resources` | `.resources()` | Creates `template.spec.containers[0].resources` structure. |

**Mutual exclusion:**

- `.spec(obj)` sets `_spec`, clears `_specParts` to `null`.
- `_setSpec(k, v)` and `_setDeep(path, v)` set `_specParts`, clear `_spec` to `null`.
- When `_specParts` and `_resources` both exist: resources are merged into `containers[0].resources` inside the rendered spec.

---

## 6. Merge Semantics

`merge(items)` merges an array of same-typed fragments.

| Fragment type | Scalar fields | Map fields |
|--------------|---------------|------------|
| MetadataFragment | `_name`, `_namespace`: last non-null wins | `_labels`, `_annotations`: deep-merge by key (later overrides) |
| ResourceListFragment | `_cpu`, `_memory`: last non-null wins | — |
| Other types | Returns last item in array | — |

Arrays are **replaced**, not concatenated.

```typescript
const base = metadata().name("svc").label("app", "web");
const env  = metadata().label("env", "prod");
merge([base, env]);
// → { name: "svc", labels: { app: "web", env: "prod" } }
```

---

## 7. Quantity Normalization

Applied by `cpu()` and `memory()` factory functions.

### cpu(v)

| Input | Output | Example |
|-------|--------|---------|
| `string` | pass-through | `"500m"` → `"500m"` |
| `integer` | `String(v)` | `1` → `"1"` |
| `float` | `round(v * 1000) + "m"` | `0.5` → `"500m"` |

### memory(v)

| Input | Output | Example |
|-------|--------|---------|
| `string` | pass-through | `"512Mi"` → `"512Mi"` |
| `number` | `v + "Gi"` | `2` → `"2Gi"` |

---

## 8. Code Generation Heuristic

The emitter decides which OpenAPI schemas get builder classes.

### Resource builders (_ResourceBuilder subclass)

**Condition:** schema has `x-kubernetes-group-version-kind` extension.

Generated output:
- Class extending `_ResourceBuilder` with `constructor(apiVersion, kind)`
- Per-spec-property methods from the spec schema (calls `_setSpec`)
- Factory function (lowercase class name)

**Skip list for spec property methods:** `status`, `apiVersion`, `kind`, `metadata`

### Schema builders (_SchemaBuilder subclass)

**Condition:** schema has NO GVK AND has at least one property with `Ref` or `Array(Ref)` type.

Generated output:
- Class extending `_SchemaBuilder`
- Per-property chainable methods (calls `_set`)
- Factory function (lowercase class name)

### Deep-path shortcuts

**Condition:** resource spec has a `template` property referencing `PodTemplateSpec`.

Generated methods:
- `.containers(v)` → `_setDeep("template.spec.containers", v)`
- `.initContainers(v)` → `_setDeep("template.spec.initContainers", v)`

**Applicable to:** Deployment, StatefulSet, DaemonSet, Job, ReplicaSet

### Factory function naming

Class name with first character lowercased:

| Class | Factory |
|-------|---------|
| `Deployment` | `deployment()` |
| `Container` | `container()` |
| `LabelSelector` | `labelSelector()` |
| `PodTemplateSpec` | `podTemplateSpec()` |

Convenience aliases (e.g., `selector` for `labelSelector`) may be added per module.

---

## 9. Fragment Resolution

`_resolveFragments(obj)` recursively unwraps all nested builders before serialization.

**Algorithm:**
1. `null` / `undefined` / primitive → pass-through
2. Array → map each element through `_resolveFragments`
3. Object with `_toJSON()` → call it, then resolve the result recursively
4. Plain object → shallow-copy, resolve each value recursively

All builder types must implement either `_toJSON()` (SchemaBuilder, fragments) or `_render()` (ResourceBuilder). The `build()` function calls `_render()` on top-level items; `_resolveFragments` handles nested builders via `_toJSON()`.

---

## 10. build() Contract

```typescript
build(input: { _render(): any } | { _render(): any }[]): void
```

**Rules:**
- Must be called **exactly once** per entrypoint. Zero calls → exit 7. Multiple calls → exit 7.
- Normalizes single object to `[object]`.
- For each item: calls `_render()`. Items without `_render()` throw `TypeError`.
- Rendered output must pass strict JSON validation (default `--strict-json=true`).

**Forbidden in output:** `undefined`, `bigint`, `symbol`, functions, class instances, `Date`, `Map`, `Set`, `RegExp`, cyclic references.

---

## 11. Template Reuse

Builders are immutable, so any builder instance can be stored and reused as a template.

```typescript
import { deployment } from "k8s/apps/v1";
import { container, podTemplate } from "k8s/core/v1";
import { metadata, build } from "husako";

const webPod = podTemplate()
  .metadata(metadata().label("tier", "web"))
  .containers([
    container().name("web").image("nginx:1.25")
  ]);

const prod = deployment()
  .metadata(metadata().name("web-prod"))
  .replicas(5)
  .template(webPod);

const staging = deployment()
  .metadata(metadata().name("web-staging"))
  .replicas(1)
  .template(webPod);

build([prod, staging]);
```

Both `prod` and `staging` share the same pod template. Because of copy-on-write, modifying one never affects the other.
