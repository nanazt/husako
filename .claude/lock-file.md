# husako.lock — Implementation Reference

`husako.lock` enables incremental type generation. After a successful `husako gen`, the lock records what was generated and from which source versions. On subsequent runs, unchanged entries are skipped.

Commit `husako.lock` to version control — it serves the same role as `Cargo.lock`.

**Source files**: `crates/husako-config/src/lock.rs`, `crates/husako-core/src/lock_check.rs`

---

## HusakoLock struct

```toml
format_version = 1          # u32, bumped on breaking schema change
husako_version = "0.3.0"   # binary version string; mismatch forces full regen
# resources, charts, plugins are BTreeMap — deterministic TOML key order
```

All three maps use `#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]` — empty sections are omitted from the file.

---

## Entry enums

All enums use `#[serde(tag = "source")]` — the `source` field in TOML identifies the variant.

### ResourceLockEntry

```
Release { version, generated_at }
Git     { repo, tag, path, generated_at }
File    { path, content_hash, generated_at }   -- djb2 hash of file or directory
```

### ChartLockEntry

Each chart is independent — changes to one chart don't force regeneration of others.

```
Registry    { repo, chart, version, generated_at }
ArtifactHub { package, version, generated_at }
File        { path, content_hash, generated_at }
Git         { repo, tag, path, generated_at }
Oci         { reference, version, generated_at }
```

### PluginLockEntry

```
Git  { url, path: Option<String>, plugin_version, generated_at }
         -- path is set only for monorepo subdirectory installs
Path { path, content_hash, plugin_version, generated_at }
         -- content_hash hashes the source directory; falls back to installed dir
```

`plugin_version` comes from the plugin's `plugin.toml` manifest, not the lock itself. It's used to detect when a plugin author bumps their version without changing the source URL or path.

---

## Skip decision functions

Defined in `husako-core/src/lock_check.rs`.

### `should_skip_k8s()` — all-or-nothing

Returns `true` (skip) only if ALL conditions pass:

1. Lock file exists
2. `husako_version` in lock matches current binary version
3. `.husako/types/k8s/` directory exists and is non-empty
4. Resource names in config exactly match resource names in lock (no additions or removals)
5. Every resource passes its identity check

If any condition fails, all k8s types are regenerated. Individual resources cannot be skipped in isolation because `resolve_all()` merges all resource sources before codegen.

### `should_skip_chart(name)` — per-chart

Returns `true` (skip) only if ALL conditions pass:

1. Lock file exists and the chart is present in it
2. `husako_version` in lock matches current binary version
3. `.husako/types/helm/{name}.d.ts` exists
4. Source identity matches (repo+chart+version for registry, package+version for artifacthub, etc.)

### `should_skip_plugin(name)` — per-plugin

Returns `true` (skip) only if ALL conditions pass:

1. Lock file exists and the plugin is present in it
2. `.husako/plugins/{name}/` directory exists
3. Source identity matches:
   - Git: `url` + `path` both match the lock entry
   - Path: `path` matches AND directory content hash matches
4. The installed `plugin.toml` version equals `plugin_version` in the lock

---

## Hashing algorithm (djb2)

```
hash = 5381
for each byte b:
    hash = hash * 33 + b      (wrapping u64 arithmetic)
output: format!("{hash:016x}")  -- 16-character lowercase hex string
```

- **File**: djb2 of the raw file bytes
- **Directory**: collect all files recursively, sort lexicographically by path, then feed each file's relative path bytes + content bytes into a single running djb2 hash
- **Fallback (path source plugin)**: if hashing the source directory fails (returns empty string), hash the installed `.husako/plugins/<name>/` directory instead

---

## Load / Save

- `load_lock(project_root)` → `Ok(None)` if the file doesn't exist; `Err` only if the file exists but fails to parse
- `save_lock(project_root, lock)` serializes with `toml::to_string_pretty` and prepends a comment header
- Write failure during `generate()` is **non-fatal** — a warning is printed but generation continues
- `--skip-k8s` preserves the existing resource entries in the lock verbatim (no regen, no update to those entries)
- `--no-incremental` bypasses all skip checks; the lock is still written after generation, so the next run will be incremental again
