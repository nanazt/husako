# Templates

`husako new` supports three project templates. Choose one with the `-t` flag:

```
husako new my-app -t project
```

The default template is `simple`.

## simple

A single entry file. Good for trying things out or writing one-off scripts.

```
my-app/
├── .gitignore
├── husako.toml
└── entry.husako
```

`entry.husako` contains a minimal working example with a single Deployment.

## project

Separate directories for deployments, shared libraries, and environment configs. The entry point is `env/dev.husako`, which imports resources from `deployments/` and shared helpers from `lib/`.

```
my-app/
├── .gitignore
├── husako.toml
├── deployments/
│   └── nginx.ts
├── env/
│   └── dev.ts
└── lib/
    ├── index.ts
    └── metadata.ts
```

`lib/metadata.ts` exports a shared metadata factory. `deployments/nginx.ts` imports from `lib/` and exports a factory function. `env/dev.husako` calls the factories and passes the result to `build()`.

Good for single-environment setups where you want to keep resource definitions separate from the entry point.

## multi-env

Shared base resources with per-environment entry points. Base modules export functions that accept environment-specific parameters (namespace, replica count, image tag), and each environment directory has a `main.ts` that calls them with the right values.

```
my-app/
├── .gitignore
├── husako.toml
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

`base/nginx.ts` exports a function like:

```typescript
import { Deployment } from "k8s/apps/v1";
import { name, namespace } from "k8s/meta/v1";
import { name, image } from "k8s/core/v1";

export function nginxDeployment(ns: string, replicas: number, tag: string) {
  return Deployment()
    .metadata(name("nginx").namespace(ns))
    .replicas(replicas)
    .containers([
      name("nginx").image(`nginx:${tag}`)
    ]);
}
```

Each environment's `main.ts` calls these with environment-specific values:

```typescript
// dev/main.husako
import { nginxDeployment } from "../base/nginx.ts";
import husako from "husako";

husako.build([nginxDeployment("dev", 1, "latest")]);
```

Render a specific environment:

```
husako render my-app/dev/main.husako
husako render my-app/staging/main.husako
```

Or add entry aliases to `husako.toml` so you can use short names:

```toml
[entries]
dev = "dev/main.husako"
staging = "staging/main.husako"
release = "release/main.husako"
```

Then:

```
husako render dev
husako render staging
```
