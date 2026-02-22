# Milestone 7: Schema-based Validation

**Status**: Completed
**Commit**: `d8b98dd`

## Goal

Replace quantity-only validation with a general-purpose, schema-driven validation system. The OpenAPI schema itself drives what gets validated and how, covering type checking, required fields, enum values, quantity format, numeric bounds, and string patterns.

## Pipeline Change

```
compile() -> execute() -> validate_quantities() -> emit_yaml()    # old (M6)
compile() -> execute() -> validate(schema)      -> emit_yaml()    # new (M7)
```

## Deliverables

### Schema Store (`_schema.json`)

Generated during `husako init`, replaces `_validation.json`:

```json
{
  "version": 2,
  "gvk_index": {
    "apps/v1:Deployment": "io.k8s.api.apps.v1.Deployment",
    "v1:Namespace": "io.k8s.api.core.v1.Namespace"
  },
  "schemas": {
    "io.k8s.api.apps.v1.Deployment": { "properties": { ... }, "required": [...] },
    "io.k8s.apimachinery.pkg.api.resource.Quantity": { "type": "string", "format": "quantity" }
  }
}
```

- `gvk_index`: maps `<apiVersion>:<kind>` -> schema name (from `x-kubernetes-group-version-kind`)
- `schemas`: all component schemas with simplified `$ref` (stripped `#/components/schemas/` prefix)
- Quantity schema annotated with `"format": "quantity"` for custom validation dispatch

### Schema Validator

Recursive document + schema walker. At each node:

1. Resolve `$ref` -> follow to referenced schema
2. Handle `allOf` -> validate against each sub-schema
3. Handle `x-kubernetes-int-or-string` -> accept number or string
4. Skip `null` values (treat as "not set")
5. Check `type` (string, integer, number, boolean, array, object)
6. Check `required` fields present
7. Check `enum` values in allowed set
8. Check `format` — dispatch: `"quantity"` -> `is_valid_quantity()`
9. Check `minimum` / `maximum` for numbers
10. Check `pattern` for strings (regex via `regex-lite`)
11. Recurse into `properties`, `items`, `additionalProperties`

**Not in scope**: `oneOf`/`anyOf`, `x-kubernetes-validation` (CEL), unknown field rejection.

**Fallback**: When no `_schema.json` exists or GVK not found, same heuristic as before (quantity-check `resources.requests/limits`).

**Cycle protection**: Depth counter (max 64).

### Error Format

```
doc[0] at $.spec.replicas: expected type integer, got string
doc[0] at $.spec.strategy.type: invalid value "bluegreen", expected one of: Recreate, RollingUpdate
doc[0] at $.spec.containers[0].resources.requests.cpu: invalid quantity "2gb"
doc[0] at $.spec.selector: missing required field "selector"
```

## Architecture Decisions

### SchemaStore (`validate.rs`)

```rust
pub struct SchemaStore {
    gvk_index: HashMap<String, String>,
    schemas: HashMap<String, Value>,
}
```

Loaded from `_schema.json` at render time. Provides:
- `schema_for_gvk(api_version, kind)` -> look up document schema
- `resolve_ref(name)` -> follow `$ref` to referenced schema

### ValidationError

```rust
pub struct ValidationError {
    pub doc_index: usize,
    pub path: String,
    pub kind: ValidationErrorKind,
}

pub enum ValidationErrorKind {
    TypeMismatch { expected, got },
    MissingRequired { field },
    InvalidEnum { value, allowed },
    InvalidQuantity { value },
    PatternMismatch { value, pattern },
    BelowMinimum { value, minimum },
    AboveMaximum { value, maximum },
}
```

### Schema Store Generator (`schema_store.rs`)

Much simpler than the old `validation.rs` — no DFS walk needed:
1. Collect all `components.schemas` from all specs
2. Simplify `$ref` values (strip `#/components/schemas/` prefix)
3. Annotate Quantity schema with `format: "quantity"`
4. Build GVK index from `x-kubernetes-group-version-kind`
5. Sort keys for deterministic output

