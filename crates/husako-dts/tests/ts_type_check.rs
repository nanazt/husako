//! TypeScript type-check regression test.
//!
//! Generates `.d.ts` files from a mock OpenAPI spec, writes them to a temp
//! directory alongside minimal `husako` base type stubs, then invokes
//! `tsc --noEmit` to verify that the four patterns from the SDK template
//! compile without TypeScript errors.
//!
//! **Prerequisite**: `tsc` must be on PATH (install with `npm install -g typescript`).
//! The test fails — not skips — when `tsc` is absent, so CI can detect a missing
//! Node.js setup early rather than silently skipping type coverage.

use std::collections::HashMap;
use std::path::Path;

use husako_dts::{GenerateOptions, GenerateResult};

fn generate(specs: HashMap<String, serde_json::Value>) -> GenerateResult {
    husako_dts::generate(&GenerateOptions { specs }).expect("generate should succeed")
}

/// Build a mock OpenAPI spec that covers all four error patterns from the
/// SDK template:
///
/// 1. `Deployment().metadata(...)` — was broken by raw Deployment interface
/// 2. `LabelSelector().matchLabels(...)` — was broken by raw LabelSelector interface
/// 3. `Container().name(...)` — was broken by raw Container interface
/// 4. `Container().resources(requests(...).limits(...))` — type mismatch
fn mock_spec() -> serde_json::Value {
    serde_json::json!({
        "components": {
            "schemas": {
                // GVK resource — gets builder only (no raw interface)
                "io.k8s.api.apps.v1.Deployment": {
                    "description": "A Deployment.",
                    "properties": {
                        "apiVersion": {"type": "string"},
                        "kind": {"type": "string"},
                        "metadata": {
                            "$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta"
                        },
                        "spec": {
                            "$ref": "#/components/schemas/io.k8s.api.apps.v1.DeploymentSpec"
                        }
                    },
                    "x-kubernetes-group-version-kind": [
                        {"group": "apps", "version": "v1", "kind": "Deployment"}
                    ]
                },
                // Non-GVK with Ref property → builder schema (no raw interface)
                "io.k8s.api.apps.v1.DeploymentSpec": {
                    "properties": {
                        "replicas": {"type": "integer"},
                        "selector": {
                            "$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelector"
                        }
                    },
                    "required": ["selector"]
                },
                // Common schema — no Ref properties → raw interface only
                "io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta": {
                    "properties": {
                        "name": {"type": "string"},
                        "namespace": {"type": "string"}
                    }
                },
                // Common schema — has Array(Ref) → builder schema (no raw interface)
                "io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelector": {
                    "properties": {
                        "matchLabels": {
                            "type": "object",
                            "additionalProperties": {"type": "string"}
                        },
                        "matchExpressions": {
                            "type": "array",
                            "items": {
                                "$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelectorRequirement"
                            }
                        }
                    }
                },
                // Common schema — simple types only → raw interface only
                "io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelectorRequirement": {
                    "properties": {
                        "key": {"type": "string"},
                        "operator": {"type": "string"}
                    },
                    "required": ["key", "operator"]
                },
                // Non-GVK core/v1 with resources: ResourceRequirements → builder + union type
                "io.k8s.api.core.v1.Container": {
                    "properties": {
                        "name": {"type": "string"},
                        "image": {"type": "string"},
                        "resources": {
                            "$ref": "#/components/schemas/io.k8s.api.core.v1.ResourceRequirements"
                        }
                    },
                    "required": ["name"]
                },
                // Non-GVK core/v1 — simple properties only → raw interface only
                "io.k8s.api.core.v1.ResourceRequirements": {
                    "properties": {
                        "limits": {
                            "type": "object",
                            "additionalProperties": {"type": "string"}
                        },
                        "requests": {
                            "type": "object",
                            "additionalProperties": {"type": "string"}
                        }
                    }
                }
            }
        }
    })
}

fn write_file(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
}

#[test]
fn generated_types_pass_tsc() {
    let tsc_available = std::process::Command::new("tsc")
        .arg("--version")
        .output()
        .is_ok();
    assert!(
        tsc_available,
        "tsc must be installed to run this test (npm install -g typescript)"
    );

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Generate types from mock spec
    let result = generate(HashMap::from([("apis/apps/v1".to_string(), mock_spec())]));

    // Write generated type files
    for (rel_path, content) in &result.files {
        if rel_path.ends_with(".d.ts") {
            write_file(root, &format!("types/{rel_path}"), content);
        }
    }

    // Write husako SDK type stubs
    write_file(root, "types/husako.d.ts", husako_sdk::HUSAKO_DTS);
    write_file(root, "types/husako/_base.d.ts", husako_sdk::HUSAKO_BASE_DTS);

    // Write tsconfig.json
    write_file(
        root,
        "tsconfig.json",
        r#"{
  "compilerOptions": {
    "baseUrl": ".",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "paths": {
      "husako": ["types/husako.d.ts"],
      "husako/_base": ["types/husako/_base.d.ts"],
      "k8s/*": ["types/k8s/*"]
    }
  },
  "files": ["entry.ts"]
}"#,
    );

    // Write entry.ts with all four previously-broken patterns using the new chain API.
    // Note: `.containers()` shortcut is not used here because the mock DeploymentSpec
    // lacks a `template: PodTemplateSpec` property (the shortcut is generated only when
    // that property is present). All four type errors are covered by the patterns below.
    write_file(
        root,
        "entry.ts",
        r#"import husako from "husako";
import { Deployment } from "k8s/apps/v1";
import { Container, cpu, memory, requests } from "k8s/core/v1";
import { LabelSelector } from "k8s/_common";
import { name, namespace, label } from "k8s/meta/v1";

// Pattern 1: Deployment().metadata() — accepts MetadataChain from chain starters
// Pattern 2: LabelSelector().matchLabels() — builder method
const nginx = Deployment()
  .metadata(name("nginx").namespace("default").label("app", "nginx"))
  .replicas(1)
  .selector(LabelSelector().matchLabels({ app: "nginx" }));

// Pattern 3: Container().name() — builder property method
// Pattern 4: Container().resources(requests(cpu().memory())) — ResourceRequirementsChain
const c = Container()
  .name("nginx")
  .image("nginx:1.25")
  .resources(
    requests(cpu("250m").memory("128Mi")),
  );

husako.build([nginx]);
"#,
    );

    // Run tsc --noEmit from the temp directory
    let output = std::process::Command::new("tsc")
        .arg("--noEmit")
        .current_dir(root)
        .output()
        .expect("failed to run tsc");

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "tsc --noEmit failed with TypeScript errors:\n{stdout}{stderr}\n\
             Generated types:\n{}\n",
            result
                .files
                .iter()
                .filter(|(k, _)| k.ends_with(".d.ts"))
                .map(|(k, v)| format!("--- {k} ---\n{v}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
