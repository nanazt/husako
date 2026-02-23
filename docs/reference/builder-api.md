# Builder API

This page is the complete reference for the husako builder DSL.

## Builder hierarchy

Three types of builders exist, each with different responsibilities:

### _ResourceBuilder

Top-level Kubernetes resources — schemas that carry `apiVersion` and `kind` (i.e., those with `x-kubernetes-group-version-kind` in the OpenAPI spec).

| Method | Description |
|--------|-------------|
| `.metadata(fragment)` | Sets metadata. Accepts MetadataFragment or plain object. |
| `.spec(value)` | Replaces the full spec object. Clears per-property parts. |
| `.set(key, value)` | Sets an arbitrary top-level field outside spec. |
| `.resources(...fragments)` | Sets container resource requirements. |

Per-spec-property methods (`.replicas()`, `.selector()`, `.template()`, etc.) are generated from the OpenAPI spec.

Each calls an internal `_setSpec()` and returns a new instance.

Deep-path shortcuts (`.containers()`, `.initContainers()`) are generated for resources that have a `template` property.

They call an internal `_setDeep()` and reach into `spec.template.spec`.

**Examples:** `Deployment`, `Service`, `Namespace`, `StatefulSet`, `DaemonSet`, `ConfigMap`

### _SchemaBuilder

Intermediate types with complex nested properties but no `apiVersion`/`kind`.

Generated for schemas that have at least one property referencing another schema.

| Method | Description |
|--------|-------------|
| `_set(key, value)` | Sets one property. Returns a new instance. |
| `_toJSON()` | Resolves all nested fragments and returns a plain object. |

Per-property methods (`.name()`, `.image()`, `.ports()`, etc.) are generated and call `_set()` internally.

**Examples:** `Container`, `PodSpec`, `PodTemplateSpec`, `DeploymentSpec`

### Fragment builders

Hand-crafted builders in the `"husako"` module for common cross-resource concerns.

| Fragment | Factory functions | Methods |
|----------|-------------------|---------|
| MetadataFragment | `metadata()` | `.name(v)`, `.namespace(v)`, `.label(k, v)`, `.annotation(k, v)` |
| ResourceListFragment | `cpu(v)`, `memory(v)` | `.cpu(v)`, `.memory(v)` |
| ResourceRequirementsFragment | `requests(rl)`, `limits(rl)` | `.requests(rl)`, `.limits(rl)` |

---

## Copy-on-write

Every chainable method returns a **new** builder instance.

The original is never mutated.

```typescript
const base = Deployment().metadata(metadata().name("base")).replicas(1);
const prod = base.replicas(3);   // base still has replicas=1
const dev  = base.replicas(1);   // independent from prod
```

This makes builders safe to use as templates and share across multiple resource definitions.

---

## Merge semantics

`merge(items)` merges an array of same-typed fragments.

| Fragment type | Scalar fields | Map fields |
|--------------|---------------|------------|
| MetadataFragment | `name`, `namespace`: last non-null wins | `labels`, `annotations`: deep-merge by key (later entries override earlier ones) |
| ResourceListFragment | `cpu`, `memory`: last non-null wins | — |
| Other types | Returns last item in array | — |

Arrays are **replaced**, not concatenated.

```typescript
const base = metadata().name("svc").label("app", "web");
const env  = metadata().label("env", "prod");
merge([base, env]);
// → { name: "svc", labels: { app: "web", env: "prod" } }
```

---

## Render precedence

When `_render()` builds the `spec` field, it checks three sources in order:

| Priority | Source | Set by | Behavior |
|----------|--------|--------|----------|
| 1 | `_spec` | `.spec(obj)` | Full replacement. Per-property parts are ignored. |
| 2 | `_specParts` | Per-property methods, `.containers()`, etc. | Accumulated per property. Merged with `_resources` if present. |
| 3 | `_resources` | `.resources()` | Creates `template.spec.containers[0].resources`. |

Calling `.spec(obj)` clears any per-property parts.

Calling per-property methods clears any previously set `.spec()`.

---

## Fragment builders in detail

### MetadataFragment

```typescript
import { metadata } from "husako";

metadata()
  .name("my-resource")
  .namespace("production")
  .label("app", "my-resource")
  .label("version", "1.2.3")
  .annotation("team", "platform")
```

### Quantity fragments

```typescript
import { cpu, memory, requests, limits } from "husako";

// CPU quantity
cpu("500m")   // pass-through string
cpu(1)        // integer → "1"
cpu(0.5)      // float → "500m"

// Memory quantity
memory("512Mi")  // pass-through string
memory(2)        // number → "2Gi"

// Combining into ResourceRequirements
requests(cpu(0.25).memory("128Mi"))
  .limits(cpu(1).memory("256Mi"))
```

Full quantity normalization table:

**cpu(v):**

| Input | Output | Example |
|-------|--------|---------|
| string | pass-through | `"500m"` → `"500m"` |
| integer | `String(v)` | `1` → `"1"` |
| float | `round(v * 1000) + "m"` | `0.5` → `"500m"` |

**memory(v):**

| Input | Output | Example |
|-------|--------|---------|
| string | pass-through | `"512Mi"` → `"512Mi"` |
| number | `v + "Gi"` | `2` → `"2Gi"` |

---

## build() strict JSON contract

`build()` validates every rendered resource against a strict JSON contract before emitting YAML.

**Banned values in output:**

| Type | Example |
|------|---------|
| `undefined` | `{ key: undefined }` |
| `bigint` | `BigInt(9007199254740991n)` |
| `symbol` | `Symbol("foo")` |
| Functions | `() => {}` |
| Class instances | `new MyClass()` |
| `Date` | `new Date()` |
| `Map` | `new Map()` |
| `Set` | `new Set()` |
| `RegExp` | `/foo/` |
| Cyclic references | Object that references itself |

If any banned value appears in the rendered output, husako exits with code 7 and reports the JSON path of the offending value.

**build() rules:**

- Must be called exactly once per entry file. Zero or multiple calls → exit 7.
- Accepts a single builder or an array of builders.
- Every item must have a `_render()` method. Plain objects throw `TypeError`.

---

## TypeScript declaration shape

In generated `.d.ts` files, factory functions use declaration merging (interface + function):

```typescript
export interface Deployment extends _ResourceBuilder {
  replicas(value: number): this;
  selector(value: LabelSelector | LabelSelectorSpec): this;
  template(value: PodTemplateSpec | PodTemplateSpecSpec): this;
  containers(value: (Container | ContainerSpec)[]): this;
  initContainers(value: (Container | ContainerSpec)[]): this;
}
export function Deployment(): Deployment;
```

The factory function and interface share the same name, so `Deployment()` returns a typed `Deployment` instance.