### Removed Code

From `quantity.rs`:
- `ValidationMap`, `PathSegment`, `QuantityPath` — path matching
- `walk_and_validate`, `validate_quantities`, `validate_doc_with_map` — path walking
- Related tests

Kept in `quantity.rs`:
- `is_valid_quantity()` — grammar checker (used by `validate.rs`)
- `QuantityError` — error type
- `validate_doc_fallback()` — heuristic for unknown schemas

Deleted files:
- `crates/husako-dts/src/validation.rs`
- `crates/husako-dts/src/snapshots/husako_dts__validation__tests__validation_json.snap`

## Files Created

```
crates/husako-core/src/validate.rs                    # SchemaStore + recursive validator
crates/husako-dts/src/schema_store.rs                 # _schema.json generator
crates/husako-dts/src/snapshots/..schema_store..snap  # Snapshot
```

## Files Modified

```
crates/husako-core/Cargo.toml          # +regex-lite
crates/husako-core/src/lib.rs          # schema_store replaces validation_map
crates/husako-core/src/quantity.rs     # Trimmed, validate_doc_fallback pub(crate)
crates/husako-dts/src/lib.rs           # _schema.json replaces _validation.json
crates/husako-cli/src/main.rs          # load_schema_store()
crates/husako-cli/Cargo.toml           # +husako-dts dev-dep
crates/husako-cli/tests/integration.rs # 6 new schema validation tests
```

## Tests

### Unit Tests — `schema_store.rs` (8 tests)

- Simplifies `$ref` values
- Annotates Quantity with `format: "quantity"`
- Builds correct GVK index (core group, named group)
- Core group has no prefix slash
- Merges multiple specs
- Version is 2
- Preserves required and enum
- Snapshot

### Unit Tests — `validate.rs` (20 tests)

- Type: string at integer field -> TypeMismatch
- Required: missing required field -> MissingRequired
- Enum: "bluegreen" for strategy -> InvalidEnum; "Always" -> ok
- Format quantity: "500m" -> ok; "2gb" -> InvalidQuantity; number -> ok
- Pattern: DNS label match -> ok; invalid -> PatternMismatch
- Bounds: port 80 -> ok; port 0 -> BelowMinimum; port 70000 -> AboveMaximum
- `x-kubernetes-int-or-string`: number/string -> ok, boolean -> error
- `allOf`: validates against all sub-schemas
- `$ref` resolution: follows chains correctly
- Null at optional position -> skip
- Depth limit: self-referencing schema -> no stack overflow
- Fallback: no schema store -> quantity heuristic
- Fallback: unknown GVK -> quantity heuristic
- SchemaStore: wrong version rejected, valid version accepted
- Display format for errors

### Integration Tests (6 new)

- `schema_invalid_enum_exit_7` — "bluegreen" strategy -> exit 7
- `schema_type_mismatch_exit_7` — string at integer replicas -> exit 7
- `schema_missing_required_exit_7` — missing selector -> exit 7
- `schema_valid_passes` — fully valid document -> exit 0
- `schema_invalid_quantity_exit_7` — "2gb" with schema -> exit 7
- `init_generates_schema_json` — init produces `_schema.json`

## Acceptance Criteria

- [x] Type mismatch detected and reported with path
- [x] Missing required fields detected
- [x] Invalid enum values detected with allowed list
- [x] Quantity validation via format dispatch
- [x] Numeric bounds checking (min/max)
- [x] Pattern matching via regex-lite
- [x] allOf validated against all sub-schemas
- [x] $ref chains followed correctly
- [x] Null values skipped (optional fields)
- [x] Self-referencing schemas don't stack overflow
- [x] Fallback to quantity heuristic when no schema
- [x] All 158 workspace tests pass
- [x] clippy clean, fmt clean
