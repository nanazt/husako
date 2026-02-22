# PLAN.md — husako (Implement From Scratch)

Date: 2026-02-21 (Asia/Seoul)
Purpose: A single, high-signal implementation plan for agents (Claude Code / Codex) to build **husako** from zero.

---

## 0) Project overview

**husako** is a Rust CLI that generates Kubernetes YAML/JSON from a TypeScript “configuration as code” project:

- Users write modular, reusable TS modules (like a normal codebase).
- husako compiles TS to JS with **oxc**, executes it in **QuickJS** (`rquickjs`), captures a manifest value, validates it, and emits YAML.

Core pipeline:

**TypeScript (project) → oxc compile → QuickJS execute → capture output → validate → emit YAML/JSON**

---

## 1) Target authoring style (TypeScript)

### 1.1 Desired ergonomics (must support)

- Chainable builders / fragments.
- No required call order (you can chain in any sequence).
- Intermediate values can be assigned to variables and reused safely.
- Helpers that enable real workflows should live under the `husako` namespace (e.g. `husako.merge`).

### 1.2 Example (canonical)

> Note: `Deployment` is Kubernetes `apps/v1`, not `core/v1`.

```ts
import * as husako from "husako";
import { Deployment } from "k8s/apps/v1";
import { name, namespace, label, cpu, memory, requests, limits } from "husako";

const nginx_metadata = name("nginx")
  .namespace("nginx-ns")
  .label("key1", "value1")
  .label("key2", "value2");

const another_labels_1 = label("key3", "value3").label("key4", "value4");
const another_labels_2 = label("key5", "value5").label("key6", "value6");

const nginx = new Deployment()
  .metadata(husako.merge([nginx_metadata, another_labels_1, another_labels_2]))
  // Convenience: if you pass a ResourceList builder, treat it as requests by default.
  .resources(
    requests(cpu(1).memory("2Gi")),
    limits(cpu("500m").memory(1)), // numeric memory defaults to Gi (policy)
  );

husako.build([nginx]);
```

---

## 2) Hard contracts (do not break)

### 2.1 Entrypoint contract: `husako.build()`

- Entrypoint is executed as an **ESM module**.
- Entrypoint **must call** `husako.build(input)` exactly once.
- If build is not called → exit **7** with a clear error.
- If build is called multiple times → exit **7** with a clear error.

Optional transition support:

- If build was not called, failed command.
- If both exist, build wins (warn only under `--verbose`).

### 2.2 Strict JSON (default `--strict-json=true`)

The value captured from `husako.build(...)` must be JSON-serializable:

Allowed:

- `null | boolean | number | string`
- arrays and plain objects composed of allowed values

Forbidden:

- `undefined`, `bigint`, `symbol`
- functions, class instances, `Date`, `Map`, `Set`, `RegExp`
- cyclic references

Errors MUST include:

- `doc[index]`
- JSON path (`$.spec...`)
- value kind

### 2.3 Runtime import policy (no Node)

Supported:

- relative imports (`./`, `../`)
- builtin modules: `"husako"`, `"k8s/<group>/<version>"`

Not supported:

- npm/bare specifiers (other than the builtin set)
- Node built-ins (`fs`, `path`, `process`, …)
- network imports

Filesystem restriction:

- By default, resolved imports must stay within **project root** (canonicalized).
- Provide `--allow-outside-root` escape hatch.

### 2.4 Exit codes (stable)

- 0 success
- 1 unexpected failure
- 2 invalid args/config
- 3 compile failure (oxc)
- 4 runtime failure (QuickJS / module loading)
- 5 type generation failure
- 6 OpenAPI fetch/cache failure
- 7 emit/validation/contract failure

---

## 3) TypeScript runtime API (what must exist)

### 3.1 Builtin module: `"husako"`

Runtime exports (JS) + typings (`.d.ts`) must include:

- `build(input: BuildInput): void`
  - captures output to Rust via host sink
- `merge<T>(...xs: Mergeable<T>[]): T`
  - merges same-typed fragments/builders (unbounded arity)
- Fragment builders:
  - `name(v: string): MetadataFragment`
  - `namespace(v: string): MetadataFragment`
  - `label(k: string, v: string): MetadataFragment`
  - `annotation(k: string, v: string): MetadataFragment` (recommended)
- Resource quantity builders (for resources maps):
  - `cpu(v: number | string): ResourceListFragment`
  - `memory(v: number | string): ResourceListFragment`
  - (optional) `storage(v: number | string)`, `ephemeralStorage(v: number | string)`
  - `requests(x: ResourceListFragment): ResourceRequirementsFragment`
  - `limits(x: ResourceListFragment): ResourceRequirementsFragment`

