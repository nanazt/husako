# Milestone 12: Real-World OpenAPI Schema Testing

## Goal

Replace synthetic-only test coverage with real Kubernetes OpenAPI specs and CRD schemas. Verify the full pipeline (parse -> classify -> generate DTS/JS -> validate -> render) works correctly with production schemas from cert-manager, fluxcd, and cnpg.

## Fixture Layout

```
crates/husako-dts/tests/fixtures/openapi/
  k8s/
    VERSION                          # "kubernetes v1.35.1"
    api/v1.json                      # core/v1 (247 schemas)
    apis/apps/v1.json                # apps/v1 (162 schemas)
    apis/batch/v1.json               # batch/v1 (143 schemas)
    apis/networking.k8s.io/v1.json   # networking/v1 (55 schemas)
  crds/
    cert-manager/
      VERSION                        # "cert-manager v1.17.2"
      apis/cert-manager.io/v1.json
    fluxcd/
      VERSION                        # "fluxcd v2.4.0"
      apis/source.toolkit.fluxcd.io/v1.json
      apis/kustomize.toolkit.fluxcd.io/v1.json
      apis/helm.toolkit.fluxcd.io/v2.json
    cnpg/
      VERSION                        # "cnpg v1.25.1"
      apis/postgresql.cnpg.io/v1.json
```

Fixtures are committed to the repo. No network calls in CI.

## Changes

### New Files

| File | Purpose |
|------|---------|
| `crates/husako-dts/tests/helpers/mod.rs` | `load_k8s_fixtures()`, `load_crd_fixtures(name)` — recursive JSON scanning matching `scan_spec_files` pattern |
| `crates/husako-dts/tests/real_specs.rs` | Layer 1 (7 tests) + Layer 2 (3 tests) |
| `crates/husako-cli/tests/real_spec_e2e.rs` | Layer 3 E2E tests (3 tests) |

### Modified Files

| File | Change |
|------|--------|
| `crates/husako-dts/Cargo.toml` | Add `husako-core` and `serde_json` as dev-dependencies |

## Test Matrix (13 tests added)

### Layer 1: Schema Parsing & Generation

| Test | Asserts |
|------|---------|
| `parse_k8s_core_v1` | Pod, Service, Namespace, ConfigMap in GVK index |
| `parse_k8s_apps_v1` | Deployment, StatefulSet, DaemonSet, ReplicaSet in GVK index |
| `k8s_core_v1_schema_count` | >100 schemas in core/v1 |
| `generate_from_real_k8s_specs` | `_common.d.ts`, `apps/v1.{d.ts,js}`, `core/v1.{d.ts,js}`, `_schema.json`; DTS has builder classes + factories |
| `generate_from_cert_manager` | `cert-manager.io/v1.d.ts` with Certificate, Issuer classes |
| `generate_from_fluxcd` | Files for source/kustomize/helm toolkit groups |
| `generate_from_cnpg` | `postgresql.cnpg.io/v1.d.ts` with Cluster class |

### Layer 2: Schema Validation

| Test | Asserts |
|------|---------|
| `schema_store_from_real_specs` | `SchemaStore::from_json()` loads successfully |
| `validate_deployment_against_real_schema` | Valid Deployment passes validation |
| `validate_invalid_enum_against_real_schema` | CNPG `primaryUpdateStrategy: "bluegreen"` rejected with `InvalidEnum` |

### Layer 3: E2E Runtime

| Test | Asserts |
|------|---------|
| `e2e_render_deployment_from_real_specs` | `husako init` + `husako render` with `deployment().spec({...})` produces correct YAML |
| `e2e_render_cnpg_cluster` | CNPG `cluster().spec({...})` renders `kind: Cluster` |
| `e2e_render_cert_manager_certificate` | cert-manager `certificate().spec({...})` renders `kind: Certificate` |

## Known Findings

Real Kubernetes OpenAPI v3 specs wrap `$ref` inside `allOf: [{"$ref": "..."}]` to attach descriptions/defaults. The current `ts_type_from_schema` does not handle this wrapping, so per-property spec methods are not generated from real specs. E2E tests use `.spec({...})` as a workaround. This is a candidate for a future milestone.

## Verification

- 218 tests pass (`cargo test --workspace --all-features`)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean
- `cargo fmt --all --check` clean
