mod helpers;

use std::collections::HashMap;

use husako_dts::{GenerateOptions, GenerateResult};

fn generate_from(specs: HashMap<String, serde_json::Value>) -> GenerateResult {
    let options = GenerateOptions { specs };
    husako_dts::generate(&options).expect("generate should succeed")
}

// ---------------------------------------------------------------------------
// Layer 1: Schema Parsing & Generation
// ---------------------------------------------------------------------------

#[test]
fn parse_k8s_core_v1() {
    let specs = helpers::load_k8s_fixtures();
    // Only use the core/v1 spec
    let core_spec = specs.get("api/v1").expect("api/v1 fixture should exist");

    let schemas = husako_dts::schema_store::generate_schema_store(&HashMap::from([(
        "api/v1".to_string(),
        core_spec.clone(),
    )]));
    let gvk_index = schemas["gvk_index"].as_object().unwrap();

    // Core types must be present in the GVK index
    assert!(
        gvk_index.contains_key("v1:Pod"),
        "Pod missing from GVK index"
    );
    assert!(
        gvk_index.contains_key("v1:Service"),
        "Service missing from GVK index"
    );
    assert!(
        gvk_index.contains_key("v1:Namespace"),
        "Namespace missing from GVK index"
    );
    assert!(
        gvk_index.contains_key("v1:ConfigMap"),
        "ConfigMap missing from GVK index"
    );
}

#[test]
fn parse_k8s_apps_v1() {
    let specs = helpers::load_k8s_fixtures();
    let apps_spec = specs
        .get("apis/apps/v1")
        .expect("apis/apps/v1 fixture should exist");

    let schemas = husako_dts::schema_store::generate_schema_store(&HashMap::from([(
        "apis/apps/v1".to_string(),
        apps_spec.clone(),
    )]));
    let gvk_index = schemas["gvk_index"].as_object().unwrap();

    assert!(
        gvk_index.contains_key("apps/v1:Deployment"),
        "Deployment missing"
    );
    assert!(
        gvk_index.contains_key("apps/v1:StatefulSet"),
        "StatefulSet missing"
    );
    assert!(
        gvk_index.contains_key("apps/v1:DaemonSet"),
        "DaemonSet missing"
    );
    assert!(
        gvk_index.contains_key("apps/v1:ReplicaSet"),
        "ReplicaSet missing"
    );
}

#[test]
fn k8s_core_v1_schema_count() {
    let specs = helpers::load_k8s_fixtures();
    let core_spec = specs.get("api/v1").expect("api/v1 fixture should exist");

    let schema_count = core_spec["components"]["schemas"]
        .as_object()
        .unwrap()
        .len();
    assert!(
        schema_count > 100,
        "core/v1 should have >100 schemas, got {schema_count}"
    );
}

#[test]
fn generate_from_real_k8s_specs() {
    let specs = helpers::load_k8s_fixtures();
    let result = generate_from(specs);

    // Common types
    assert!(
        result.files.contains_key("k8s/_common.d.ts"),
        "_common.d.ts missing"
    );

    // apps/v1
    assert!(
        result.files.contains_key("k8s/apps/v1.d.ts"),
        "apps/v1.d.ts missing"
    );
    assert!(
        result.files.contains_key("k8s/apps/v1.js"),
        "apps/v1.js missing"
    );

    // core/v1
    assert!(
        result.files.contains_key("k8s/core/v1.d.ts"),
        "core/v1.d.ts missing"
    );
    assert!(
        result.files.contains_key("k8s/core/v1.js"),
        "core/v1.js missing"
    );

    // _schema.json
    assert!(
        result.files.contains_key("k8s/_schema.json"),
        "_schema.json missing"
    );

    // DTS content checks
    let apps_dts = &result.files["k8s/apps/v1.d.ts"];
    assert!(
        apps_dts.contains("interface Deployment extends _ResourceBuilder"),
        "Deployment builder interface missing in apps/v1.d.ts"
    );
    assert!(
        apps_dts.contains("export function Deployment"),
        "Deployment() factory missing in apps/v1.d.ts"
    );

    // JS content checks
    let apps_js = &result.files["k8s/apps/v1.js"];
    assert!(
        apps_js.contains("class _Deployment"),
        "Deployment internal class missing in apps/v1.js"
    );
}

#[test]
fn generate_from_cert_manager() {
    let specs = helpers::load_crd_fixtures("cert-manager");
    assert!(!specs.is_empty(), "cert-manager fixtures should exist");

    let result = generate_from(specs);

    assert!(
        result.files.contains_key("k8s/cert-manager.io/v1.d.ts"),
        "cert-manager.io/v1.d.ts missing. Available files: {:?}",
        result.files.keys().collect::<Vec<_>>()
    );

    let dts = &result.files["k8s/cert-manager.io/v1.d.ts"];
    assert!(
        dts.contains("interface Certificate"),
        "Certificate interface missing in cert-manager DTS"
    );
    assert!(
        dts.contains("interface Issuer"),
        "Issuer interface missing in cert-manager DTS"
    );
}