### 3.2 Builtin modules: `"k8s/<group>/<version>"`

These are runtime modules (not type-only). They must export classes like:

- `class Deployment extends ResourceBuilder<AppsV1Deployment> {}`
- `class Namespace extends ResourceBuilder<V1Namespace> {}`
- etc.

**Rule:** These classes should be thin wrappers around a shared runtime base class (to keep runtime code small).

### 3.3 Immutability / reuse requirement

Fragments must be safe to reuse:

```ts
const base = label("env", "dev");
const a = base.label("team", "a");
const b = base.label("team", "b");
// a and b must not interfere.
```

Implementation requirement:

- Fragment objects are immutable (persistent) OR use copy-on-write.

---

## 4) Merge semantics (define precisely)

`husako.merge(...)` merges _same-typed_ fragments:

- For scalars: last argument wins
- For object maps (labels/annotations): deep-merge by key; last wins on conflicts
- For arrays: **replace** by default (no implicit concat)
  - If concatenation is needed, provide explicit helper later (e.g. `husako.concat(...)`)

This order-sensitive merge is intentional: it enables “base → environment override” workflows.

---

## 5) Quantity policy (string/number) + validation

### 5.1 Core requirement

Quantity-like fields must accept `string | number`, except schema says string-only or number-only.

### 5.2 Validation strategy (two-tier)

1. **Schema-aware** (preferred):

- During type generation, emit `.husako/types/k8s/_validation.json` that marks quantity paths per `<apiVersion>:<kind>`.
- During render, load this map (fast) and validate only known quantity fields.

2. **Fallback heuristic** (when validation map is missing):

- Validate only:
  - `resources.requests.*`
  - `resources.limits.*`

### 5.3 Normalization policy (practical, k8s-friendly)

For `resources.requests/limits` maps, normalization depends on the resource key:

- `cpu`:
  - `number`:
    - integer → `"N"`
    - fractional → `"Nm"` (millicores) if representable (e.g. `0.5` → `"500m"`)
  - `string`: validate Kubernetes quantity grammar
- `memory`, `ephemeral-storage`, `storage`:
  - `number`: interpret as Gi by default → `"NGi"` (matches desired ergonomics)
  - `string`: validate grammar (`"2Gi"` ok, `"2gb"` rejected)
- extended resources (e.g. `nvidia.com/gpu`):
  - `number`: `"N"`
  - `string`: validate grammar (or accept integers only; decide later)

For quantity fields outside resources maps:

- accept `number` and format as decimal string without suffix (no guessing).

> This policy is not Kubernetes “base unit” semantics; it is a husako UX decision for safety and practicality.
> Document it clearly in user docs.

---

## 6) Rust architecture (workspace design)

Recommended crates:

```
crates/
  husako-cli/            # clap parsing, IO, exit code mapping (thin)
  husako-core/           # orchestration + validation pipeline
  husako-compile-oxc/    # TS->JS compilation
  husako-runtime-qjs/    # QuickJS runner + module loader + build output capture
  husako-openapi/        # OpenAPI v3 fetch + disk cache
  husako-dts/            # OpenAPI -> .d.ts + _validation.json
  husako-yaml/           # JSON -> YAML/JSON emitter
  husako-sdk/            # builtin JS sources + base .d.ts (optional)
docs/
examples/
tests/
```

Boundary rules:

- CLI is thin.
- Runtime boundary payload is `serde_json::Value`.
- Error enums at crate boundaries (`thiserror`), user-facing formatting in CLI.

---

## 7) Implementation roadmap (from zero)

### Milestone 1 — Minimal `husako render` (single file, build-capture)

Deliver:

- oxc compile TS→JS
- QuickJS eval entry module
- builtin `"husako"` module provides `build()` that sends output to Rust sink
- strict JSON enforcement
- YAML emitter
- basic integration tests

Acceptance:

- `husako render examples/basic.ts` → exit 0, YAML output
- missing build → exit 7
- build called twice → exit 7

### Milestone 2 — Module loader + project imports (real code projects)

Deliver:

- relative import resolution (`./`, `../`) with extension + index inference
- project-root restriction + `--allow-outside-root`
- per-run compilation cache

Acceptance:

- `examples/project/env/dev.ts` imports modules and renders
- outside-root import fails by default

### Milestone 3 — Minimal runtime SDK for your chaining style

Deliver:

