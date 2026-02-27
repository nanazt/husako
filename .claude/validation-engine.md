# Validation Engine

husako validates every resource in `husako.build()` output before emitting YAML. Validation happens after the strict JSON contract check and before YAML serialization.

**Source files**: `crates/husako-core/src/validate.rs`, `crates/husako-core/src/quantity.rs`

---

## Two validation paths

### 1. JSON Schema validation (preferred)

Uses `_schema.json` files generated per group-version by `husako-dts`. The schema store is loaded from `.husako/types/k8s/_schema.json`.

For each document in the build output:
1. Look up `apiVersion` + `kind` in the `gvk_index` to find the schema name
2. Recursively validate the document against that schema

`SchemaStore` format: `{ "version": 2, "gvk_index": { "apps/v1:Deployment": "io.k8s.api..." }, "schemas": { ... } }`

### 2. Quantity-only fallback

Used when no `SchemaStore` is available, or when a document's GVK is not in the store (e.g. CRDs, custom resources). Calls `validate_doc_fallback()` from `quantity.rs`.

See `.claude/quantity-grammar.md` for how the fallback heuristic works.

---

## What IS validated

| Feature | Detail |
|---------|--------|
| Type | `string`, `number`, `integer`, `boolean`, `array`, `object`; null is skipped (treated as "not set") |
| `required` | missing required properties → error |
| `pattern` | regex match via `regex_lite` |
| `enum` | exact value match (string values only) |
| `minimum` / `maximum` | numeric bounds (f64) |
| `format: "quantity"` | Kubernetes quantity grammar via `is_valid_quantity()` |
| `$ref` | recursive resolution; depth guard at MAX_DEPTH = 64 |
| `allOf` | each sub-schema applied in sequence |
| `x-kubernetes-int-or-string` | accepts integer OR string; rejects anything else |
| `additionalProperties` (schema value) | validated when present as a schema object |

---

## What is NOT validated

- `oneOf` / `anyOf` — no validation; treated as any type
- `not` — ignored
- `if` / `then` / `else` — ignored
- `x-kubernetes-validation` / CEL expressions — ignored
- `additionalProperties: false` — unknown fields are not rejected
- `minLength` / `maxLength` — not implemented
- `minItems` / `maxItems` — not implemented
- Format values other than `"quantity"` — `"date"`, `"uuid"`, `"int64"`, etc. are not validated

---

## Error format

```
doc[N] at PATH: MESSAGE
```

- `N` — zero-based index into the build output array
- `PATH` — JSON path from document root, e.g. `$.spec.containers[0].resources.limits.cpu`
- `MESSAGE` — human-readable description of the failure

Examples:

```
doc[0] at $.spec.replicas: expected type integer, got string
doc[0] at $.spec.containers[0].resources.limits.cpu: invalid quantity "2gb"
doc[1] at $.spec: missing required field "selector"
doc[0] at $.spec.strategy.type: invalid value "bluegreen", expected one of: Recreate, RollingUpdate
```

Errors are **collected** (not short-circuited) — all errors in a document are reported together.

---

## Strict JSON contract (checked before schema validation)

husako enforces a strict JSON contract on the output of `husako.build()`. The following are banned and cause exit 7:

- `undefined`
- `bigint`
- `symbol`
- Functions and class instances
- `Date`, `Map`, `Set`, `RegExp`
- Circular references

This check runs in the QuickJS runtime before the Rust-side validation. A violation exits with code 7 immediately.
