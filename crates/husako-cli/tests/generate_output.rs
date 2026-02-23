/// Integration tests validating the **structural correctness** of `husako generate` output.
///
/// Unlike the existing `integration.rs` (exit codes, grep for keywords) and
/// `real_spec_e2e.rs` (render round-trips), these tests check:
///   - `_schema.json` GVK index completeness and internal consistency
///   - Generated `.js` runtime code (per-property methods, deep-path shortcuts)
///   - Generated `.d.ts` type definitions (interfaces, imports, builder patterns)
///   - `tsconfig.json` path mapping integrity
use std::path::Path;

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn husako_at(dir: &Path) -> assert_cmd::Command {
    let mut cmd = cargo_bin_cmd!("husako");
    cmd.current_dir(dir);
    cmd
}

fn fixtures_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../husako-dts/tests/fixtures/openapi")
}

fn copy_fixture(fixture_name: &str, target: &Path) {
    let src = fixtures_dir().join(fixture_name);
    copy_dir_recursive(&src, target);
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

/// Run `husako generate --spec-dir` on real k8s fixtures copied into `root/specs`.
fn generate_from_real_specs(root: &Path) {
    let spec_dir = root.join("specs");
    copy_fixture("k8s", &spec_dir);

    husako_at(root)
        .args(["generate", "--spec-dir", spec_dir.to_str().unwrap()])
        .assert()
        .success();
}

/// Read a generated file relative to the project root.
fn read_generated(root: &Path, rel_path: &str) -> String {
    let path = root.join(rel_path);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

/// Parse `.husako/types/k8s/_schema.json`.
fn parse_schema_json(root: &Path) -> Value {
    let content = read_generated(root, ".husako/types/k8s/_schema.json");
    serde_json::from_str(&content).expect("_schema.json should be valid JSON")
}

// ---------------------------------------------------------------------------
// Mock spec helpers (direct $ref, no allOf wrapping) for per-property tests.
// Real k8s v3 specs wrap $ref in allOf which the emitter currently does not
// resolve for per-property method generation.
// ---------------------------------------------------------------------------

/// Mock apps/v1 spec with direct $ref (enables per-property methods).
fn mock_apps_v1_spec() -> Value {
    serde_json::json!({
        "components": {
            "schemas": {
                "io.k8s.api.apps.v1.Deployment": {
                    "description": "Deployment enables declarative updates.",
                    "properties": {
                        "apiVersion": {"type": "string"},
                        "kind": {"type": "string"},
                        "metadata": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta"},
                        "spec": {"$ref": "#/components/schemas/io.k8s.api.apps.v1.DeploymentSpec"}
                    },
                    "x-kubernetes-group-version-kind": [
                        {"group": "apps", "version": "v1", "kind": "Deployment"}
                    ]
                },
                "io.k8s.api.apps.v1.DeploymentSpec": {
                    "properties": {
                        "replicas": {"type": "integer", "description": "Number of desired pods."},
                        "selector": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelector"},
                        "strategy": {"$ref": "#/components/schemas/io.k8s.api.apps.v1.DeploymentStrategy"},
                        "template": {"$ref": "#/components/schemas/io.k8s.api.core.v1.PodTemplateSpec"}
                    },
                    "required": ["selector"]
                },
                "io.k8s.api.apps.v1.DeploymentStrategy": {
                    "properties": {
                        "type": {"type": "string", "enum": ["Recreate", "RollingUpdate"]}
                    }
                },
                "io.k8s.api.apps.v1.StatefulSet": {
                    "description": "StatefulSet represents a set of pods.",
                    "properties": {
                        "apiVersion": {"type": "string"},
                        "kind": {"type": "string"},
                        "metadata": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta"},
                        "spec": {"$ref": "#/components/schemas/io.k8s.api.apps.v1.StatefulSetSpec"}
                    },
                    "x-kubernetes-group-version-kind": [
                        {"group": "apps", "version": "v1", "kind": "StatefulSet"}
                    ]
                },
                "io.k8s.api.apps.v1.StatefulSetSpec": {
                    "properties": {
                        "replicas": {"type": "integer"},
                        "selector": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelector"},
                        "template": {"$ref": "#/components/schemas/io.k8s.api.core.v1.PodTemplateSpec"},
                        "serviceName": {"type": "string"}
                    },
                    "required": ["selector", "serviceName"]
                },
                "io.k8s.api.apps.v1.DaemonSet": {
                    "description": "DaemonSet represents a daemon set.",
                    "properties": {
                        "apiVersion": {"type": "string"},
                        "kind": {"type": "string"},
                        "metadata": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta"},
                        "spec": {"$ref": "#/components/schemas/io.k8s.api.apps.v1.DaemonSetSpec"}
                    },
                    "x-kubernetes-group-version-kind": [
                        {"group": "apps", "version": "v1", "kind": "DaemonSet"}
                    ]
                },
                "io.k8s.api.apps.v1.DaemonSetSpec": {
                    "properties": {
                        "selector": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelector"},
                        "template": {"$ref": "#/components/schemas/io.k8s.api.core.v1.PodTemplateSpec"}
                    },
                    "required": ["selector"]
                },
                "io.k8s.api.apps.v1.ReplicaSet": {
                    "description": "ReplicaSet ensures availability of pods.",
                    "properties": {
                        "apiVersion": {"type": "string"},
                        "kind": {"type": "string"},
                        "metadata": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta"},
                        "spec": {"$ref": "#/components/schemas/io.k8s.api.apps.v1.ReplicaSetSpec"}
                    },
                    "x-kubernetes-group-version-kind": [
                        {"group": "apps", "version": "v1", "kind": "ReplicaSet"}
                    ]
                },
                "io.k8s.api.apps.v1.ReplicaSetSpec": {
                    "properties": {
                        "replicas": {"type": "integer"},
                        "selector": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelector"}
                    },
                    "required": ["selector"]
                },
                "io.k8s.api.core.v1.PodTemplateSpec": {
                    "properties": {
                        "spec": {"$ref": "#/components/schemas/io.k8s.api.core.v1.PodSpec"}
                    }
                },
                "io.k8s.api.core.v1.PodSpec": {
                    "properties": {
                        "containers": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/io.k8s.api.core.v1.Container"}
                        },
                        "initContainers": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/io.k8s.api.core.v1.Container"}
                        }
                    }
                },
                "io.k8s.api.core.v1.Container": {
                    "properties": {
                        "name": {"type": "string"},
                        "image": {"type": "string"},
                        "resources": {"$ref": "#/components/schemas/io.k8s.api.core.v1.ResourceRequirements"}
                    }
                },
                "io.k8s.api.core.v1.ResourceRequirements": {
                    "properties": {
                        "limits": {"type": "object", "additionalProperties": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.api.resource.Quantity"}},
                        "requests": {"type": "object", "additionalProperties": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.api.resource.Quantity"}}
                    }
                },
                "io.k8s.apimachinery.pkg.api.resource.Quantity": {
                    "description": "Quantity is a representation of a decimal number.",
                    "type": "string"
                },
                "io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta": {
                    "description": "Standard object metadata.",
                    "properties": {
                        "name": {"type": "string"},
                        "namespace": {"type": "string"},
                        "labels": {"type": "object", "additionalProperties": {"type": "string"}}
                    }
                },
                "io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelector": {
                    "properties": {
                        "matchLabels": {"type": "object", "additionalProperties": {"type": "string"}},
                        "matchExpressions": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelectorRequirement"}
                        }
                    }
                },
                "io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelectorRequirement": {
                    "properties": {
                        "key": {"type": "string"},
                        "operator": {"type": "string"},
                        "values": {"type": "array", "items": {"type": "string"}}
                    },
                    "required": ["key", "operator"]
                }
            }
        }
    })
}

/// Mock core/v1 spec with Service, Namespace, and ConfigMap.
fn mock_core_v1_spec() -> Value {
    serde_json::json!({
        "components": {
            "schemas": {
                "io.k8s.api.core.v1.Service": {
                    "description": "Service is a named abstraction of a network service.",
                    "properties": {
                        "apiVersion": {"type": "string"},
                        "kind": {"type": "string"},
                        "metadata": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta"},
                        "spec": {"$ref": "#/components/schemas/io.k8s.api.core.v1.ServiceSpec"}
                    },
                    "x-kubernetes-group-version-kind": [
                        {"group": "", "version": "v1", "kind": "Service"}
                    ]
                },
                "io.k8s.api.core.v1.ServiceSpec": {
                    "properties": {
                        "ports": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/io.k8s.api.core.v1.ServicePort"}
                        },
                        "selector": {"type": "object", "additionalProperties": {"type": "string"}},
                        "type": {"type": "string"}
                    }
                },
                "io.k8s.api.core.v1.ServicePort": {
                    "properties": {
                        "port": {"type": "integer"},
                        "targetPort": {"type": "integer"},
                        "protocol": {"type": "string"}
                    },
                    "required": ["port"]
                },
                "io.k8s.api.core.v1.Namespace": {
                    "description": "Namespace provides a scope for Names.",
                    "properties": {
                        "apiVersion": {"type": "string"},
                        "kind": {"type": "string"},
                        "metadata": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta"}
                    },
                    "x-kubernetes-group-version-kind": [
                        {"group": "", "version": "v1", "kind": "Namespace"}
                    ]
                },
                "io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta": {
                    "description": "Standard object metadata.",
                    "properties": {
                        "name": {"type": "string"},
                        "namespace": {"type": "string"},
                        "labels": {"type": "object", "additionalProperties": {"type": "string"}}
                    }
                }
            }
        }
    })
}

