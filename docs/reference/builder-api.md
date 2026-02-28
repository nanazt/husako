# Builder API

This page is the complete reference for the husako builder DSL.

## Builder hierarchy

Two types of builders exist:

### _ResourceBuilder

Top-level Kubernetes resources — schemas that carry `apiVersion` and `kind` (i.e., those with `x-kubernetes-group-version-kind` in the OpenAPI spec).

| Method | Description |
|--------|-------------|
| `.metadata(chain)` | Sets metadata. Accepts a `MetadataChain` or `SpecFragment`. |
| `.containers(items)` | Sets containers. Accepts a `ContainerChain[]` or `SpecFragment[]`. |
| `.spec(value)` | Replaces the full spec object. Clears per-property parts. |
| `.set(key, value)` | Sets an arbitrary top-level field outside spec. |
| `.resources(r)` | Sets container resource requirements. Accepts a `ResourceRequirementsChain`. |

Per-spec-property methods (`.replicas()`, `.selector()`, `.template()`, etc.) are generated from the OpenAPI spec.

Each calls an internal `_setSpec()` and returns a new instance.

Deep-path shortcuts (`.containers()`, `.initContainers()`) reach into `spec.template.spec`.

**Examples:** `Deployment`, `Service`, `Namespace`, `StatefulSet`, `DaemonSet`, `ConfigMap`

### Chain starters and SpecFragment

Chain starter functions are exported from schema modules (`"k8s/meta/v1"`, `"k8s/core/v1"`, etc.).

Each starter creates a `SpecFragment` that records field values. Fragments chain — each call returns the same fragment so you can keep adding fields:

```typescript
import { name, namespace, label } from "k8s/meta/v1";   // ObjectMeta starters
import { name, image, imagePullPolicy } from "k8s/core/v1"; // Container starters

// MetadataChain context — passed to .metadata()
name("nginx").namespace("default").label("app", "nginx")

// ContainerChain context — passed to .containers()
name("nginx").image("nginx:1.25").imagePullPolicy("Always")
```

`SpecFragment` is compatible with both `.metadata()` and `.containers()` — the call site determines how the fragment is consumed.

---

## Copy-on-write

Every chainable method on `_ResourceBuilder` returns a **new** builder instance.

The original is never mutated:

```typescript
const base = Deployment()
  .metadata(name("base").namespace("default"))
  .replicas(1);

const prod = base.replicas(3);   // base still has replicas=1
const dev  = base.replicas(1);   // independent from prod
```

Chain fragments (`SpecFragment`) mutate in place — they are ephemeral builder objects consumed once by `.metadata()` or `.containers()`.

---

## Render precedence

When `_render()` builds the `spec` field, it checks three sources in order:

| Priority | Source | Set by | Behavior |
|----------|--------|--------|----------|
| 1 | `_spec` | `.spec(obj)` | Full replacement. Per-property parts are ignored. |
| 2 | `_specParts` | Per-property methods, `.containers()`, etc. | Accumulated per property. |
| 3 | `_resources` | `.resources()` | Creates `template.spec.containers[0].resources`. |

Calling `.spec(obj)` clears any per-property parts.

Calling per-property methods clears any previously set `.spec()`.

---

## Chain starters in detail

### Metadata — `k8s/meta/v1`

```typescript
import { name, namespace, label, annotation } from "k8s/meta/v1";

name("my-resource")
  .namespace("production")
  .label("app", "my-resource")
  .label("version", "1.2.3")
  .annotation("team", "platform")
```

### Containers — `k8s/core/v1`

```typescript
import { name, image, imagePullPolicy } from "k8s/core/v1";

name("web")
  .image("nginx:1.25")
  .imagePullPolicy("Always")
```

### Quantity starters — `k8s/core/v1`

```typescript
import { cpu, memory, requests } from "k8s/core/v1";

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

## husako.build() strict JSON contract

`husako.build()` validates every rendered resource against a strict JSON contract before emitting YAML.

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

**husako.build() rules:**

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
  containers(value: ContainerChain[]): this;
  initContainers(value: ContainerChain[]): this;
}
export function Deployment(): Deployment;
```

Chain interfaces in `_chains.d.ts`:

```typescript
export interface MetadataChain {
  name(v: string): MetadataChain;
  namespace(v: string): MetadataChain;
  label(k: string, v: string): MetadataChain;
  annotation(k: string, v: string): MetadataChain;
}

export interface ContainerChain {
  name(v: string): ContainerChain;
  image(v: string): ContainerChain;
  imagePullPolicy(v: "Always" | "IfNotPresent" | "Never"): ContainerChain;
  resources(r: ResourceRequirementsChain): ContainerChain;
}

// SpecFragment extends both — compatible with .metadata() and .containers()
export interface SpecFragment extends MetadataChain, ContainerChain {
  name(v: string): SpecFragment;
  namespace(v: string): SpecFragment;
  image(v: string): SpecFragment;
}
```
