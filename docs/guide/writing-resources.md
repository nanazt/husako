# Writing Resources

husako resources are written using a builder DSL. No YAML, no plain objects for resource structure — just chained method calls that compile to typed Kubernetes manifests.

## Factory functions

Every resource type is a PascalCase factory function. Call it with no arguments to get an empty builder:

```typescript
import { Deployment } from "k8s/apps/v1";
import { Service } from "k8s/core/v1";

const deploy = Deployment();
const svc = Service();
```

No `new` keyword, no `new Deployment()`. The factory is the API.

## Chaining methods

Every method on a builder returns a new builder instance. You build up a resource by chaining:

```typescript
const deploy = Deployment()
  .replicas(3)
  .selector(LabelSelector().matchLabels({ app: "web" }));
```

Each property has a corresponding method generated from the OpenAPI spec. Method names match the spec field names in camelCase.

## metadata()

`metadata()` is the entry point for metadata chains. It returns a `MetadataFragment` with its own chainable methods:

```typescript
import { metadata } from "husako";

const meta = metadata()
  .name("my-app")
  .namespace("production")
  .label("app", "my-app")
  .label("version", "1.2.3")
  .annotation("team", "platform");
```

Pass the fragment to any resource's `.metadata()` method:

```typescript
const deploy = Deployment().metadata(meta);
```

The shorthand functions `name()`, `namespace()`, `label()`, and `annotation()` from `"husako"` are aliases that create a `metadata()` chain and call the corresponding method. Use whichever style you prefer.

## Resource quantities

CPU and memory have dedicated fragment builders with automatic normalization:

```typescript
import { cpu, memory, requests, limits } from "husako";
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

Chain `requests()` and `limits()` together:

```typescript
import { Container } from "k8s/core/v1";
import { cpu, memory, requests, limits } from "husako";

Container()
  .name("web")
  .image("nginx:1.25")
  .resources(
    requests(cpu(0.25).memory("128Mi"))
      .limits(cpu(1).memory("256Mi"))
  )
```

`requests(resourceList)` creates a `ResourceRequirementsFragment`. Call `.limits(resourceList)` on it to add limits. Pass the whole thing to `.resources()`.

## Workload shortcuts

Workload resources like `Deployment`, `StatefulSet`, `DaemonSet`, `Job`, and `ReplicaSet` have shortcut methods that reach into `template.spec`:

```typescript
import { Deployment } from "k8s/apps/v1";
import { Container } from "k8s/core/v1";

Deployment()
  .containers([
    Container().name("app").image("myapp:latest")
  ])
  .initContainers([
    Container().name("init").image("busybox:latest").command(["sh", "-c", "echo init"])
  ])
```

`.containers(v)` sets `spec.template.spec.containers`. `.initContainers(v)` sets `spec.template.spec.initContainers`. You don't need to build the full `template` → `spec` chain manually.

## Reusing builders

Because every method returns a new instance, you can assign builders to variables and reuse them freely:

```typescript
const webPod = PodTemplateSpec()
  .metadata(metadata().label("tier", "web"))
  .containers([
    Container().name("web").image("nginx:1.25")
  ]);

const prod = Deployment()
  .metadata(metadata().name("web-prod"))
  .replicas(5)
  .template(webPod);

const staging = Deployment()
  .metadata(metadata().name("web-staging"))
  .replicas(1)
  .template(webPod);

build([prod, staging]);
```

`prod` and `staging` share the same pod template object. Modifying one never affects the other.

You can also parameterize with functions:

```typescript
function webDeployment(name: string, namespace: string, replicas: number) {
  return Deployment()
    .metadata(metadata().name(name).namespace(namespace).label("app", name))
    .replicas(replicas)
    .containers([
      Container().name("web").image("myapp:latest")
    ]);
}

build([
  webDeployment("web", "production", 5),
  webDeployment("web", "staging", 1),
]);
```

## The build() call

Every entry file must call `build()` exactly once:

```typescript
import { build } from "husako";

build([resource1, resource2, resource3]);
```

`build()` accepts a single builder or an array of builders. Each item must be a resource builder instance (it must have a `_render()` method). Passing plain objects throws a `TypeError`.

**Rules:**
- Missing `build()` call → exit code 7
- Multiple `build()` calls → exit code 7
- Items without `_render()` → `TypeError`

The output must also pass strict JSON validation. Banned values in the rendered output: `undefined`, `bigint`, `symbol`, functions, class instances, `Date`, `Map`, `Set`, `RegExp`, and cyclic references.
