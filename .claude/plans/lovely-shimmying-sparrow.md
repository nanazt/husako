# Plan: Chain API Part 2 — Explicit Resources Form

## Context

Part 1 completed: chain starters (`name`, `label`, `cpu`, `memory`, etc.) moved from `"husako"` to
`"k8s/meta/v1"` and `"k8s/core/v1"`. `husako` now exports only `build` as a default.

Part 2 adds a `requests()` wrapper function to `"k8s/core/v1"`, making limits expressible in a
type-safe way without `Record<string, any>`. The pattern mirrors the old `"husako"` API but the
import source moves to `"k8s/core/v1"`. `_ResourceChain._toJSON()` changes to return a bare resource
list; `requests()` wraps it into a `_ResourceRequirementsChain` that carries both requests and
optional limits, and `_toJSON()` on that chain produces the full `{ requests, limits? }` structure.
`_ResourceBuilder.resources()` already calls `chain._toJSON()` — no additional special handling
needed.

## Design Decision

```typescript
import { cpu, memory, requests } from "k8s/core/v1";

// BEFORE (removed — auto-wrap shorthand)
.resources(cpu("250m").memory("128Mi"))

// AFTER
.resources(requests(cpu("250m").memory("128Mi")))                      // requests only
.resources(requests(cpu("250m").memory("128Mi"))
           .limits(cpu("500m").memory("256Mi")))                       // requests + limits
```

**Type signatures:**
```typescript
// _ResourceChain — bare resource list; returned by cpu() / memory()
interface _ResourceChain {
  cpu(v: string | number): _ResourceChain;
  memory(v: string | number): _ResourceChain;
  _toJSON(): { cpu?: string; memory?: string };   // bare list — no requests wrapper
}

// _ResourceRequirementsChain — full requirements; returned by requests()
interface _ResourceRequirementsChain {
  limits(chain: _ResourceChain): _ResourceRequirementsChain;
  _toJSON(): { requests?: Record<string, string>; limits?: Record<string, string> };
}

// resources() on SpecFragment and ResourceBuilder
resources(r: _ResourceRequirementsChain): this;
```

## Critical Files

| File | Change |
|------|--------|
| `crates/husako-sdk/src/js/husako_base.js` | `_createResourceChain._toJSON()` bare list; add `_createResourceRequirementsChain()` |
| `crates/husako-sdk/src/dts/husako_base.d.ts` | Add `_ResourceRequirementsChain`; update `_ResourceChain._toJSON()` type; update `resources()` signatures |
| `crates/husako-sdk/src/js/husako_k8s_core_v1.js` | Export `requests()` starter |
| `crates/husako-sdk/src/dts/husako_k8s_core_v1.d.ts` | Export `requests: (chain: _ResourceChain) => _ResourceRequirementsChain` |
| `crates/husako-runtime-qjs/src/lib.rs` | Update mock `k8s/core/v1.js` to add `requests()`; update test JS strings |
| `crates/husako-cli/tests/integration.rs` | Update test TS strings and snapshots |

## Tasks

### Task 1 — `husako_base.js`

**1a. `_createResourceChain._toJSON()` → bare list**
```javascript
// before
return Object.keys(req).length > 0 ? { requests: req } : {};
// after
return req;  // bare { cpu?, memory? }
```

**1b. Add `_createResourceRequirementsChain()`**
```javascript
export function _createResourceRequirementsChain(reqList) {
  const r = {
    _husakoTag: "ResourceRequirementsChain",
    _requests: reqList,
    _limits: undefined,
  };
  r.limits = function(chain) {
    r._limits = chain && typeof chain._toJSON === "function" ? chain._toJSON() : chain;
    return r;
  };
  r._toJSON = function() {
    const obj = {};
    if (r._requests && Object.keys(r._requests).length > 0) obj.requests = r._requests;
    if (r._limits && Object.keys(r._limits).length > 0) obj.limits = r._limits;
    return obj;
  };
  return r;
}
```

### Task 2 — `husako_base.d.ts`

