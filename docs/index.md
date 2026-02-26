---
layout: home

hero:
  name: husako
  text: Kubernetes resources in TypeScript
  tagline: Type safety, autocomplete, and the full TypeScript language — instead of templating hacks on top of YAML.
  actions:
    - theme: brand
      text: Get Started
      link: /guide/getting-started
    - theme: alt
      text: View on GitHub
      link: https://github.com/nanazt/husako

features:
  - icon: 🔒
    title: Type Safe
    details: Every resource kind generates a typed builder class. Your editor catches mistakes before kubectl does.
  - icon: ⚡
    title: Self-Contained
    details: husako bundles its own TypeScript compiler and JavaScript runtime. No separate installation needed — download and run.
  - icon: 📝
    title: Resources as Code
    details: Use functions, variables, loops, and imports to compose resources. Share common metadata, parameterize environments, reuse pod templates across deployments.
  - icon: 🧩
    title: Extensible
    details: Add Helm chart type generation, CRDs from any source, and community plugins for tools like FluxCD.
---

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

Every builder exports a PascalCase factory function — `Deployment()`, `Service()`, `Container()`. Properties are chainable methods with full type safety. See the [Getting Started guide](/guide/getting-started) to set up a project.
