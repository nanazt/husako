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

Three source types are supported:

### release

Downloads the official Kubernetes OpenAPI spec for the given version from GitHub:

```toml
[resources]
core = { source = "release", version = "1.32.0" }
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

## husako.lock

`husako gen` creates a `husako.lock` file at the project root each time it runs. It records which type definitions were generated and from which source versions, so subsequent runs can skip regenerating types for unchanged dependencies.

**Commit `husako.lock` to version control.** This ensures all team members and CI environments generate types from the same resolved versions, the same way `Cargo.lock` pins exact crate versions across machines.

### What it tracks

For each entry in `[resources]`, `[charts]`, and `[plugins]`, the lock records the identity fields that determine the generated output — the version for a `release` source, the repo URL and tag for a `git` source, and so on. On subsequent runs, husako compares these against the current `husako.toml`; if they match and the type files exist on disk, the entry is skipped.

| Entry type | Skip condition |
|---|---|
| `release` resource | version unchanged AND `.husako/types/k8s/` exists |
| `git` resource | repo, tag, path unchanged AND `.husako/types/k8s/` exists |
| `file` resource | path unchanged AND file content unchanged AND `.husako/types/k8s/` exists |
| `registry` chart | repo, chart, version unchanged AND `.husako/types/helm/{name}.d.ts` exists |
| `artifacthub` chart | package, version unchanged AND type file exists |
| `git` chart | repo, tag, path unchanged AND type file exists |
| `oci` chart | reference, version unchanged AND type file exists |
| `file` chart | path unchanged AND file content unchanged AND type file exists |
| `git` plugin | URL (and path) unchanged AND `plugin.toml` version unchanged AND `.husako/plugins/{name}/` exists |
| `path` plugin | path unchanged AND directory content unchanged AND `plugin.toml` version unchanged |

> **Note on k8s resources**: All resources (`[resources]`) are regenerated as a single unit. If any resource changes, all k8s types are regenerated. Individual resources cannot be regenerated in isolation.

### Bypassing the lock

To regenerate all types regardless of the lock:

```
husako gen --no-incremental
```

This is useful when:
- A git plugin's remote HEAD changed (there is no version pin for git plugins without a tag)
- A git tag was moved upstream
- You suspect the lock is stale after manual edits

The lock is still written after `--no-incremental` so the next run will be incremental again.
