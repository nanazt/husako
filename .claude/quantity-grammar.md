# Kubernetes Quantity Grammar

Kubernetes quantity strings represent resource amounts (CPU, memory, storage). husako validates these in `crates/husako-core/src/quantity.rs`.

---

## Grammar

```
quantity ::= sign? numeric suffix?
           | sign? numeric exponent

sign     ::= '+' | '-'
numeric  ::= digits ('.' digits?)?
           | '.' digits
digits   ::= [0-9]+
suffix   ::= DecimalSI | BinarySI
exponent ::= ('e' | 'E') sign? digits

DecimalSI ::= 'n' | 'u' | 'm' | 'k' | 'M' | 'G' | 'T' | 'P' | 'E'
BinarySI  ::= 'Ki' | 'Mi' | 'Gi' | 'Ti' | 'Pi' | 'Ei'
```

**Disambiguation rule**: suffix is tried before exponent. `"1E"` matches the Exa suffix (valid). `"1E3"` does not match any suffix, so it falls through to exponent parsing (also valid). `"1Ei"` matches the EbiByte suffix (valid).

A bare number with no suffix or exponent is valid. An optional sign is allowed before the numeric part.

---

## Valid examples

| Value | Interpretation |
|-------|----------------|
| `500m` | 500 milli — 0.5 CPU cores |
| `1Gi` | 1 gibibyte (2^30 bytes) |
| `128Mi` | 128 mebibytes |
| `2.5Gi` | 2.5 gibibytes |
| `1k` | 1 kilo (10^3, decimal SI) |
| `1e3` | 1000 (scientific notation) |
| `1E` | Exa (10^18, decimal SI suffix) |
| `1E3` | 1000 (exponent, since `E3` is not a valid suffix) |
| `.5` | 0.5 (leading decimal point) |
| `+1` | 1 (explicit positive sign) |
| `0` | zero |

## Invalid examples

| Value | Reason |
|-------|--------|
| `1gi` | lowercase `gi` — not in the suffix list |
| `1mm` | double suffix — only one suffix allowed |
| `abc` | no leading numeric content |
| `Gi` | suffix without digits |
| `1 Gi` | space not allowed |
| `1.2.3` | double decimal point |
| `2gb` | `gb` is not a valid suffix |
| `` | empty string |

---

## Fallback heuristic (`validate_doc_fallback`)

When no `_schema.json` is available for a resource (e.g. CRDs not in the OpenAPI spec), `validate_doc_fallback()` applies a structural heuristic:

1. Recursively traverses the document at any nesting depth
2. Whenever it finds an object with a `resources` key, checks `resources.requests.*` and `resources.limits.*`
3. Validates each leaf value:
   - **String** → must pass `is_valid_quantity()`
   - **Number** or **null** → accepted without further validation
   - **Any other type** → reported as an error

This catches the most common quantity misuse (wrong unit strings like `"2gb"`) without requiring a full schema.