/// Write a mock spec to a spec directory at the given discovery path.
fn write_mock_spec_json(dir: &Path, group_path: &str, spec: &Value) {
    let spec_path = dir.join(format!("{group_path}.json"));
    std::fs::create_dir_all(spec_path.parent().unwrap()).unwrap();
    std::fs::write(spec_path, spec.to_string()).unwrap();
}

/// Generate types from mock apps/v1 + core/v1 specs.
/// Returns the project root (tempdir kept alive via the returned TempDir).
fn generate_from_mock_specs() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    let spec_dir = root.join("specs");
    write_mock_spec_json(&spec_dir, "apis/apps/v1", &mock_apps_v1_spec());
    write_mock_spec_json(&spec_dir, "api/v1", &mock_core_v1_spec());

    husako_at(root)
        .args(["generate", "--spec-dir", spec_dir.to_str().unwrap()])
        .assert()
        .success();

    dir
}

// ===================================================================
// Phase 1 — Critical: silent runtime breakage detection
// ===================================================================

#[test]
fn gvk_index_contains_expected_resources() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    generate_from_real_specs(root);

    let schema = parse_schema_json(root);
    let gvk_index = schema["gvk_index"].as_object().expect("gvk_index should be an object");

    // Core resources (api/v1)
    for kind in ["Pod", "Service", "ConfigMap", "Secret", "Namespace", "ServiceAccount"] {
        let key = format!("v1:{kind}");
        assert!(
            gvk_index.contains_key(&key),
            "{key} missing from gvk_index"
        );
    }

    // Apps resources
    for kind in ["Deployment", "StatefulSet", "DaemonSet", "ReplicaSet"] {
        let key = format!("apps/v1:{kind}");
        assert!(
            gvk_index.contains_key(&key),
            "{key} missing from gvk_index"
        );
    }

    // Batch resources
    for kind in ["Job", "CronJob"] {
        let key = format!("batch/v1:{kind}");
        assert!(
            gvk_index.contains_key(&key),
            "{key} missing from gvk_index"
        );
    }

    // Networking resources
    for kind in ["Ingress", "NetworkPolicy"] {
        let key = format!("networking.k8s.io/v1:{kind}");
        assert!(
            gvk_index.contains_key(&key),
            "{key} missing from gvk_index"
        );
    }

    // No GVK should start with "/" (core group formatting bug)
    for key in gvk_index.keys() {
        assert!(
            !key.starts_with('/'),
            "GVK key '{key}' should not start with '/'"
        );
    }
}

