# Plugin Specification

This document defines the husako plugin system: manifest format, module resolution, storage layout, lifecycle, and helper authoring rules.

## Overview

A plugin extends husako with:

1. **Dependency presets** — pre-configured `[resources]`/`[charts]` sources (CRD schemas, chart repos)
2. **Helper modules** — importable JS + `.d.ts` with ergonomic builder classes
3. **Templates** — project scaffolds (future, Phase 3)

## Plugin Structure

```
husako-plugin-<name>/
├── plugin.toml           # Manifest (required)
├── modules/              # Importable JS + .d.ts (optional)
│   ├── index.js          # Default module: import from "<name>"
│   ├── index.d.ts        # TypeScript declarations
│   ├── helm.js           # Sub-module: import from "<name>/helm"
│   └── helm.d.ts
└── templates/            # Project templates (Phase 3, optional)
    └── gitops/
```

## Plugin Manifest (`plugin.toml`)

```toml
[plugin]
name = "fluxcd"
version = "0.1.0"
description = "FluxCD integration for husako"

# Dependency presets — merged into the project's schema resolution
[resources]
flux-source = { source = "git", url = "https://github.com/fluxcd/source-controller", path = "config/crd/bases" }
flux-helm = { source = "git", url = "https://github.com/fluxcd/helm-controller", path = "config/crd/bases" }

# Chart presets
[charts]
# (none for fluxcd, but plugins can declare chart sources)

# Module mappings: import specifier → file path (relative to plugin root)
[modules]
"fluxcd" = "modules/index.js"
"fluxcd/helm" = "modules/helm.js"
```

### Manifest Fields

| Field | Required | Description |
|-------|----------|-------------|
| `plugin.name` | Yes | Plugin name (lowercase, hyphens allowed) |
| `plugin.version` | Yes | SemVer version string |
| `plugin.description` | No | Short description |
| `resources` | No | Resource dependency presets (same schema as `husako.toml [resources]`) |
| `charts` | No | Chart dependency presets (same schema as `husako.toml [charts]`) |
| `modules` | No | Module import mappings |

### Resource Presets

Resource presets in `plugin.toml` use the same `SchemaSource` format as `husako.toml`, except git entries use `url` instead of `repo` for clarity:

```toml
[resources]
my-crd = { source = "git", url = "https://github.com/example/repo", path = "config/crd/bases" }
```

The `url` field is aliased to `repo` during parsing so the same `SchemaSource` struct is reused.

## Project Configuration (`husako.toml`)

```toml
[plugins]
fluxcd = { source = "git", url = "https://github.com/nanazt/husako-plugin-fluxcd" }
# Plugin bundled inside a monorepo — only the subdirectory is fetched
fluxcd = { source = "git", url = "https://github.com/nanazt/husako", path = "plugins/fluxcd" }
my-local = { source = "path", path = "./plugins/my-plugin" }
```

### Plugin Source Types

| Source | Fields | Description |
|--------|--------|-------------|
| `git` | `url`, `path` (optional) | Clone from a git repository (HEAD of default branch). When `path` is set, only that subdirectory is fetched via sparse-checkout (useful for monorepos). |
| `path` | `path` | Use a local directory (must be relative) |

## Storage Layout

```
.husako/
├── cache/            # Existing: downloaded specs
├── types/            # Existing: generated .d.ts + .js
│   ├── husako.d.ts
│   ├── k8s/
│   └── helm/
└── plugins/          # NEW: installed plugin copies
    └── fluxcd/       # One directory per plugin
        ├── plugin.toml
        └── modules/
            ├── index.js
            ├── index.d.ts
            ├── helm.js
            └── helm.d.ts
```

- Plugins are installed to `.husako/plugins/<name>/`.
- For `source = "git"` without `path`, the repo is shallow-cloned to `.husako/plugins/<name>/`.
- For `source = "git"` with `path`, only the subdirectory is fetched via sparse-checkout and its contents are copied to `.husako/plugins/<name>/`.
- For `source = "path"`, the directory is copied to `.husako/plugins/<name>/`.
- `.husako/plugins/` is ephemeral (like `.husako/types/`) — can be regenerated.

## Module Resolution

The resolver chain is updated to include plugins:

