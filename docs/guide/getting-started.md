# Getting Started

## Installation

**npm** (recommended):

```
npm install -g husako
```

**Cargo** (from source):

```
cargo install husako
```

Or download prebuilt binaries from [GitHub Releases](https://github.com/nanazt/husako/releases).

---

## Requirements

- **git** — used by `husako gen` and `husako add` for git-based schema and chart sources

To build from source:

- **Rust 1.85+**

No TypeScript compiler or JavaScript runtime installation required. husako bundles its own.

---

## Create a project

```
husako new my-app
cd my-app
```

This generates a starter `entry.husako`, a `husako.toml` config file, and `.gitignore`.

The `entry.husako` contains a minimal working example you can run immediately.

---

## Generate types

`husako gen` writes `.d.ts` type definitions to `.husako/` and regenerates `tsconfig.json` at the project root. Your editor uses these for autocomplete and type checking. `tsconfig.json` is husako-managed — do not edit it manually, and add it to `.gitignore` (`husako new` does this automatically).

**To get started immediately** (no cluster required):

```
husako gen --skip-k8s
```

This writes the core `husako.d.ts` — enough to start writing resources and running `husako render`.

**To add typed builders for Kubernetes resource kinds** (Deployment, Service, etc.):

```
husako add --release
husako gen
```

`husako add --release` prompts you to pick a Kubernetes version and adds it to `husako.toml`. Then `husako gen` fetches the schema and generates typed builders for every resource kind.

You can also provide a pre-fetched spec directory:

```
husako gen --spec-dir ./openapi-specs
```

::: tip
After adding or removing a dependency with `husako add` or `husako remove`, types are regenerated automatically. You rarely need to run `husako gen` directly.
:::

::: tip
After the first `husako gen`, a `husako.lock` file is created at the project root. Commit it to version control. It enables incremental generation — subsequent runs skip types that have not changed.
:::

::: tip
`.husako/` is auto-managed by husako and should never be committed to version control. It is added to `.gitignore` by `husako new`.
:::

---

## Write resources

With types generated, import builders from `k8s/*` paths:

```typescript
import { Deployment } from "k8s/apps/v1";
import { LabelSelector } from "k8s/_common";
import { name, namespace, label } from "k8s/meta/v1";
import { name, image, cpu, memory, requests } from "k8s/core/v1";
import husako from "husako";

const nginx = Deployment()
  .metadata(name("nginx").namespace("default").label("app", "nginx"))
  .replicas(1)
  .selector(LabelSelector().matchLabels({ app: "nginx" }))
  .containers([
    name("nginx")
      .image("nginx:1.25")
      .resources(
        requests(cpu("250m").memory("128Mi"))
          .limits(cpu("500m").memory("256Mi"))
      )
  ]);

husako.build([nginx]);
```

The `husako.build()` call at the end is required.

It collects all resources and signals husako to emit YAML.

See [Writing Resources](/guide/writing-resources) for the full builder API.

---

## Render

```
husako render entry.husako
```

This compiles the TypeScript, runs it, and prints multi-document YAML to stdout.

Pipe straight to kubectl:

```
husako render entry.husako | kubectl apply -f -
```

Use an entry alias from `husako.toml` instead of a file path:

```
husako render dev
```

See [Configuration](/guide/configuration) for entry aliases and project setup.