#[test]
fn gvk_index_values_resolve_to_schemas() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    generate_from_real_specs(root);

    let schema = parse_schema_json(root);
    let gvk_index = schema["gvk_index"].as_object().unwrap();
    let schemas = schema["schemas"].as_object().unwrap();

    for (gvk_key, schema_name_val) in gvk_index {
        let schema_name = schema_name_val
            .as_str()
            .unwrap_or_else(|| panic!("gvk_index[{gvk_key}] should be a string"));
        assert!(
            schemas.contains_key(schema_name),
            "gvk_index[{gvk_key}] references '{schema_name}' which does not exist in schemas"
        );
    }
}

#[test]
fn js_deployment_per_property_methods_render() {
    let dir = generate_from_mock_specs();
    let root = dir.path();

    // Verify generated methods exist in JS
    let apps_js = read_generated(root, ".husako/types/k8s/apps/v1.js");
    assert!(
        apps_js.contains("_setSpec(\"replicas\""),
        "replicas _setSpec missing in apps/v1.js"
    );
    assert!(
        apps_js.contains("_setDeep(\"template.spec.containers\""),
        "containers _setDeep missing in apps/v1.js"
    );

    // Write TS using per-property builder methods
    let entry = root.join("deploy.ts");
    std::fs::write(
        &entry,
        r#"
import { build, name } from "husako";
import { Deployment } from "k8s/apps/v1";

const d = Deployment()
    .metadata(name("nginx"))
    .replicas(3)
    .selector({ matchLabels: { app: "nginx" } })
    .containers([{ name: "nginx", image: "nginx:1.27" }]);

build([d]);
"#,
    )
    .unwrap();

    // Render and verify YAML structure
    let output = husako_at(root)
        .args(["render", entry.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success(), "render failed: {:?}", output);
    let yaml = String::from_utf8(output.stdout).unwrap();

    assert!(yaml.contains("apiVersion: apps/v1"));
    assert!(yaml.contains("kind: Deployment"));
    assert!(yaml.contains("name: nginx"));
    // replicas via _setSpec should appear under spec
    assert!(yaml.contains("replicas: 3"));
    // containers via _setDeep should appear under spec.template.spec
    assert!(yaml.contains("image: nginx:1.27"));
}

#[test]
fn k8s_and_crd_generate_together() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Copy both k8s and cnpg fixtures into one spec dir
    let spec_dir = root.join("specs");
    copy_fixture("k8s", &spec_dir);
    copy_fixture("crds/cnpg", &spec_dir);

    husako_at(root)
        .args(["generate", "--spec-dir", spec_dir.to_str().unwrap()])
        .assert()
        .success();

    // Both standard k8s and CRD files should be generated
    assert!(root.join(".husako/types/k8s/apps/v1.js").exists());
    assert!(
        root.join(".husako/types/k8s/postgresql.cnpg.io/v1.js")
            .exists()
    );

    // _schema.json should contain GVK entries for both
    let schema = parse_schema_json(root);
    let gvk_index = schema["gvk_index"].as_object().unwrap();
    assert!(
        gvk_index.contains_key("apps/v1:Deployment"),
        "k8s Deployment missing from combined gvk_index"
    );
    assert!(
        gvk_index.contains_key("postgresql.cnpg.io/v1:Cluster"),
        "cnpg Cluster missing from combined gvk_index"
    );

    // Write TS importing from both k8s and CRD, then render
    let entry = root.join("combined.ts");
    std::fs::write(
        &entry,
        r#"
import { build, name } from "husako";
import { Deployment } from "k8s/apps/v1";
import { Cluster } from "k8s/postgresql.cnpg.io/v1";

const d = Deployment()
    .metadata(name("nginx"))
    .spec({
        replicas: 2,
        selector: { matchLabels: { app: "nginx" } },
        template: { spec: { containers: [{ name: "nginx", image: "nginx:1.27" }] } }
    });

const pg = Cluster()
    .metadata(name("my-pg"))
    .spec({ instances: 3, storage: { size: "10Gi" } });

build([d, pg]);
"#,
    )
    .unwrap();

    let output = husako_at(root)
        .args(["render", entry.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    let yaml = String::from_utf8(output.stdout).unwrap();

    assert!(yaml.contains("kind: Deployment"));
    assert!(yaml.contains("kind: Cluster"));
    assert!(yaml.contains("name: nginx"));
    assert!(yaml.contains("name: my-pg"));
}

// ===================================================================
// Phase 2 — Structural correctness
// ===================================================================

#[test]
fn schema_refs_are_consistent() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    generate_from_real_specs(root);

    let schema = parse_schema_json(root);
    let schemas = schema["schemas"].as_object().unwrap();

    // Collect all $ref values recursively
    let mut refs = Vec::new();
    for (_name, schema_val) in schemas {
        collect_refs(schema_val, &mut refs);
    }

    // Every $ref target should exist as a schema key
    for ref_target in &refs {
        assert!(
            schemas.contains_key(ref_target.as_str()),
            "$ref target '{ref_target}' does not exist in schemas"
        );
    }
}

/// Recursively collect all `$ref` string values from a JSON tree.
fn collect_refs(value: &Value, refs: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(r)) = map.get("$ref") {
                refs.push(r.clone());
            }
            for v in map.values() {
                collect_refs(v, refs);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_refs(v, refs);
            }
        }
        _ => {}
    }
}

