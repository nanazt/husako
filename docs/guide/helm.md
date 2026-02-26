# Helm Chart Values

husako gens TypeScript types from Helm chart `values.schema.json` files.

This gives you autocomplete and type checking when composing Helm chart values in TypeScript.

::: info Putting values to use
husako gens the `Values` builder, but you need a resource builder that accepts a `values`
field to actually deploy the chart. The [Flux CD plugin's `HelmRelease`](./plugins/flux) is
currently the primary consumer. See the [Flux CD guide](./plugins/flux) for a complete
end-to-end example combining `helm/*` imports with `HelmRelease`.
:::

## Overview

When you add a chart dependency to `husako.toml` and run `husako gen`, husako:

1. Fetches the chart's `values.schema.json`
2. Converts the JSON Schema to TypeScript interfaces and builder classes
3. Writes them to `.husako/types/helm/<chart-name>.d.ts` and `.js`

You then import typed value builders from `"helm/<chart-name>"`.

---

## Adding a chart dependency

Pass the chart URL or ArtifactHub identifier to `husako add`:

```
# ArtifactHub (org/chart)
husako add bitnami/postgresql

# Helm registry — chart name required as second argument or --name
husako add https://kubernetes.github.io/ingress-nginx ingress-nginx

# OCI registry
husako add oci://ghcr.io/bitnami/postgresql

# Git repo containing a chart
husako add https://github.com/example/charts --path charts/my-chart
```

husako detects the source type from the URL, resolves the latest version, and writes the entry to `husako.toml`. Use `--version` to pin to a specific release or partial prefix (`--version 16` matches the latest `16.x.x`).

**Manual (`husako.toml`):**

```toml
[charts]
ingress-nginx = { source = "registry", repo = "https://kubernetes.github.io/ingress-nginx", chart = "ingress-nginx", version = "4.11.0" }
```

---

## Generating types

After adding a chart dependency, run:

```
husako gen
```

This fetches `values.schema.json` for each chart in `[charts]` and writes typed builders to
`.husako/types/helm/`.

---

## Using the generated types

Import the chart builder from `"helm/<chart-name>"` and use it to construct typed values. The exported name is the config key converted to PascalCase — `ingress-nginx` becomes `IngressNginx`:

```typescript
import { IngressNginx } from "helm/ingress-nginx";

const values = IngressNginx()
  .replicaCount(2)
  .controller({
    service: { type: "LoadBalancer" },
    resources: {
      requests: { cpu: "100m", memory: "90Mi" },
    },
  });
```

The builder type reflects the chart's `values.schema.json`. Your editor shows autocomplete
and catches typos. When you call `._toJSON()` on the builder, it resolves to a plain
object matching the chart's schema.

To use this with a `HelmRelease`, pass the builder to `.values()`:

```typescript
import { HelmRelease } from "fluxcd";
import { HelmRepository } from "fluxcd/source";
import { IngressNginx } from "helm/ingress-nginx";

const repo = HelmRepository("ingress-nginx-repo")
  .url("https://kubernetes.github.io/ingress-nginx");

const release = HelmRelease("ingress-nginx")
  .namespace("ingress")
  .chart("ingress-nginx", "4.11.0")
  .sourceRef(repo)
  .values(
    IngressNginx()
      .replicaCount(2)
      .controller({ service: { type: "LoadBalancer" } })
  );

build([repo, release]);
```

---

## Source types

### registry

Fetches from a Helm HTTP repository. Downloads `index.yaml`, finds the chart version, and
extracts `values.schema.json` from the chart archive:

```toml
[charts]
ingress-nginx = { source = "registry", repo = "https://kubernetes.github.io/ingress-nginx", chart = "ingress-nginx", version = "4.11.0" }
```

OCI registries are also supported. Use an `oci://` URL as the `repo` value:

```toml
[charts]
postgresql = { source = "registry", repo = "oci://registry-1.docker.io/bitnamicharts/postgresql", chart = "postgresql", version = "16.4.0" }
```

husako fetches the chart tarball from the OCI registry using the OCI Distribution API and
extracts `values.schema.json` from it. Anonymous access works for public registries such as
Docker Hub and GHCR. Private registries requiring credentials are not yet supported.

### artifacthub

Resolves the chart using the ArtifactHub API. Useful when you don't know the registry URL:

```toml
[charts]
cert-manager = { source = "artifacthub", repo = "cert-manager", chart = "cert-manager", version = "v1.16.2" }
```

husako first checks the `values_schema` field in the ArtifactHub API response.
If it is absent, husako automatically retries using the chart's registry URL from the
ArtifactHub entry — both HTTP and OCI registries are supported.
Charts on private OCI registries that require credentials cannot be fetched automatically —
download `values.schema.json` manually and use `source = "file"` instead.

### git

Shallow-clones a git repository and reads `values.schema.json` from a subdirectory:

```toml
[charts]
my-chart = { source = "git", repo = "https://github.com/example/charts", path = "charts/my-chart", version = "main" }
```

The `version` field is used as the git branch or tag.

### file

Reads `values.schema.json` directly from a local path:

```toml
[charts]
local-chart = { source = "file", path = "./charts/my-chart/values.schema.json" }
```

Useful for charts in the same repository.

---

## ArtifactHub

Pass an `org/chart` identifier directly — husako queries ArtifactHub for the latest version and writes the entry:

```
husako add bitnami/postgresql
husako add cert-manager/cert-manager --version v1.16
```

The `org/chart` format must use only lowercase letters, digits, hyphens, underscores, and dots, with exactly one `/`.

---

## Checking for updates

```
husako outdated
```

For registry and ArtifactHub sources, husako queries upstream for newer versions and reports
what's available. For git sources, it checks for newer tags.

```
husako update
```

Updates all versioned chart dependencies to their latest versions and regenerates types.