1. `BuiltinResolver` — `husako`, `husako/_base`
2. **`PluginResolver`** — plugin module imports (NEW)
3. `HusakoK8sResolver` — `k8s/*`
4. `HusakoHelmResolver` — `helm/*`
5. `HusakoFileResolver` — relative imports (`./`, `../`)

### PluginResolver Rules

Given plugin "fluxcd" with modules:
```toml
[modules]
"fluxcd" = "modules/index.js"
"fluxcd/helm" = "modules/helm.js"
```

- `import { HelmRelease } from "fluxcd"` → `.husako/plugins/fluxcd/modules/index.js`
- `import { helmRelease } from "fluxcd/helm"` → `.husako/plugins/fluxcd/modules/helm.js`

The resolver:
1. Checks if the import specifier matches any installed plugin's module mappings
2. Returns the absolute path to the mapped `.js` file
3. Falls through to the next resolver if no match

### TypeScript Support

Plugin `.d.ts` files are exposed via `tsconfig.json` path mappings:

```json
{
  "compilerOptions": {
    "paths": {
      "fluxcd": [".husako/plugins/fluxcd/modules/index.d.ts"],
      "fluxcd/helm": [".husako/plugins/fluxcd/modules/helm.d.ts"]
    }
  }
}
```

## Lifecycle

### `husako gen` (updated flow)

1. Process `[plugins]` — fetch/update each plugin to `.husako/plugins/<name>/`
2. Parse each plugin's `plugin.toml` manifest
3. Merge plugin `[resources]` presets into the resource resolution set
4. Merge plugin `[charts]` presets into the chart resolution set
5. Process all `[resources]` — generate k8s types (includes plugin CRDs)
6. Process all `[charts]` — generate helm types (includes plugin charts)
7. Update `tsconfig.json` path mappings to include plugin modules

### `husako plugin add <name> --path <dir>` / `husako plugin add <name> --url <git-url>`

1. Parse plugin manifest from source
2. Add `[plugins]` entry to `husako.toml`
3. Suggest running `husako gen`

### `husako plugin remove <name>`

1. Remove entry from `husako.toml [plugins]`
2. Remove `.husako/plugins/<name>/` directory
3. Remove plugin presets from generated types (next `husako gen` handles this)

### `husako plugin list`

List installed plugins with name, version, source, and module count.

### `husako clean`

`husako clean --all` or `husako clean --types` removes `.husako/plugins/` alongside `.husako/types/`.

## Helper Authoring Rules

### Extending `_ResourceBuilder`

Plugin helpers that represent Kubernetes resources must extend `_ResourceBuilder` so they work in `build()`:

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
    const resolved = ref && typeof ref._sourceRef === "function"
      ? ref._sourceRef()
      : ref;
    return this._setDeep("chart.spec.sourceRef", resolved);
  }
  interval(v) { return this._setSpec("interval", v); }
  values(v) {
    const resolved = v && typeof v._toJSON === "function" ? v._toJSON() : v;
    return this._setSpec("values", resolved);
  }
}

export function HelmRelease(name) {
  const r = new _HelmRelease();
  return name ? r.metadata({ name }) : r;
}
```

### Conventions

- **PascalCase factory functions** — `HelmRelease()`, `HelmRepository()`, matching husako's generated builder convention
- **Internal classes prefixed with `_`** — `_HelmRelease` is not exported
- **Accept both builders and plain objects** — Check for `._toJSON()` or `._sourceRef()` and resolve
- **Chain-returning methods** — Every setter returns `this` for fluent chaining
- **Immutable semantics** — Follow husako's copy-on-write pattern where applicable

### Using `_SchemaBuilder`

For non-resource types (like Helm chart values), extend `_SchemaBuilder`:

```javascript
import { _SchemaBuilder } from "husako/_base";

class _ChartValues extends _SchemaBuilder {
  replicaCount(v) { return this._set("replicaCount", v); }
}
```

### Module Exports

Every module must use ESM exports:

```javascript
// Good
export function HelmRelease(name) { ... }
export function HelmRepository(name) { ... }

// Bad — CommonJS not supported
module.exports = { ... };
```

## Naming Conventions

- Plugin repo: `husako-plugin-<name>` (convention, not enforced)
- Plugin name in manifest: lowercase with hyphens (`fluxcd`, `argo-cd`)
- Module specifiers: match plugin name (`"fluxcd"`, `"fluxcd/helm"`)
- Installed directory: `.husako/plugins/<name>/`
