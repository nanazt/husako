# CLI Reference

## husako new

Create a new project from a template.

```
husako new <directory> [options]
```

| Flag | Default | Description |
|------|---------|-------------|
| `-t, --template` | `simple` | Template name: `simple`, `project`, or `multi-env` |

Creates `husako.toml`, `entry.ts` (or template files), and `.gitignore` in the specified directory.

---

## husako generate

Generate type definitions and `tsconfig.json` from Kubernetes schemas and Helm charts.

Alias: `gen`.

```
husako generate [options]
husako gen [options]
```

| Flag | Description |
|------|-------------|
| `--api-server <url>` | Kubernetes API server URL |
| `--spec-dir <path>` | Local directory with pre-fetched OpenAPI spec files |
| `--skip-k8s` | Only write `husako.d.ts` and `tsconfig.json`, skip Kubernetes types |

Priority chain for k8s schema source: `--skip-k8s` → CLI flags → `husako.toml [resources]` → skip.

Chart types from `[charts]` are always generated when configured.

Plugins from `[plugins]` are installed first.

Output goes to `.husako/` (auto-managed, gitignored).

---

## husako render

Compile a TypeScript entry file and emit YAML to stdout.

```
husako render <file-or-alias> [options]
```

The file argument is resolved as: direct path → entry alias from `husako.toml` → error.

| Flag | Description |
|------|-------------|
| `--allow-outside-root` | Allow imports outside the project root |
| `--timeout-ms <ms>` | Execution timeout in milliseconds |
| `--max-heap-mb <mb>` | Maximum heap memory in megabytes |
| `--verbose` | Print diagnostic traces to stderr |

---

## husako init

Set up husako in an existing project directory (in-place, no new directory created).

```
husako init [options]
```

Same flags as `husako new` but runs in the current directory.

---

## husako clean

Remove cached files and generated types.

```
husako clean [options]
```

| Flag | Description |
|------|-------------|
| `--all` | Remove `.husako/` entirely (cache + types + plugins) |
| `--cache` | Remove only `.husako/cache/` |
| `--types` | Remove only `.husako/types/` and `.husako/plugins/` |

---

## husako list / husako ls

List all dependencies declared in `husako.toml`.

```
husako list
husako ls
```

Prints resource dependencies, chart dependencies, and plugins with their source types and versions.

---

## husako add

Add a resource or chart dependency to `husako.toml`.

```
husako add [url] [options]
```

The dep name, source type, resource/chart kind, and version are all detected automatically from the URL. Use `--name` to override the derived name, or when it cannot be derived (registry URLs).

| URL / flag | Detected source | Dep name |
|-----------|----------------|----------|
| `org/chart` | ArtifactHub | `chart` (after `/`) |
| `oci://…` | OCI registry | last path component |
| `https://github.com/…` | Git (resource or chart) | repo name |
| `https://charts.example.com` | Helm registry | required via `--name` |
| `./path` or `/abs/path` | Local file or dir | file stem or dir name |
| `--release [name]` | Kubernetes release | `k8s` or given name |
| `--cluster [name]` | Live cluster | `cluster` or given name |

| Flag | Description |
|------|-------------|
| `--name <name>` | Override the derived dependency name |
| `--version <ver>` | Pin to a version or partial prefix (`16`, `16.4`, `v1.16`) |
| `--tag <tag>` | Pin a git source to a specific tag |
| `--branch <branch>` | Pin a git source to a branch instead of the latest tag |
| `--path <subdir>` | Subdirectory within a git repo |
| `--release [name]` | Add a Kubernetes release schema source |
| `--cluster [name]` | Add a live-cluster schema source (prompts for confirmation) |

For registry URLs, the chart name must be given as a second positional argument or via `--name`.

After adding, run `husako generate` to fetch types.

---

## husako remove / husako rm

Remove a dependency from `husako.toml`.

```
husako remove <name>
husako rm <name>
```

Removes the named entry from `[resources]`, `[charts]`, or `[plugins]`.

Prompts for confirmation. Use `-y` / `--yes` to skip.

---

## husako outdated

Check for newer versions of versioned dependencies.

```
husako outdated
```

Queries upstream (GitHub releases, Helm registry, ArtifactHub, git tags) for each dependency that has a version field.

Reports which ones have updates available.

---

## husako update

Update versioned dependencies to their latest versions and regenerate types.

```
husako update [name] [options]
```

| Flag | Description |
|------|-------------|
| `--dry-run` | Show what would change without writing anything |
| `--resources-only` | Only update resource dependencies |
| `--charts-only` | Only update chart dependencies |

Pass a dependency name to update only that one.

Without a name, updates all versioned dependencies.

---

## husako info

Print a project summary and dependency details.

```
husako info [name]
```

Without a name, prints a project overview: config location, number of resources/charts/plugins, and whether types are generated.

With a dependency name, prints source details for that dependency.

---

## husako debug

Run health checks on the project setup.

```
husako debug
```

Checks: config file validity, generated types staleness, `tsconfig.json` consistency, and import path resolution.

Reports any issues with suggested fixes.

---

## husako validate

Compile TypeScript and validate resource structure without emitting YAML.

```
husako validate <file-or-alias>
```

Runs the full pipeline (TypeScript compile → execute → validate JSON contract) but does not write output.

Useful in CI to catch errors early.

---

## husako plugin

Manage plugins.

### husako plugin add

```
husako plugin add <name> [options]
```

| Flag | Description |
|------|-------------|
| `--url <git-url>` | Add a plugin from a git repository |
| `--path <dir>` | Add a plugin from a local directory |

Adds the plugin to `husako.toml [plugins]` and suggests running `husako generate`.

### husako plugin remove

```
husako plugin remove <name>
```

Removes the plugin from `husako.toml` and deletes `.husako/plugins/<name>/`.

### husako plugin list

```
husako plugin list
```

Lists installed plugins with name, version, source, and module count.

---

## husako test

Run TypeScript test files using the built-in `"husako/test"` assertion module.

```
husako test [FILE...] [options]
```

| Flag | Description |
|------|-------------|
| `--timeout-ms <ms>` | Execution timeout per file in milliseconds |
| `--max-heap-mb <mb>` | Maximum heap memory per file in megabytes |

With no `FILE` arguments, husako discovers all `*.test.ts` and `*.spec.ts` files under the
project root, excluding `.husako/` and `node_modules/`.

Test files import from `"husako/test"`:

```typescript
import { test, describe, expect } from "husako/test";
```

Exit code is 0 if all tests pass, 1 if any test fails.

Run `husako generate` (or `husako generate --skip-k8s`) before `husako test` to ensure
`husako/test.d.ts` and `tsconfig.json` path mappings are written.

See [Writing Tests](/guide/testing) for full examples and the assertion API reference.

---

## Global flags

| Flag | Description |
|------|-------------|
| `-y, --yes` | Skip all confirmation prompts |
| `--verbose` | Enable verbose diagnostic output |

---

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Unexpected failure |
| 2 | Invalid args/config |
| 3 | Compile failure (oxc) |
| 4 | Runtime failure (QuickJS / module loading) |
| 5 | Type generation failure |
| 6 | OpenAPI fetch/cache failure |
| 7 | Emit/validation/contract failure |
