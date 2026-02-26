# Configuration

`husako.toml` is the project configuration file. It is created by `husako new` in the project root.

## Overview

```toml
[entries]
dev = "env/dev.ts"
staging = "env/staging.ts"

[resources]
core = { source = "release", version = "1.32.0" }
cert-manager = { source = "git", repo = "https://github.com/cert-manager/cert-manager", path = "deploy/crds" }

[charts]
ingress-nginx = { source = "registry", repo = "https://kubernetes.github.io/ingress-nginx", chart = "ingress-nginx", version = "4.11.0" }

[plugins]
flux = { source = "path", path = "plugins/flux" }
```

---

## Entry aliases

```toml
[entries]
dev = "env/dev.ts"
staging = "env/staging.ts"
```

Entry aliases map short names to file paths.

When you run `husako render dev`, husako resolves `dev` to `env/dev.ts` before rendering.

**Resolution order:** direct file path → entry alias → error with available aliases listed.

If you pass a name that is neither a real file nor a known alias, husako exits with code 2 and shows the available aliases.

---

## Resource dependencies

```toml
[resources]
core = { source = "release", version = "1.32.0" }
```

Resource dependencies declare Kubernetes schema sources for type generation.

`husako gen` reads these and produces typed builders under `.husako/types/k8s/`.

Four source types are supported:

### release

Downloads the official Kubernetes OpenAPI spec for the given version from GitHub:

```toml
[resources]
core = { source = "release", version = "1.32.0" }
```

### cluster

Fetches the OpenAPI spec from a live cluster.

Uses the kubeconfig bearer token for authentication:

```toml
[resources]
my-cluster = { source = "cluster", url = "https://localhost:6443" }
```

### git

Clones a git repository and reads CRD YAML files from a subdirectory:

```toml
[resources]
cert-manager = { source = "git", repo = "https://github.com/cert-manager/cert-manager", path = "deploy/crds" }
flux-source = { source = "git", repo = "https://github.com/fluxcd/source-controller", path = "config/crd/bases" }
```

### file

Reads CRD YAML files from a local directory:

```toml
[resources]
my-crds = { source = "file", path = "./crds" }
```

::: tip
`[resources]` is the current name for this section. The legacy name `[schemas]` is also accepted.
:::

---

## Chart dependencies

```toml
[charts]
ingress-nginx = { source = "registry", repo = "https://kubernetes.github.io/ingress-nginx", chart = "ingress-nginx", version = "4.11.0" }
```

Chart dependencies declare Helm chart sources for `values.schema.json` type generation.

`husako gen` fetches the schema and produces typed value builders under `.husako/types/helm/`.

Four source types are supported:

### registry

Fetches the chart from a Helm registry by downloading and inspecting the chart archive:

```toml
[charts]
ingress-nginx = { source = "registry", repo = "https://kubernetes.github.io/ingress-nginx", chart = "ingress-nginx", version = "4.11.0" }
```

### artifacthub

Resolves the chart via the ArtifactHub API:

```toml
[charts]
cert-manager = { source = "artifacthub", repo = "cert-manager", chart = "cert-manager", version = "v1.16.2" }
```

### git

Clones a git repository and reads the chart's `values.schema.json` from a subdirectory:

```toml
[charts]
my-chart = { source = "git", repo = "https://github.com/example/charts", path = "charts/my-chart", version = "main" }
```

### file

Reads `values.schema.json` from a local path:

```toml
[charts]
local-chart = { source = "file", path = "./charts/my-chart/values.schema.json" }
```

---

## Plugins

```toml
[plugins]
flux = { source = "path", path = "plugins/flux" }
my-local = { source = "path", path = "./plugins/my-plugin" }
```

Plugins extend husako with dependency presets and importable helper modules.

Two source types:

| Source | Fields | Description |
|--------|--------|-------------|
| `git` | `url` | Clone from a git repository (HEAD of default branch) |
| `path` | `path` | Use a local directory (relative path) |

Plugins are installed to `.husako/plugins/<name>/` during `husako gen`.

See [Plugins](/advanced/plugins) for authoring details.

---

## Cluster config

For commands that connect to a cluster directly, specify credentials in `husako.toml`.

Single cluster:

```toml
[cluster]
url = "https://localhost:6443"
token = "my-bearer-token"
```

Multiple named clusters:

```toml
[clusters.local]
url = "https://localhost:6443"

[clusters.production]
url = "https://k8s.example.com"
token = "prod-bearer-token"
```

If no token is provided, husako reads the bearer token from the active kubeconfig context (`~/.kube/config`).