#[test]
fn gvk_index_crd_resources() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    let spec_dir = root.join("specs");
    copy_fixture("crds/cnpg", &spec_dir);

    husako_at(root)
        .args(["generate", "--spec-dir", spec_dir.to_str().unwrap()])
        .assert()
        .success();

    let schema = parse_schema_json(root);
    let gvk_index = schema["gvk_index"].as_object().unwrap();
    let schemas = schema["schemas"].as_object().unwrap();

    let key = "postgresql.cnpg.io/v1:Cluster";
    assert!(
        gvk_index.contains_key(key),
        "{key} missing from CRD gvk_index"
    );

    // Referenced schema should exist
    let schema_name = gvk_index[key].as_str().unwrap();
    assert!(
        schemas.contains_key(schema_name),
        "CRD gvk_index[{key}] references '{schema_name}' which is not in schemas"
    );
}

#[test]
fn dts_apps_v1_structure() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    generate_from_real_specs(root);

    let apps_dts = read_generated(root, ".husako/types/k8s/apps/v1.d.ts");

    // Must import from husako/_base
    assert!(
        apps_dts.contains("from \"husako/_base\""),
        "apps/v1.d.ts should import from husako/_base"
    );

    // Must have builder interface extending _ResourceBuilder
    assert!(
        apps_dts.contains("interface Deployment extends _ResourceBuilder"),
        "Deployment builder interface missing"
    );
    assert!(
        apps_dts.contains("interface StatefulSet extends _ResourceBuilder"),
        "StatefulSet builder interface missing"
    );
    assert!(
        apps_dts.contains("interface DaemonSet extends _ResourceBuilder"),
        "DaemonSet builder interface missing"
    );

    // Must have factory function declarations
    assert!(
        apps_dts.contains("export function Deployment(): Deployment"),
        "Deployment() factory missing"
    );
    assert!(
        apps_dts.contains("export function StatefulSet(): StatefulSet"),
        "StatefulSet() factory missing"
    );

    // Data interfaces should exist (separate from builder interfaces)
    assert!(
        apps_dts.contains("interface DeploymentSpec"),
        "DeploymentSpec data interface missing"
    );
}

