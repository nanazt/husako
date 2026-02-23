# Helm Chart Values

husako generates TypeScript types from Helm chart `values.schema.json` files.

This gives you autocomplete and type checking when composing Helm chart values in TypeScript.

::: info Putting values to use
husako generates the `Values` builder, but you need a resource builder that accepts a `values`
field to actually deploy the chart. The [Flux CD plugin's `HelmRelease`](./plugins/flux) is
currently the primary consumer. See the [Flux CD guide](./plugins/flux) for a complete
end-to-end example combining `helm/*` imports with `HelmRelease`.
:::

## Overview

When you add a chart dependency to `husako.toml` and run `husako generate`, husako:

1. Fetches the chart's `values.schema.json`
2. Converts the JSON Schema to TypeScript interfaces and builder classes
3. Writes them to `.husako/types/helm/<chart-name>.d.ts` and `.js`

You then import typed value builders from `"helm/<chart-name>"`.

---

## Adding a chart dependency

**Interactive:**

```
husako add --chart
```

This prompts for the chart source type, searches ArtifactHub if selected, and helps you pick
the name and version.

**Manual (`husako.toml`):**

```toml
[charts]
ingress-nginx = { source = "registry", repo = "https://kubernetes.github.io/ingress-nginx", chart = "ingress-nginx", version = "4.11.0" }
```

---

## Generating types

After adding a chart dependency, run:

```
husako generate
```

This fetches `values.schema.json` for each chart in `[charts]` and writes typed builders to
`.husako/types/helm/`.

---

## Using the generated types

Import the `Values` builder from `"helm/<chart-name>"` and use it to construct typed values:

```typescript
import { Values } from "helm/ingress-nginx";

const values = Values()
  .replicaCount(2)
  .controller({
    service: { type: "LoadBalancer" },
    resources: {
      requests: { cpu: "100m", memory: "90Mi" },
    },
  });
```

The `Values` type reflects the chart's `values.schema.json`. Your editor shows autocomplete
and catches typos. When you call `._toJSON()` on a `Values` builder, it resolves to a plain
object matching the chart's schema.

To use this with a `HelmRelease`, pass the builder to `.values()`:

```typescript
import { HelmRelease } from "flux";
import { HelmRepository } from "flux/source";
import { Values } from "helm/ingress-nginx";

const repo = HelmRepository("ingress-nginx-repo")
  .url("https://kubernetes.github.io/ingress-nginx");

const release = HelmRelease("ingress-nginx")
  .namespace("ingress")
  .chart("ingress-nginx", "4.11.0")
  .sourceRef(repo)
  .values(
    Values()
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

### artifacthub

Resolves the chart using the ArtifactHub API. Useful when you don't know the registry URL:

```toml
[charts]
cert-manager = { source = "artifacthub", repo = "cert-manager", chart = "cert-manager", version = "v1.16.2" }
```

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

## ArtifactHub search

When using `husako add --chart` interactively, selecting ArtifactHub opens an inline search.

Type to filter charts, use arrow keys to select, and press Enter to confirm. husako then
fetches available versions and shows a selection list.

The interactive flow:

1. Choose source type (ArtifactHub is the first/default option)
2. Search for the chart by name
3. Select from results
4. Choose a version (latest tagged first)
5. husako writes the entry to `husako.toml`

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
