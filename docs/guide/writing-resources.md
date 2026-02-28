# Writing Resources

husako resources are written using a builder DSL.

No YAML, no plain objects for resource structure — just chained method calls that compile to typed Kubernetes manifests.

---

## Factory functions

Every resource type is a PascalCase factory function.

Call it with no arguments to get an empty builder:

```typescript
import { Deployment } from "k8s/apps/v1";
import { Service } from "k8s/core/v1";

const deploy = Deployment();
const svc = Service();
```

No `new` keyword, no `new Deployment()`. The factory is the API.

---

## Chaining methods

Every method on a builder returns a new builder instance.

You build up a resource by chaining:

```typescript
const deploy = Deployment()
  .replicas(3)
  .selector(LabelSelector().matchLabels({ app: "web" }));
```

Each property has a corresponding method generated from the OpenAPI spec.

Method names match the spec field names in camelCase.

---

## Metadata — chain starters from `k8s/meta/v1`

Metadata is set using chain starter functions from `"k8s/meta/v1"`.

Each starter creates a fragment compatible with `.metadata()`:

```typescript
import { name, namespace, label, annotation } from "k8s/meta/v1";

const deploy = Deployment()
  .metadata(
    name("my-app")
      .namespace("production")
      .label("app", "my-app")
      .label("version", "1.2.3")
      .annotation("team", "platform")
  );
```

The starters chain — each call returns the same fragment so you can keep adding fields:

```typescript
name("my-app").namespace("default").label("app", "my-app")
```

Store a fragment in a variable and reuse it:

```typescript
import { name, namespace, label } from "k8s/meta/v1";

const meta = name("web").namespace("production").label("app", "web");
const prod = Deployment().metadata(meta).replicas(5);
const staging = Deployment().metadata(meta).replicas(1);
```

---

## Containers — chain starters from `k8s/core/v1`

Container fields are set using chain starters from `"k8s/core/v1"`:

```typescript
import { name, image, imagePullPolicy } from "k8s/core/v1";

Deployment()
  .containers([
    name("web")
      .image("nginx:1.25")
      .imagePullPolicy("Always")
  ])
```

Pass an array of chain fragments to `.containers()`. Each fragment becomes one container.

---

## Duplicate imports in `.husako` files

`.husako` files allow importing the same name from multiple schema modules without aliasing.

Both `name` functions below are valid — the call site determines which is used:

```typescript
import { name, namespace, label } from "k8s/meta/v1";   // ObjectMeta starters
import { name, image } from "k8s/core/v1";               // Container starters

Deployment()
  .metadata(name("nginx").namespace("default"))   // uses k8s/meta/v1 name
  .containers([name("nginx").image("nginx:1.25")]) // uses k8s/core/v1 name
```

The husako LSP suppresses the TypeScript duplicate identifier warning for `k8s/*` imports.

---

## Resource quantities

CPU and memory use dedicated chain starter functions with automatic normalization:

```typescript
import { cpu, memory, requests } from "k8s/core/v1";
```

### cpu(v)

| Input | Output |
|-------|--------|
| `"500m"` | `"500m"` (pass-through) |
| `1` | `"1"` (integer → string) |
| `0.5` | `"500m"` (float → milliCPU) |

### memory(v)

| Input | Output |
|-------|--------|
| `"512Mi"` | `"512Mi"` (pass-through) |
| `2` | `"2Gi"` (number → GiB) |

### Building resource requirements

Use `requests()` to wrap resource lists, and chain `.limits()` to add limits:

```typescript
import { name, image, cpu, memory, requests } from "k8s/core/v1";

name("web")
  .image("nginx:1.25")
  .resources(
    requests(cpu(0.25).memory("128Mi"))
      .limits(cpu(1).memory("256Mi"))
  )
```

`requests(resourceList)` creates a `ResourceRequirementsChain`.

Call `.limits(resourceList)` on it to add limits, then pass the result to `.resources()`.

---

## Workload shortcuts

Workload resources like `Deployment`, `StatefulSet`, `DaemonSet`, `Job`, and `ReplicaSet` have shortcut methods that reach into `template.spec`:

```typescript
import { Deployment } from "k8s/apps/v1";
import { name, image } from "k8s/core/v1";

Deployment()
  .containers([
    name("app").image("myapp:latest")
  ])
  .initContainers([
    name("init").image("busybox:latest").command(["sh", "-c", "echo init"])
  ])
```

`.containers(v)` sets `spec.template.spec.containers`.

`.initContainers(v)` sets `spec.template.spec.initContainers`.

You don't need to build the full `template` → `spec` chain manually.

---

## Reusing builders

Because every method returns a new instance, you can assign builders to variables and reuse them freely:

```typescript
import { Deployment } from "k8s/apps/v1";
import { name, namespace, label } from "k8s/meta/v1";
import { name, image } from "k8s/core/v1";
import husako from "husako";

const webPod = Deployment()
  .containers([name("web").image("nginx:1.25")]);

const prod = webPod
  .metadata(name("web-prod").namespace("prod").label("env", "prod"))
  .replicas(5);

const staging = webPod
  .metadata(name("web-staging").namespace("staging").label("env", "staging"))
  .replicas(1);

husako.build([prod, staging]);
```

`prod` and `staging` share the same base builder.

Modifying one never affects the other.

You can also parameterize with functions:

```typescript
import { name, namespace, label } from "k8s/meta/v1";
import { name, image } from "k8s/core/v1";

function webDeployment(appName: string, ns: string, replicas: number) {
  return Deployment()
    .metadata(name(appName).namespace(ns).label("app", appName))
    .replicas(replicas)
    .containers([name("web").image("myapp:latest")]);
}

husako.build([
  webDeployment("web", "production", 5),
  webDeployment("web", "staging", 1),
]);
```

---

## The build() call

Every entry file must call `husako.build()` exactly once:

```typescript
import husako from "husako";

husako.build([resource1, resource2, resource3]);
```

`husako.build()` accepts a single builder or an array of builders.

Each item must be a resource builder instance (it must have a `_render()` method).

Passing plain objects throws a `TypeError`.

Rules:

- Missing `husako.build()` call → exit code 7
- Multiple `husako.build()` calls → exit code 7
- Items without `_render()` → `TypeError`

The output must also pass strict JSON validation.

Banned values in the rendered output: `undefined`, `bigint`, `symbol`, functions, class instances, `Date`, `Map`, `Set`, `RegExp`, and cyclic references.
