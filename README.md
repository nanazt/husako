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

Pipe straight to kubectl:

```
$ husako render entry.ts | kubectl apply -f -
```

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

## Documentation

Full usage guide, CLI reference, and examples at **[nanazt.github.io/husako](https://nanazt.github.io/husako/)**.

## License

MIT
