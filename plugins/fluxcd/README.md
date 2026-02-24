# husako-plugin-fluxcd

FluxCD plugin for [husako](https://github.com/nanazt/husako). Provides type-safe builders for FluxCD resources — source controllers, Helm releases, and Kustomizations.

Compatible with FluxCD v2.x.

## Setup

Add the plugin to your `husako.toml`:

```toml
[plugins]
fluxcd = { source = "path", path = "plugins/fluxcd" }
```

Then run:

```bash
husako generate
```

This installs the plugin, fetches CRD schemas for the FluxCD controllers, and generates type definitions so your editor provides full autocomplete.

## Modules

| Import | Contents |
|--------|----------|
| `"fluxcd"` | `HelmRelease`, `Kustomization`, plus re-exports of all source types |
| `"fluxcd/source"` | `GitRepository`, `HelmRepository`, `OCIRepository` |

For most use cases, importing from `"fluxcd"` alone is enough.

## Usage

### HelmRelease with HelmRepository

```typescript
import { build, name, namespace } from "husako";
import { HelmRelease, HelmRepository } from "fluxcd";

const repo = HelmRepository()
  .metadata(name("bitnami").namespace("flux-system"))
  .url("https://charts.bitnami.com/bitnami")
  .interval("1h");

const redis = HelmRelease()
  .metadata(name("redis").namespace("default"))
  .chart("redis", "18.0.0")
  .sourceRef(repo)
  .interval("5m")
  .values({ architecture: "standalone" });

build([repo, redis]);
```

### Kustomization with GitRepository

```typescript
import { build, name, namespace } from "husako";
import { Kustomization, GitRepository } from "fluxcd";

const repo = GitRepository()
  .metadata(name("infra").namespace("flux-system"))
  .url("https://github.com/example/infra")
  .ref({ branch: "main" })
  .interval("5m");

const ks = Kustomization()
  .metadata(name("infra").namespace("flux-system"))
  .sourceRef(repo)
  .path("./clusters/production")
  .interval("10m")
  .prune(true)
  .targetNamespace("default");

build([repo, ks]);
```

### OCI Repository

```typescript
import { build, name, namespace } from "husako";
import { HelmRelease } from "fluxcd";
import { OCIRepository } from "fluxcd/source";

const oci = OCIRepository()
  .metadata(name("podinfo").namespace("flux-system"))
  .url("oci://ghcr.io/stefanprodan/manifests/podinfo")
  .ref({ tag: "6.3.0" })
  .interval("10m");

build([oci]);
```

## Typed Helm values

husako can generate typed values builders for the Helm charts your FluxCD releases deploy. Add the chart to `[charts]` in `husako.toml` and run `husako generate`:

```toml
[charts]
ingress-nginx = { source = "registry", repo = "https://kubernetes.github.io/ingress-nginx", chart = "ingress-nginx", version = "4.11.0" }
```

After generation, import the `Values` builder and pass it to `.values()`:

```typescript
import { build, metadata } from "husako";
import { HelmRelease, HelmRepository } from "fluxcd";
import { Values } from "helm/ingress-nginx";

const repo = HelmRepository("ingress-nginx-repo")
  .metadata(metadata().namespace("flux-system"))
  .url("https://kubernetes.github.io/ingress-nginx")
  .interval("1h");

const values = Values()
  .replicaCount(2)
  .controller({
    service: { type: "LoadBalancer" },
    resources: { requests: { cpu: "100m", memory: "90Mi" } },
  });

const release = HelmRelease("ingress-nginx")
  .metadata(metadata().namespace("ingress"))
  .sourceRef(repo)
  .chart("ingress-nginx", "4.11.0")
  .interval("1h")
  .values(values);

build([repo, release]);
```

See [Helm Chart Values](https://nanazt.github.io/husako/guide/helm) in the husako docs for details on chart type generation.

## Source Ref Linking

Source builders (`GitRepository`, `HelmRepository`, `OCIRepository`) can be passed directly to `.sourceRef()` on `HelmRelease` and `Kustomization`. The plugin resolves the source's `kind`, `name`, and `namespace` automatically:

```typescript
const repo = HelmRepository()
  .metadata(name("charts").namespace("flux-system"))
  .url("https://charts.example.com");

// Passing the builder directly — no need to construct { kind, name, namespace }
const release = HelmRelease()
  .sourceRef(repo)
  .chart("my-app", "1.0.0");
```

You can also pass a plain object if needed:

```typescript
const release = HelmRelease()
  .sourceRef({ kind: "HelmRepository", name: "charts", namespace: "flux-system" })
  .chart("my-app", "1.0.0");
```

## API Reference

### Source Controllers (`"fluxcd/source"`)

**GitRepository** — `source.toolkit.fluxcd.io/v1`

| Method | Parameter | Description |
|--------|-----------|-------------|
| `.url(url)` | `string` | Git repository URL |
| `.ref(ref)` | `{ branch?, tag?, semver?, commit? }` | Git reference |
| `.interval(interval)` | `string` | Reconciliation interval (e.g. `"5m"`) |
| `.secretRef(name)` | `string` | Secret name for authentication |

**HelmRepository** — `source.toolkit.fluxcd.io/v1`

| Method | Parameter | Description |
|--------|-----------|-------------|
| `.url(url)` | `string` | Helm repository URL |
| `.type(type)` | `"default" \| "oci"` | Repository type |
| `.interval(interval)` | `string` | Reconciliation interval |
| `.secretRef(name)` | `string` | Secret name for authentication |

**OCIRepository** — `source.toolkit.fluxcd.io/v1beta2`

| Method | Parameter | Description |
|--------|-----------|-------------|
| `.url(url)` | `string` | OCI artifact URL |
| `.ref(ref)` | `{ tag?, semver?, digest? }` | OCI reference |
| `.interval(interval)` | `string` | Reconciliation interval |
| `.secretRef(name)` | `string` | Secret name for authentication |

### Deploy Controllers (`"fluxcd"`)

**HelmRelease** — `helm.toolkit.fluxcd.io/v2`

| Method | Parameter | Description |
|--------|-----------|-------------|
| `.chart(name, version)` | `string, string \| number` | Chart name and version |
| `.sourceRef(ref)` | builder or `{ kind, name, namespace? }` | Source reference |
| `.interval(interval)` | `string` | Reconciliation interval |
| `.values(values)` | `Record<string, unknown>` | Helm values override |
| `.valuesFrom(sources)` | `Array<{ kind, name, valuesKey? }>` | External values sources |
| `.dependsOn(deps)` | `Array<{ name, namespace? }>` | Dependency ordering |

**Kustomization** — `kustomize.toolkit.fluxcd.io/v1`

| Method | Parameter | Description |
|--------|-----------|-------------|
| `.sourceRef(ref)` | builder or `{ kind, name, namespace? }` | Source reference |
| `.path(path)` | `string` | Path within the source |
| `.interval(interval)` | `string` | Reconciliation interval |
| `.prune(enable)` | `boolean` | Enable garbage collection |
| `.targetNamespace(ns)` | `string` | Target namespace for resources |
| `.dependsOn(deps)` | `Array<{ name, namespace? }>` | Dependency ordering |

All builders also inherit `.metadata()`, `.spec()`, `.set()` from `_ResourceBuilder`.

## Compatibility

| FluxCD | source-controller | helm-controller | kustomize-controller |
|--------|-------------------|-----------------|----------------------|
| v2.x   | v1.8.0            | v1.5.0          | v1.8.0               |

These CRDs are fetched during `husako generate` and used for schema validation.

## License

MIT
