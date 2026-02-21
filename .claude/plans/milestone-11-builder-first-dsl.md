# Milestone 11: Builder-First DSL

## Goal

Implement the builder-first DSL per `docs/builder-spec.md`. Replace `new Deployment()` with factory functions (`deployment()`), add `metadata()` as the entry point for metadata chains, and generate `_common.js` for common schema builders.

## Changes

### SDK (`husako-sdk`)

| File | Change |
|------|--------|
| `src/js/husako.js` | Add `metadata()` factory function |
| `src/dts/husako.d.ts` | Add `metadata()` type declaration |

### Emitter (`husako-dts`)

| File | Change |
|------|--------|
| `src/emitter.rs` | Add `to_factory_name()` helper; emit factory functions in `emit_builder_class()`, `emit_schema_builder_class()`, `emit_schema_builder_js()`, `emit_group_version_js()`; add `emit_common_js()`; update `emit_common()` to include schema builders |
| `src/lib.rs` | Emit `_common.js` alongside `_common.d.ts` when common schemas have complex properties |

### Templates

| File | Change |
|------|--------|
| `templates/simple/entry.ts` | Use `deployment()` factory, `metadata()` entry point |
| `templates/project/lib/metadata.ts` | Rename export to `appMetadata()`, use `metadata()` |
| `templates/project/lib/index.ts` | Re-export `appMetadata` |
| `templates/project/deployments/nginx.ts` | Use `deployment()` factory, `appMetadata()` |
| `templates/multi-env/base/nginx.ts` | Use `deployment()` factory, `metadata()` |
| `templates/multi-env/base/service.ts` | Use `service()` factory, `metadata()` |

### Examples

| File | Change |
|------|--------|
| `examples/canonical.ts` | Use `deployment()` factory, `metadata()` |

### Integration tests

| File | Change |
|------|--------|
| `tests/integration.rs` | Add factory functions to mock k8s modules; update all test code to use factory functions |

## Factory Function Naming

Class name with first character lowercased:

- `Deployment` → `deployment()`
- `StatefulSet` → `statefulSet()`
- `Container` → `container()`
- `LabelSelector` → `labelSelector()`

## Verification

- 209 tests pass (`cargo test --workspace --all-features`)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean
- `cargo fmt --all --check` clean
- `cargo run -- render examples/basic.ts` — identical YAML
