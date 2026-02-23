# husako-plugin-flux

Flux CD plugin for [husako](https://github.com/nanazt/husako). Provides type-safe builders for Flux CD resources — source controllers, Helm releases, and Kustomizations.

## Setup

Add the plugin to your `husako.toml`:

```toml
[plugins]
flux = { source = "path", path = "plugins/flux" }
```

Then run:

```bash
husako generate
```

This installs the plugin, fetches CRD schemas for the Flux controllers, and generates type definitions so your editor provides full autocomplete.

## Modules

| Import | Contents |
|--------|----------|
| `"flux"` | `HelmRelease`, `Kustomization`, plus re-exports of all source types |
| `"flux/source"` | `GitRepository`, `HelmRepository`, `OCIRepository` |

For most use cases, importing from `"flux"` alone is enough.

## Usage

### HelmRelease with HelmRepository

```typescript
import { build, name, namespace } from "husako";
import { HelmRelease, HelmRepository } from "flux";

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
import { Kustomization, GitRepository } from "flux";

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
import { HelmRelease } from "flux";
import { OCIRepository } from "flux/source";

const oci = OCIRepository()
  .metadata(name("podinfo").namespace("flux-system"))
  .url("oci://ghcr.io/stefanprodan/manifests/podinfo")
  .ref({ tag: "6.3.0" })
  .interval("10m");

build([oci]);
```

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

### Source Controllers (`"flux/source"`)

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

### Deploy Controllers (`"flux"`)

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

## Bundled CRD Versions

| Controller | Version |
|------------|---------|
| source-controller | v1.8.0 |
| helm-controller | v1.5.0 |
| kustomize-controller | v1.8.0 |

These are fetched during `husako generate` and used for schema validation.

## License

MIT