```typescript
// Add new interface
export interface _ResourceRequirementsChain {
  readonly _husakoTag: "ResourceRequirementsChain";
  limits(chain: _ResourceChain): _ResourceRequirementsChain;
  _toJSON(): Record<string, any>;
}

// Update _ResourceChain._toJSON return type comment (bare list)
_toJSON(): Record<string, any>;

// Update resources() signatures — _ResourceChain no longer accepted directly
resources(r: _ResourceRequirementsChain): _SpecFragment;   // on _SpecFragment
resources(value: _ResourceRequirementsChain): this;        // on _ResourceBuilder
```

### Task 3 — `husako-dts/src/emitter.rs`: emit `requests()` for core/v1

`k8s/core/v1.js` is **generated** (not static). `husako-sdk` only has `husako_base.js`,
`husako.js`, `husako_test.js`. The emitter already emits `cpu()` / `memory()` chain starters for
`k8s/core/v1`. Add `requests()` emission to that same site.

In `emitter.rs`, when emitting the `k8s/core/v1` module (or any module that includes
`ResourceRequirements`-related schemas), append:

**`.js` output:**
```javascript
import { _createResourceRequirementsChain } from "husako/_base";
export function requests(chain) {
  const list = chain && typeof chain._toJSON === "function" ? chain._toJSON() : chain;
  return _createResourceRequirementsChain(list);
}
```

**`.d.ts` output:**
```typescript
import { _ResourceChain, _ResourceRequirementsChain } from "husako/_base";
export function requests(chain: _ResourceChain): _ResourceRequirementsChain;
```

Look at how `cpu()` / `memory()` are emitted in `emitter.rs` — `requests()` should follow the same
pattern, likely triggered by the presence of `ResourceList`-typed schemas in the same module.

### Task 4 — `husako-runtime-qjs/src/lib.rs`: update tests

- Update mock `k8s/core/v1.js` in `test_options_with_k8s` to add `requests()` export
- `cpu_normalization`: `resources(cpu(0.5))` → `resources(requests(cpu(0.5)))`
- `memory_normalization`: same
- `spec_overrides_resources`: check for direct `_ResourceChain` to `resources()` — update if present
- `resources_requests_and_limits`: verify still passes (may already use correct form)

### Task 5 — `husako-cli/tests/integration.rs`: update tests + snapshots

- `cpu_normalization`: `.resources(cpu(0.5))` → `.resources(requests(cpu(0.5)))`
- `memory_normalization`: same
- Update import stubs: `k8s/core/v1.d.ts` stub in `project_with_typed_k8s` to include `requests`
- Re-run with `INSTA_UPDATE=always` if snapshots change

### Task 6 — `dsl-spec.md`: update resources section

- Section 2 Fragment Builders: replace `ResourceRequirementsFragment` row with new `_ResourceRequirementsChain` entry showing `requests()` + `.limits()`
- Section 1 example: update to show `requests(cpu(...).memory(...)).limits(...)` form
- Section 3 Import Rules: add `requests` to `k8s/core/v1` exports row

## Tests to Add / Modify

- **Modify** `husako-runtime-qjs`: `cpu_normalization`, `memory_normalization`, `spec_overrides_resources`
- **Modify** `husako-cli`: `cpu_normalization`, `memory_normalization`; update `k8s/core/v1.d.ts` stub
- **Add** `resources_with_limits` in both crates:
  `requests(cpu("250m").memory("128Mi")).limits(cpu("500m").memory("256Mi"))` → verify YAML contains both `requests:` and `limits:` under `resources:`

## Docs to Update

- `dsl-spec.md` (Task 6) — internal design doc

## Impact on Parts 2–4

| Part | Impact |
|------|--------|
| Part 1 | Completed — no remaining work affected |
| Part 2 (LSP) | Aligned. Part 2 spec already references `request()` / `limit()` starters for quantity completions — this plan provides exactly those (`requests()` and `.limits()`). LSP quantity completion triggers on `cpu("...")` / `memory("...")` cursor regardless of enclosing call. |
| Part 3 (Editor) | None |
| Part 4 (Tests/Docs) | `ContainerChain.resources()` signature stays `resources(r: _ResourceRequirementsChain)` — matches Part 4 DSL spec draft. No conflict. |

## Verification

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
# Confirm: resources(cpu("250m")) is a TypeScript type error
# Confirm: resources(requests(cpu("250m")).limits(cpu("500m"))) produces correct YAML
```
