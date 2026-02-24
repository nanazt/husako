# Plugins

Plugins extend husako with dependency presets, importable helper modules, and project templates.

## Overview

A plugin can provide:

1. **Dependency presets** — pre-configured `[resources]` and `[charts]` entries (CRD schemas, chart repos)
2. **Helper modules** — importable TypeScript-typed JS files with ergonomic builder classes
3. **Templates** — project scaffolds (planned)

The official bundled plugin is the FluxCD plugin (`plugins/fluxcd/` in the husako repository).

It provides builders for `HelmRelease`, `Kustomization`, `GitRepository`, `HelmRepository`, and `OCIRepository`.

---

## Installing a plugin

**Interactive:**

```
husako plugin add <name> --url <git-url>
husako plugin add <name> --path <local-dir>
```

**Manual (`husako.toml`):**

```toml
[plugins]
fluxcd = { source = "path", path = "plugins/fluxcd" }
my-local = { source = "path", path = "./plugins/my-plugin" }
```

After adding a plugin, run `husako generate` to install it and regenerate types.

---

## Using plugin modules

Once installed, import from the plugin's declared module specifiers:

```typescript
import { HelmRelease, Kustomization } from "fluxcd";
import { GitRepository, HelmRepository } from "fluxcd/source";
```

The specifiers are declared in the plugin's `plugin.toml`.

Your editor gets autocomplete from the plugin's `.d.ts` files, which are wired into `tsconfig.json` path mappings by `husako generate`.

---

## Plugin structure

A plugin is a directory with the following layout:

```
husako-plugin-<name>/
├── plugin.toml           # Manifest (required)
├── modules/              # Importable modules (optional)
│   ├── index.js
│   ├── index.d.ts
│   ├── source.js         # Sub-module
│   └── source.d.ts
└── templates/            # Project templates (optional, planned)
```

Convention: the repository is named `husako-plugin-<name>`. Not enforced.

---

## Plugin manifest

`plugin.toml` declares the plugin's identity, dependency presets, and module mappings:

```toml
[plugin]
name = "fluxcd"
version = "0.1.0"
description = "FluxCD integration for husako"

[resources]
flux-source = { source = "git", url = "https://github.com/fluxcd/source-controller", path = "config/crd/bases" }
flux-helm = { source = "git", url = "https://github.com/fluxcd/helm-controller", path = "config/crd/bases" }

[modules]
"fluxcd" = "modules/index.js"
"fluxcd/source" = "modules/source.js"
```

| Field | Required | Description |
|-------|----------|-------------|
| `plugin.name` | Yes | Plugin name (lowercase, hyphens allowed) |
| `plugin.version` | Yes | SemVer version string |
| `plugin.description` | No | Short description |
| `resources` | No | Resource dependency presets |
| `charts` | No | Chart dependency presets |
| `modules` | No | Module specifier → file path mappings |

Resource presets in `plugin.toml` use the same format as `husako.toml [resources]`, but `url` is used instead of `repo` for git sources.

---

## Writing a plugin

### Extending _ResourceBuilder

Plugin modules that represent Kubernetes resources should extend `_ResourceBuilder` so they work with `build()`:

```javascript
import { _ResourceBuilder } from "husako/_base";

class _HelmRelease extends _ResourceBuilder {
  constructor() {
    super("helm.toolkit.fluxcd.io/v2", "HelmRelease");
  }

  chart(name, version) {
    return this._setDeep("chart.spec", { chart: name, version: version });
  }

  sourceRef(ref) {
    const resolved = typeof ref._sourceRef === "function"
      ? ref._sourceRef()
      : ref;
    return this._setDeep("chart.spec.sourceRef", resolved);
  }

  interval(v) { return this._setSpec("interval", v); }

  values(v) {
    const resolved = typeof v._toJSON === "function" ? v._toJSON() : v;
    return this._setSpec("values", resolved);
  }
}

export function HelmRelease(name) {
  const r = new _HelmRelease();
  return name ? r.metadata({ name }) : r;
}
```

### Extending _SchemaBuilder

For non-resource types (e.g. typed value objects), extend `_SchemaBuilder`:

```javascript
import { _SchemaBuilder } from "husako/_base";

class _ChartValues extends _SchemaBuilder {
  replicaCount(v) { return this._set("replicaCount", v); }
}

export function ChartValues() { return new _ChartValues(); }
```

### Conventions

- **PascalCase factory functions** — `HelmRelease()`, `GitRepository()`, matching husako's generated convention
- **Internal classes prefixed with `_`** — `_HelmRelease` is not exported
- **Accept both builders and plain objects** — check for `._toJSON()` or `._sourceRef()` before using
- **Chain-returning methods** — every setter should return `this` for fluent chaining
- **ESM exports only** — CommonJS (`module.exports`) is not supported

### TypeScript declarations

Provide `.d.ts` alongside each `.js` module:

```typescript
// index.d.ts
import { _ResourceBuilder } from "husako/_base";

export interface HelmRelease extends _ResourceBuilder {
  chart(name: string, version: string): this;
  sourceRef(ref: any): this;
  interval(v: string): this;
  values(v: object): this;
}

export function HelmRelease(name?: string): HelmRelease;
```

---

## Plugin lifecycle

1. `husako generate` reads `[plugins]` from `husako.toml`
2. For `git` sources: shallow-clones to `.husako/plugins/<name>/`
3. For `path` sources: copies the directory to `.husako/plugins/<name>/`
4. Parses `plugin.toml` from each installed plugin
5. Merges plugin `[resources]` presets into the generation set
6. Merges plugin `[charts]` presets into the generation set
7. Generates k8s types (including plugin CRDs)
8. Generates Helm types (including plugin charts)
9. Writes `tsconfig.json` path mappings for plugin modules

`husako plugin remove <name>` removes the entry from `husako.toml` and deletes `.husako/plugins/<name>/`.

`husako clean --types` removes `.husako/plugins/` alongside `.husako/types/`.

Re-running `husako generate` reinstalls everything from scratch.