#[test]
fn generate_from_fluxcd() {
    let specs = helpers::load_crd_fixtures("fluxcd");
    assert!(!specs.is_empty(), "fluxcd fixtures should exist");

    let result = generate_from(specs);

    // source toolkit
    assert!(
        result
            .files
            .contains_key("k8s/source.toolkit.fluxcd.io/v1.d.ts"),
        "source.toolkit.fluxcd.io/v1.d.ts missing. Available: {:?}",
        result.files.keys().collect::<Vec<_>>()
    );

    // kustomize toolkit
    assert!(
        result
            .files
            .contains_key("k8s/kustomize.toolkit.fluxcd.io/v1.d.ts"),
        "kustomize.toolkit.fluxcd.io/v1.d.ts missing"
    );

    // helm toolkit
    assert!(
        result
            .files
            .contains_key("k8s/helm.toolkit.fluxcd.io/v2.d.ts"),
        "helm.toolkit.fluxcd.io/v2.d.ts missing"
    );
}

#[test]
fn generate_from_cnpg() {
    let specs = helpers::load_crd_fixtures("cnpg");
    assert!(!specs.is_empty(), "cnpg fixtures should exist");

    let result = generate_from(specs);

    assert!(
        result.files.contains_key("k8s/postgresql.cnpg.io/v1.d.ts"),
        "postgresql.cnpg.io/v1.d.ts missing. Available: {:?}",
        result.files.keys().collect::<Vec<_>>()
    );

    let dts = &result.files["k8s/postgresql.cnpg.io/v1.d.ts"];
    assert!(
        dts.contains("interface Cluster"),
        "Cluster interface missing in cnpg DTS"
    );
}

// ---------------------------------------------------------------------------
// Layer 2: Schema Validation
// ---------------------------------------------------------------------------

#[test]
fn schema_store_from_real_specs() {
    let specs = helpers::load_k8s_fixtures();
    let store_json = husako_dts::schema_store::generate_schema_store(&specs);
    let store = husako_core::validate::SchemaStore::from_json(&store_json);
    assert!(
        store.is_some(),
        "SchemaStore should load from real-spec _schema.json"
    );
}

#[test]
fn validate_deployment_against_real_schema() {
    let specs = helpers::load_k8s_fixtures();
    let store_json = husako_dts::schema_store::generate_schema_store(&specs);
    let store =
        husako_core::validate::SchemaStore::from_json(&store_json).expect("store should load");

    let doc = serde_json::json!([{
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": {
            "name": "nginx",
            "namespace": "default",
            "labels": {"app": "nginx"}
        },
        "spec": {
            "replicas": 3,
            "selector": {
                "matchLabels": {"app": "nginx"}
            },
            "template": {
                "metadata": {
                    "labels": {"app": "nginx"}
                },
                "spec": {
                    "containers": [{
                        "name": "nginx",
                        "image": "nginx:1.27",
                        "ports": [{"containerPort": 80}],
                        "resources": {
                            "requests": {"cpu": "100m", "memory": "128Mi"},
                            "limits": {"cpu": "500m", "memory": "256Mi"}
                        }
                    }]
                }
            }
        }
    }]);

    let result = husako_core::validate::validate(&doc, Some(&store));
    assert!(
        result.is_ok(),
        "valid Deployment should pass validation: {:?}",
        result.err()
    );
}

#[test]
fn validate_invalid_enum_against_real_schema() {
    // Real k8s specs don't use enum constraints, but CRD specs do.
    // CNPG ClusterSpec has primaryUpdateStrategy enum: ["unsupervised", "supervised"].
    let specs = helpers::load_crd_fixtures("cnpg");
    let store_json = husako_dts::schema_store::generate_schema_store(&specs);
    let store =
        husako_core::validate::SchemaStore::from_json(&store_json).expect("store should load");

    let doc = serde_json::json!([{
        "apiVersion": "postgresql.cnpg.io/v1",
        "kind": "Cluster",
        "metadata": {"name": "test-pg"},
        "spec": {
            "instances": 3,
            "primaryUpdateStrategy": "bluegreen",
            "storage": { "size": "10Gi" }
        }
    }]);

    let result = husako_core::validate::validate(&doc, Some(&store));
    assert!(
        result.is_err(),
        "invalid primaryUpdateStrategy should be rejected"
    );

    let errors = result.unwrap_err();
    let has_enum_error = errors.iter().any(|e| {
        matches!(
            &e.kind,
            husako_core::validate::ValidationErrorKind::InvalidEnum { value, .. }
            if value == "bluegreen"
        )
    });
    assert!(
        has_enum_error,
        "should have InvalidEnum error for 'bluegreen', got: {:?}",
        errors.iter().map(|e| e.to_string()).collect::<Vec<_>>()
    );
}
