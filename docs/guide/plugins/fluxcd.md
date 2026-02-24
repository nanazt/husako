# FluxCD Plugin

The FluxCD plugin provides builders for GitOps resources from the Flux toolkit. It is
bundled in the husako repository at `plugins/fluxcd/`.

**Provided modules:**

| Import | Resources |
|--------|-----------|
| `"fluxcd"` | `HelmRelease`, `Kustomization` |
| `"fluxcd/source"` | `GitRepository`, `HelmRepository`, `OCIRepository` |

---

## Setup

Add the plugin to `husako.toml`:

```toml
[plugins]
fluxcd = { source = "git", url = "https://github.com/nanazt/husako", path = "plugins/fluxcd" }
```

Then run `husako generate`. This installs the plugin, generates FluxCD CRD types, and updates
`tsconfig.json` with path mappings for `"fluxcd"` and `"fluxcd/source"`.

---

## Source types (`fluxcd/source`)

FluxCD source resources represent where Flux should pull manifests or charts from. Each source
type exposes a `_sourceRef()` method used for cross-resource linking.

### GitRepository

```typescript
import { GitRepository } from "fluxcd/source";
import { metadata, build } from "husako";

const gitRepo = GitRepository("my-repo")
  .metadata(metadata().namespace("flux-system"))
  .url("https://github.com/example/my-k8s-config")
  .ref({ branch: "main" })
  .interval("1m");

build([gitRepo]);
```

### HelmRepository

```typescript
import { HelmRepository } from "fluxcd/source";
import { metadata, build } from "husako";

const helmRepo = HelmRepository("ingress-nginx")
  .metadata(metadata().namespace("flux-system"))
  .url("https://kubernetes.github.io/ingress-nginx")
  .interval("1h");

build([helmRepo]);
```

### OCIRepository

```typescript
import { OCIRepository } from "fluxcd/source";
import { metadata, build } from "husako";

const ociRepo = OCIRepository("my-oci")
  .metadata(metadata().namespace("flux-system"))
  .url("oci://ghcr.io/example/charts/my-chart")
  .ref({ tag: "v1.0.0" })
  .interval("10m");

build([ociRepo]);
```

---

## Kustomization

`Kustomization` deploys manifests from a `GitRepository` or `OCIRepository`:

```typescript
import { Kustomization } from "fluxcd";
import { GitRepository } from "fluxcd/source";
import { metadata, build } from "husako";

const gitRepo = GitRepository("my-config")
  .metadata(metadata().namespace("flux-system"))
  .url("https://github.com/example/my-k8s-config")
  .ref({ branch: "main" })
  .interval("1m");

const ks = Kustomization("my-app")
  .metadata(metadata().namespace("flux-system"))
  .sourceRef(gitRepo)
  .path("./clusters/production")
  .interval("5m")
  .prune(true);

build([gitRepo, ks]);
```

---

## HelmRelease

`HelmRelease` deploys a Helm chart from a `HelmRepository` or `OCIRepository`:

```typescript
import { HelmRelease } from "fluxcd";
import { HelmRepository } from "fluxcd/source";
import { metadata, build } from "husako";

const helmRepo = HelmRepository("ingress-nginx")
  .metadata(metadata().namespace("flux-system"))
  .url("https://kubernetes.github.io/ingress-nginx");

const release = HelmRelease("ingress-nginx")
  .metadata(metadata().namespace("ingress"))
  .sourceRef(helmRepo)
  .chart("ingress-nginx", "4.11.0")
  .interval("1h");

build([helmRepo, release]);
```

See [Helm Chart Values](../helm) to generate typed `values` builders for the chart.

---

## Linking with `_sourceRef()`

All FluxCD source resources expose a `_sourceRef()` method that returns `{ kind, name, namespace }`.

When you pass a source builder to `.sourceRef()`, husako calls `_sourceRef()` automatically:

```typescript
const repo = HelmRepository("my-repo")
  .metadata(metadata().namespace("flux-system"))
  .url("https://example.com/charts");

const release = HelmRelease("my-app")
  .sourceRef(repo);   // { kind: "HelmRepository", name: "my-repo", namespace: "flux-system" }
```

This duck-typed convention (`typeof ref._sourceRef === "function"`) links resources without
hard-coded type checks. It works the same way for `Kustomization` linking to a `GitRepository`.

---

## Full example — HelmRepository + HelmRelease + typed values

```typescript
import { HelmRelease } from "fluxcd";
import { HelmRepository } from "fluxcd/source";
import { Values } from "helm/ingress-nginx";
import { metadata, build } from "husako";

const helmRepo = HelmRepository("ingress-nginx-repo")
  .metadata(metadata().namespace("flux-system"))
  .url("https://kubernetes.github.io/ingress-nginx")
  .interval("1h");

const values = Values()
  .replicaCount(2)
  .controller({
    service: { type: "LoadBalancer" },
    resources: {
      requests: { cpu: "100m", memory: "90Mi" },
    },
  });

const release = HelmRelease("ingress-nginx")
  .metadata(metadata().namespace("ingress"))
  .sourceRef(helmRepo)
  .chart("ingress-nginx", "4.11.0")
  .interval("1h")
  .values(values);

build([helmRepo, release]);
```

For typed values, add `ingress-nginx` to `[charts]` in `husako.toml` and run
`husako generate`. See [Helm Chart Values](../helm) for details.
