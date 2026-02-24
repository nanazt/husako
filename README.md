# husako

husako is a CLI tool for writing Kubernetes resources in TypeScript. Define your deployments, services, and configurations using a typed builder API — then compile to YAML with a single command.

You get type safety, autocomplete, and the full expressiveness of a real programming language — functions, variables, loops, imports — instead of templating hacks on top of YAML.

Inspired by [gaji](https://github.com/dodok8/gaji).

## Quick example

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

```
$ husako render entry.ts
```

Every builder exports a PascalCase factory function — `Deployment()`, `Service()`, `Container()`. Properties are chainable methods with full type safety and autocomplete. See [Getting started](#getting-started) below.

## Install

**npm** (recommended):

```
npm install -g husako
```

**Cargo** (from source):

```
cargo install husako
```

Or download prebuilt binaries from [GitHub Releases](https://github.com/nanazt/husako/releases).

## Requirements

- **Rust 1.85+** — to build from source
- **git** — used by `husako generate` and `husako add` for git-based schema and chart sources

No TypeScript compiler or JavaScript runtime installation required. husako bundles its own.

## Getting started

### 1. Create a project

```
husako new my-app
cd my-app
```

This generates a starter `entry.ts` and `.gitignore`.

### 2. Generate types

Connect to a running cluster to generate typed builders for every resource kind:

```
husako generate --api-server https://localhost:6443
```

Or use the short alias:

```
husako gen --api-server https://localhost:6443
```

Skip Kubernetes type generation:

```
husako generate --skip-k8s
```

This writes a `.husako/` directory with `.d.ts` type definitions and `tsconfig.json`. Your editor picks these up for autocomplete.

### 3. Write resources

With types initialized, you can use the typed builder API:

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

Every builder exports a PascalCase factory function — `Deployment()`, `Service()`, `Container()`, etc. Properties are available as chainable methods. Workload resources get `.containers()` and `.initContainers()` shortcuts that reach into `template.spec`.

`metadata()` is the entry point for metadata chains. Resource quantities have their own fragment builders — `cpu()`, `memory()`, `requests()`, `limits()` — that chain together and normalize values (e.g. `cpu(0.5)` becomes `"500m"`, `memory(2)` becomes `"2Gi"`).

### 4. Render

```
husako render entry.ts
```

Pipe straight to kubectl:

```
husako render entry.ts | kubectl apply -f -
```

## Commands

| Command | Description |
| --- | --- |
| `husako new <dir>` | Create a new project from a template |
| `husako generate` (alias: `gen`) | Generate type definitions and `tsconfig.json` |
| `husako render <file>` | Compile TypeScript and emit YAML |

### `husako new`

| Flag | Default | Description |
| --- | --- | --- |
| `-t, --template` | `simple` | Template name: `simple`, `project`, or `multi-env` |

### `husako generate` (alias: `gen`)

| Flag | Description |
| --- | --- |
| `--api-server <url>` | Kubernetes API server URL |
| `--spec-dir <path>` | Local directory with pre-fetched OpenAPI spec files |
| `--skip-k8s` | Only write `husako.d.ts` and `tsconfig.json`, skip Kubernetes types |

### `husako render`

| Flag | Description |
| --- | --- |
| `--allow-outside-root` | Allow imports outside the project root |
| `--timeout-ms <ms>` | Execution timeout in milliseconds |
| `--max-heap-mb <mb>` | Maximum heap memory in megabytes |
| `--verbose` | Print diagnostic traces to stderr |

## Templates

`husako new` supports three project templates.

### `simple` (default)

A single entry file. Good for trying things out.

```
my-app/
├── .gitignore
└── entry.ts
```

### `project`

Separate directories for deployments, shared libraries, and environment configs. The entry point is `env/dev.ts`, which imports resources from `deployments/` and shared helpers from `lib/`.

```
my-app/
├── .gitignore
├── deployments/
│   └── nginx.ts
├── env/
│   └── dev.ts
└── lib/
    ├── index.ts
    └── metadata.ts
```

### `multi-env`

Shared base resources with per-environment entry points. Base modules export functions that accept environment-specific parameters (namespace, replica count, image tag), and each environment directory has a `main.ts` that calls them with the right values.

```
my-app/
├── .gitignore
├── base/
│   ├── nginx.ts
│   └── service.ts
├── dev/
│   └── main.ts
├── staging/
│   └── main.ts
└── release/
    └── main.ts
```

Render a specific environment:

```
husako render my-app/dev/main.ts
husako render my-app/staging/main.ts
```

## How it works

husako runs entirely offline after `husako generate`. The TypeScript source is stripped of types by [oxc](https://oxc.rs), then executed in an embedded [QuickJS](https://bellard.org/quickjs/) runtime. The runtime captures the array passed to `husako.build()`, validates it against a strict JSON contract (no `undefined`, no functions, no cycles), and emits multi-document YAML. There is no Node.js dependency and no network access at render time.

## Project structure

For contributors — the workspace is split into focused crates:

```
crates/
├── husako-cli/          # CLI entry point (clap)
├── husako-core/         # Pipeline orchestration and validation
├── husako-compile-oxc/  # TypeScript → JavaScript via oxc
├── husako-runtime-qjs/  # QuickJS execution and module loading
├── husako-openapi/      # OpenAPI v3 fetch and disk cache
├── husako-dts/          # OpenAPI → .d.ts type generation
├── husako-yaml/         # JSON → YAML emitter
└── husako-sdk/          # Built-in JS runtime sources and base type definitions
```

## License

MIT
