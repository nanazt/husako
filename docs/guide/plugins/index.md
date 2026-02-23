# Official Plugins

Plugins extend husako with dependency presets and importable helper modules.

## What plugins provide

A plugin can declare:

1. **Dependency presets** — pre-configured `[resources]` entries (CRD schemas) and `[charts]`
   entries (chart repos) that are merged into the project's generation set automatically
2. **Helper modules** — importable TypeScript-typed JS files with builder classes for
   plugin-specific resource kinds (e.g. `import { HelmRelease } from "flux"`)

Plugin modules follow the same factory function convention as generated builders. Your editor
gets autocomplete from the plugin's `.d.ts` files, wired in via `tsconfig.json` path mappings.

## Installing a plugin

Add an entry to `[plugins]` in `husako.toml`:

```toml
[plugins]
flux = { source = "git", url = "https://github.com/nanazt/husako", path = "plugins/flux" }
```

Then run `husako generate`. husako fetches the plugin, merges its dependency presets, generates
all types (including CRDs declared by the plugin), and updates `tsconfig.json`.

**Interactive install:**

```
husako plugin add <name> --url <git-url>
husako plugin add <name> --path <local-dir>
```

## Official plugins

| Plugin | Description | Modules |
|--------|-------------|---------|
| [Flux CD](./flux) | GitOps controllers for Kubernetes | `"flux"`, `"flux/source"` |

The Flux CD plugin ships bundled in the husako repository at `plugins/flux/`. It provides
builders for `HelmRelease`, `Kustomization`, and all three source types.

## Community plugins

Any git repository containing a valid `plugin.toml` works as a husako plugin. Users reference
it via:

```toml
[plugins]
my-plugin = { source = "git", url = "https://github.com/example/husako-plugin-example" }
```

See [Writing a Plugin](../../advanced/plugins) if you want to build your own.
