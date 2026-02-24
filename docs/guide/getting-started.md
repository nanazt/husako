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

- **git** — used by `husako generate` and `husako add` for git-based schema and chart sources

To build from source:

- **Rust 1.85+**

No TypeScript compiler or JavaScript runtime installation required. husako bundles its own.

---

## Create a project

```
husako new my-app
cd my-app
```

This generates a starter `entry.ts`, a `husako.toml` config file, and `.gitignore`.

The `entry.ts` contains a minimal working example you can run immediately.

---

## Generate types

Types are generated from your Kubernetes cluster's OpenAPI spec (or a pre-fetched spec file).

They give you typed builder classes for every resource kind your cluster supports.

Connect to a running cluster:

```
husako generate --api-server https://localhost:6443
```

Use the short alias:

```
husako gen --api-server https://localhost:6443
```

Or use a locally downloaded spec directory:

```
husako generate --spec-dir ./openapi-specs
```

Skip Kubernetes type generation (only writes `husako.d.ts` and `tsconfig.json`):

```
husako generate --skip-k8s
```

This writes a `.husako/` directory with `.d.ts` type definitions and a `tsconfig.json`.

Your editor reads these automatically for autocomplete and type checking.

::: tip
`.husako/` is auto-managed by husako and should never be committed to version control. It is added to `.gitignore` by `husako new`.
:::

---

## Write resources

With types generated, import builders from `k8s/*` paths:

```typescript
import { Deployment } from "k8s/apps/v1";
import { Container } from "k8s/core/v1";
import { LabelSelector } from "k8s/_common";
import { metadata, cpu, memory, requests, limits, build } from "husako";

const nginx = Deployment()
  .metadata(metadata().name("nginx").namespace("default").label("app", "nginx"))
  .replicas(1)
  .selector(LabelSelector().matchLabels({ app: "nginx" }))
  .containers([
    Container()
      .name("nginx")
      .image("nginx:1.25")
      .resources(
        requests(cpu("250m").memory("128Mi"))
          .limits(cpu("500m").memory("256Mi"))
      )
  ]);

build([nginx]);
```

The `build()` call at the end is required.

It collects all resources and signals husako to emit YAML.

See [Writing Resources](/guide/writing-resources) for the full builder API.

---

## Render

```
husako render entry.ts
```

This compiles the TypeScript, runs it, and prints multi-document YAML to stdout.

Pipe straight to kubectl:

```
husako render entry.ts | kubectl apply -f -
```

Use an entry alias from `husako.toml` instead of a file path:

```
husako render dev
```

See [Configuration](/guide/configuration) for entry aliases and project setup.