- `"husako"` exports: `merge`, `name/namespace/label`, `cpu/memory/requests/limits`
- `"k8s/apps/v1"` exports: `Deployment` builder class
- `"k8s/core/v1"` exports: `Namespace` builder class (and a couple more)
- builders support `.metadata(...)` and `.resources(...)`

Acceptance:

- canonical TS example in Section 1 renders correct YAML

### Milestone 4 — OpenAPI fetch/cache (offline-capable)

Deliver:

- fetch OpenAPI v3 index + documents (URL and file)
- disk cache under `.husako/cache/openapi/`

Acceptance:

- integration test using local mock HTTP server proves cache reuse + offline mode

### Milestone 5 — Type generation + `husako init`

Deliver:

- generate `.d.ts` for:
  - `"husako"` module
  - `"k8s/<group>/<version>"` modules
- `husako init` writes/updates `tsconfig.json` paths to `.husako/types/`

Acceptance:

- editor can import from `"k8s/apps/v1"` and get autocomplete for models + builder APIs

### Milestone 6 — Schema-aware quantity validation

Deliver:

- typegen emits `_validation.json`
- render uses it for quantity validation across full schema surface
- fallback heuristic remains

Acceptance:

- a quantity field outside resources maps is validated when map exists

### Milestone 7 — Safety & diagnostics

Deliver:

- `--timeout-ms`, `--max-heap-mb` (best-effort)
- stack readability via `sourceURL` and optional sourcemaps
- `--verbose` traces (stderr only)

### Milestone 8 — Benchmarks & release engineering

Deliver:

- criterion benchmarks
- GitHub Actions CI + release artifacts + checksums
- docs for install/perf

---

## 8) Testing requirements (non-negotiable)

- Integration tests (`assert_cmd`) for:
  - exit code mapping
  - import resolution behavior
  - strict JSON contract failures
  - quantity validation failures with JSON path reporting
- Snapshot tests (`insta`) for YAML output.
- No external network usage in tests (use a local mock server for OpenAPI).

---

## 9) Quick reference commands

```bash
# Build
cargo build

# Release build
cargo build --release

# Tests
cargo test --workspace

# Lint / format
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

---

## 10) Files to never edit manually

- `.husako/cache/**` — auto-managed cache
- `.husako/types/**` — generated `.d.ts` + `_validation.json`
- any generated CLI reference docs (if added later)

---

## 11) Key dependencies (expected)

- `clap` — CLI parsing
- `oxc_*` — TS parsing/transforms/codegen
- `rquickjs` — QuickJS runtime embedding
- `serde_json`, `serde_yaml` — serialization/emission
- `thiserror` — typed errors
- `reqwest` (blocking) — OpenAPI fetch (typegen path only)
- `tempfile` — tests
- `insta`, `assert_cmd` — integration + snapshot testing

---

## 12) Release profile (recommended)

If binary size matters:

- `opt-level = "z"`
- `lto = true`
- `codegen-units = 1`
- `strip = true`

---

## 13) Milestone 13 — `husako.toml` configuration file

### M13a: `husako-config` crate + entry aliases (**DONE**)

- New `husako-config` crate: parses `husako.toml` with `HusakoConfig`, `SchemaSource` (tagged enum), `ClusterConfig`
- Validation: no absolute paths, cluster references must resolve, no `[cluster]` + `[clusters]` together
- CLI alias resolution: direct path → entry alias → error with available aliases list
- `HusakoError::Config` variant (exit code 2)
- Template TOML files for all 3 templates (`simple`, `project`, `multi-env`)
- 24 new tests (15 unit + 9 integration), total 242

### M13b+M13c: Schema source resolution (**DONE**)

- `husako init` reads `[schemas]` from `husako.toml` (config-driven mode)
- Init priority chain: `--skip-k8s` → CLI flags (legacy) → `husako.toml [schemas]` → skip
- `source = "file"`: CRD YAML → OpenAPI JSON converter (`crd.rs`), nested schema extraction, reverse-domain naming
- `source = "cluster"`: kubeconfig auto-detection from `~/.kube/` files, bearer token extraction, matched by server URL
- `source = "release"`: download OpenAPI specs from kubernetes/kubernetes GitHub releases, tag-based deterministic cache
- `source = "git"`: shallow clone at tag, extract CRD YAML, convert and cache
- Schema orchestrator (`schema_source.rs`): dispatches all 4 source types, groups CRD schemas by GVK into discovery-keyed specs
- 28 new tests, total 270

See `.claude/plans/m13-husako-toml.md` for detailed design.

---

End of plan.
