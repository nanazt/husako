# Milestone 6: Schema-aware Quantity Validation

**Status**: Completed (superseded by Milestone 7)
**Commit**: `ec4ccfc`

## Goal

Validate quantity fields in the render output using both schema-aware path matching and a fallback heuristic for unknown kinds.

## Deliverables

### Quantity Grammar Checker

Validates strings against the Kubernetes quantity grammar:
- Suffixes: DecimalSI (`n`, `u`, `m`, `k`, `M`, `G`, `T`, `P`, `E`), BinarySI (`Ki`, `Mi`, `Gi`, `Ti`, `Pi`, `Ei`)
- Exponents: `e3`, `E+3`, `e-3`
- Bare numbers: `100`, `2.5`, `.5`
- Signs: `+1`, `-1`
- Disambiguation: `"1E"` = Exa suffix, not incomplete exponent

### Validation Map (`_validation.json`)

Generated during `husako init`:
- DFS walks the OpenAPI schema graph from each top-level resource
- When a `$ref` points to `io.k8s.apimachinery.pkg.api.resource.Quantity`, records the JSONPath
- Handles arrays (`items` -> `[*]`), maps (`additionalProperties` -> `[*]`), nested refs
- Cycle detection via backtracking visited set

Format:
```json
{
  "version": 1,
  "quantities": {
    "apps/v1:Deployment": [
      "$.spec.template.spec.containers[*].resources.limits[*]",
      "$.spec.template.spec.containers[*].resources.requests[*]"
    ]
  }
}
```

### Two-tier Validation

1. **Schema-aware** (when `_validation.json` exists):
   - Load validation map, look up paths by `<apiVersion>:<kind>`
   - Walk document along recorded paths, validate leaf values
2. **Fallback heuristic** (when no map or unknown kind):
   - Recursively search for `resources.requests.*` and `resources.limits.*` at any depth

### Error Format

```
doc[0] at $.spec.template.spec.containers[0].resources.requests.cpu: invalid quantity "2gb"
```

## Architecture Decisions

### Path Matching (`quantity.rs`)

```rust
enum PathSegment { Field(String), Wildcard }
struct QuantityPath { segments: Vec<PathSegment> }
struct ValidationMap { entries: HashMap<String, Vec<QuantityPath>> }
```

The walker follows PathSegments through the JSON document:
- `Field("name")` -> descend into object key
- `Wildcard` -> iterate over array elements or object values

### Integration Point

```rust
// In render():
if let Err(errors) = quantity::validate_quantities(&value, validation_map.as_ref()) {
    return Err(HusakoError::Validation(msg));
}
```

## Files Created/Modified

```
crates/husako-core/src/quantity.rs        # Grammar checker + path walker + fallback
crates/husako-dts/src/validation.rs       # _validation.json generator (DFS)
crates/husako-core/src/lib.rs             # load_validation_map(), RenderOptions
crates/husako-cli/src/main.rs             # Load validation map on render
```

## Tests

### Unit Tests (quantity.rs)

- Grammar: valid quantities (bare, decimal, signed, millicores, binary SI, decimal SI, exa, exponent)
- Grammar: invalid quantities (empty, no digits, wrong suffix, spaces, multiple dots)
- Path parsing: simple fields, wildcards
- Walk: field paths, invalid fields, wildcards on maps/arrays, missing fields
- Fallback: validates resources, valid pass, no resources is OK
- ValidationMap: from_json, unknown kind uses fallback

### Unit Tests (validation.rs)

- Detects quantity paths in Deployment
- PV capacity detection
- Cycle detection (A -> B -> A)
- No quantities produces empty
- Snapshot: `_validation.json`

### Integration Tests

- `invalid_quantity_fallback_exit_7` — "2gb" at cpu -> exit 7
- `valid_quantities_exit_0` — "500m", "1Gi" -> exit 0
- `numbers_at_quantity_positions_exit_0` — numeric values OK

## Note

This milestone was superseded by Milestone 7, which replaced the quantity-only validation with a full schema-driven validator. The `_validation.json` was replaced by `_schema.json`, and the path-matching code was removed in favor of a recursive schema walker.