#[test]
fn dts_common_has_essential_types() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    generate_from_real_specs(root);

    let common_dts = read_generated(root, ".husako/types/k8s/_common.d.ts");

    // Common types should include ObjectMeta and LabelSelector
    assert!(
        common_dts.contains("ObjectMeta"),
        "ObjectMeta missing from _common.d.ts"
    );
    assert!(
        common_dts.contains("LabelSelector"),
        "LabelSelector missing from _common.d.ts"
    );

    // Common file should NOT contain resource types
    assert!(
        !common_dts.contains("interface Deployment"),
        "_common.d.ts should not contain Deployment"
    );
    assert!(
        !common_dts.contains("interface Pod"),
        "_common.d.ts should not contain Pod"
    );
}

#[test]
fn js_service_per_property_methods_render() {
    let dir = generate_from_mock_specs();
    let root = dir.path();

    // Verify Service methods in generated JS
    let core_js = read_generated(root, ".husako/types/k8s/core/v1.js");
    assert!(
        core_js.contains("class _Service"),
        "Service class missing from core/v1.js"
    );
    assert!(
        core_js.contains("_setSpec(\"ports\""),
        "ports _setSpec missing"
    );
    assert!(
        core_js.contains("_setSpec(\"type\""),
        "type _setSpec missing"
    );

    // Write TS using Service builder methods
    let entry = root.join("svc.ts");
    std::fs::write(
        &entry,
        r#"
import { build, name } from "husako";
import { Service } from "k8s/core/v1";

const svc = Service()
    .metadata(name("my-svc"))
    .selector({ app: "web" })
    .ports([{ port: 80, targetPort: 8080 }])
    .type("ClusterIP");

build([svc]);
"#,
    )
    .unwrap();

    let output = husako_at(root)
        .args(["render", entry.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success(), "render failed: {:?}", output);
    let yaml = String::from_utf8(output.stdout).unwrap();

    assert!(yaml.contains("kind: Service"));
    assert!(yaml.contains("name: my-svc"));
    assert!(yaml.contains("app: web"));
    assert!(yaml.contains("port: 80"));
    assert!(yaml.contains("targetPort: 8080"));
    assert!(yaml.contains("type: ClusterIP"));
}

// ===================================================================
// Phase 3 — Completeness
// ===================================================================

#[test]
fn dts_imports_are_resolvable() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    generate_from_real_specs(root);

    let common_dts = read_generated(root, ".husako/types/k8s/_common.d.ts");

    // Check all group-version .d.ts files
    let k8s_dir = root.join(".husako/types/k8s");
    for entry in walkdir(&k8s_dir) {
        let path = entry;
        if path.extension().is_some_and(|ext| ext == "dts")
            || path
                .to_str()
                .is_some_and(|s| s.ends_with(".d.ts"))
        {
            let content = std::fs::read_to_string(&path).unwrap();

            // Extract imports from "k8s/_common"
            for line in content.lines() {
                if line.contains("from \"k8s/_common\"") {
                    // Extract names between { and }
                    if let Some(start) = line.find('{')
                        && let Some(end) = line.find('}')
                    {
                        let names_str = &line[start + 1..end];
                        for name in names_str.split(',') {
                            let name = name.trim();
                            if !name.is_empty() {
                                assert!(
                                    common_dts.contains(&format!("interface {name}"))
                                        || common_dts.contains(&format!("class {name}"))
                                        || common_dts.contains(&format!("type {name}")),
                                    "'{name}' imported from k8s/_common in {} but not defined in _common.d.ts",
                                    path.display()
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Walk directory tree and collect file paths.
fn walkdir(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(walkdir(&path));
            } else {
                files.push(path);
            }
        }
    }
    files
}

#[test]
fn tsconfig_paths_point_to_generated_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    generate_from_real_specs(root);

    let tsconfig_content = read_generated(root, "tsconfig.json");
    let tsconfig: Value =
        serde_json::from_str(&tsconfig_content).expect("tsconfig.json should be valid JSON");

    let paths = &tsconfig["compilerOptions"]["paths"];

    // Check husako path
    let husako_path = paths["husako"][0]
        .as_str()
        .expect("husako path should exist");
    assert_eq!(husako_path, ".husako/types/husako.d.ts");

    // Check husako/_base path
    let base_path = paths["husako/_base"][0]
        .as_str()
        .expect("husako/_base path should exist");
    assert_eq!(base_path, ".husako/types/husako/_base.d.ts");

    // Check k8s/* wildcard path
    let k8s_path = paths["k8s/*"][0]
        .as_str()
        .expect("k8s/* path should exist");
    assert_eq!(k8s_path, ".husako/types/k8s/*");

    // Verify that the referenced files actually exist on disk
    let base_url = tsconfig["compilerOptions"]["baseUrl"]
        .as_str()
        .unwrap_or(".");
    let base_dir = root.join(base_url);

    assert!(
        base_dir.join(husako_path).exists(),
        "husako.d.ts path does not exist on disk"
    );
    assert!(
        base_dir.join(base_path).exists(),
        "husako/_base.d.ts path does not exist on disk"
    );
    // k8s/* is a wildcard — check that the k8s directory exists
    assert!(
        base_dir.join(".husako/types/k8s").exists(),
        "k8s types directory does not exist on disk"
    );
}

#[test]
fn js_setspec_paths_match_openapi_properties() {
    let dir = generate_from_mock_specs();
    let root = dir.path();

    let apps_js = read_generated(root, ".husako/types/k8s/apps/v1.js");

    // Extract _setSpec("xxx" patterns for the Deployment class block
    let deploy_methods = extract_setspec_methods(&apps_js, "_Deployment");

    // These are the properties from mock_apps_v1_spec's DeploymentSpec
    // (minus RESOURCE_SPEC_SKIP: status, apiVersion, kind, metadata)
    let expected_spec_props = ["replicas", "selector", "strategy", "template"];

    for prop in &expected_spec_props {
        assert!(
            deploy_methods.contains(&prop.to_string()),
            "_setSpec(\"{prop}\") missing from Deployment in apps/v1.js. Found: {deploy_methods:?}"
        );
    }

    // All extracted methods should be a subset of the spec properties
    for method in &deploy_methods {
        assert!(
            expected_spec_props.contains(&method.as_str()),
            "Unexpected _setSpec(\"{method}\") in Deployment"
        );
    }
}

/// Extract `_setSpec("xxx"` method names from a class block in generated JS.
fn extract_setspec_methods(js: &str, class_name: &str) -> Vec<String> {
    let mut methods = Vec::new();
    let class_marker = format!("class {class_name}");
    let mut in_class = false;

    for line in js.lines() {
        if line.contains(&class_marker) {
            in_class = true;
            continue;
        }
        if in_class {
            if line.starts_with('}') || line.starts_with("export ") {
                break;
            }
            if let Some(start) = line.find("_setSpec(\"") {
                let rest = &line[start + "_setSpec(\"".len()..];
                if let Some(end) = rest.find('"') {
                    methods.push(rest[..end].to_string());
                }
            }
        }
    }
    methods
}

#[test]
fn js_setdeep_only_on_workloads() {
    let dir = generate_from_mock_specs();
    let root = dir.path();

    let apps_js = read_generated(root, ".husako/types/k8s/apps/v1.js");

    // Workload resources (those whose spec has template → PodTemplateSpec)
    // should have _setDeep for containers and initContainers
    for class in ["_Deployment", "_StatefulSet", "_DaemonSet"] {
        let block = extract_class_block(&apps_js, class);
        assert!(
            block.contains("_setDeep(\"template.spec.containers\""),
            "{class} should have containers _setDeep. Block:\n{block}"
        );
        assert!(
            block.contains("_setDeep(\"template.spec.initContainers\""),
            "{class} should have initContainers _setDeep. Block:\n{block}"
        );
    }

    // ReplicaSet spec does NOT have template → PodTemplateSpec
    // (in our mock, ReplicaSetSpec only has replicas and selector)
    let rs_block = extract_class_block(&apps_js, "_ReplicaSet");
    assert!(
        !rs_block.contains("_setDeep"),
        "ReplicaSet should NOT have _setDeep methods. Block:\n{rs_block}"
    );
}

/// Extract a class block from generated JS (from "class Name" to the next "export" or EOF).
fn extract_class_block(js: &str, class_name: &str) -> String {
    let class_marker = format!("class {class_name}");
    let mut lines = Vec::new();
    let mut in_class = false;

    for line in js.lines() {
        if line.contains(&class_marker) {
            in_class = true;
            lines.push(line);
            continue;
        }
        if in_class {
            if line.starts_with("export ") {
                break;
            }
            lines.push(line);
            if line == "}" {
                break;
            }
        }
    }
    lines.join("\n")
}

#[test]
fn js_common_schema_builders_render() {
    let dir = generate_from_mock_specs();
    let root = dir.path();

    // _common.js should be generated since LabelSelector has Array(Ref) matchExpressions
    let common_js = read_generated(root, ".husako/types/k8s/_common.js");
    assert!(
        common_js.contains("class LabelSelector"),
        "LabelSelector _SchemaBuilder missing from _common.js"
    );

    // Write TS that uses a _SchemaBuilder from _common
    let entry = root.join("common_builder.ts");
    std::fs::write(
        &entry,
        r#"
import { build, name } from "husako";
import { Deployment } from "k8s/apps/v1";
import { labelSelector } from "k8s/_common";

const d = Deployment()
    .metadata(name("nginx"))
    .selector(labelSelector().matchLabels({ app: "nginx" }));

build([d]);
"#,
    )
    .unwrap();

    let output = husako_at(root)
        .args(["render", entry.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success(), "render failed: {:?}", output);
    let yaml = String::from_utf8(output.stdout).unwrap();

    // The labelSelector builder output should be flattened via _resolveFragments
    assert!(yaml.contains("app: nginx"));
}
