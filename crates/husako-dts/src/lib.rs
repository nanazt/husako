mod emitter;
pub mod json_schema;
mod schema;
pub mod schema_store;

use std::collections::{HashMap, HashSet};

use schema::{SchemaInfo, SchemaLocation};

#[derive(Debug, thiserror::Error)]
pub enum DtsError {
    #[error("schema error: {0}")]
    Schema(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("no schemas found in spec: {0}")]
    NoSchemas(String),
}

pub struct GenerateOptions {
    /// Map of discovery path (e.g. "api/v1", "apis/apps/v1") to OpenAPI spec JSON.
    pub specs: HashMap<String, serde_json::Value>,
}

#[derive(Debug)]
pub struct GenerateResult {
    /// Map of relative file path to file content.
    pub files: HashMap<String, String>,
}

pub fn generate(options: &GenerateOptions) -> Result<GenerateResult, DtsError> {
    let mut files = HashMap::new();

    // Parse all specs into SchemaInfo
    let mut all_schemas: Vec<SchemaInfo> = Vec::new();
    for spec in options.specs.values() {
        all_schemas.extend(schema::parse_spec(spec));
    }

    if all_schemas.is_empty() {
        return Err(DtsError::NoSchemas(
            "no schemas found in any provided spec".to_string(),
        ));
    }

    // CRD reclassification: schemas classified as Other but having GVK
    // should be placed into their proper group-version.
    for schema in &mut all_schemas {
        if schema.location == SchemaLocation::Other
            && let Some(gvk) = &schema.gvk
        {
            let group = if gvk.group.is_empty() {
                "core".to_string()
            } else {
                gvk.group.clone()
            };
            schema.location = SchemaLocation::GroupVersion {
                group,
                version: gvk.version.clone(),
            };
        }
    }

    // Separate common vs group-version schemas
    let common: Vec<&SchemaInfo> = all_schemas
        .iter()
        .filter(|s| s.location == SchemaLocation::Common)
        .collect();

    let common_names: HashSet<String> = common.iter().map(|s| s.ts_name.clone()).collect();

    // Emit _common.d.ts and _common.js
    if !common.is_empty() {
        files.insert(
            "k8s/_common.d.ts".to_string(),
            emitter::emit_common(&common),
        );

        // Emit _common.js if there are schema builders
        let has_common_builders = common.iter().any(|s| emitter::should_generate_builder(s));
        if has_common_builders {
            files.insert(
                "k8s/_common.js".to_string(),
                emitter::emit_common_js(&common),
            );
        }
    }

    // Group schemas by (group, version)
    let mut by_gv: HashMap<(String, String), Vec<&SchemaInfo>> = HashMap::new();
    for schema in &all_schemas {
        if let SchemaLocation::GroupVersion { group, version } = &schema.location {
            by_gv
                .entry((group.clone(), version.clone()))
                .or_default()
                .push(schema);
        }
    }

    // Emit per-group-version .d.ts and .js files
    for ((group, version), schemas) in &by_gv {
        let dts_content = emitter::emit_group_version(schemas, &common_names);
        let dts_path = format!("k8s/{group}/{version}.d.ts");
        files.insert(dts_path, dts_content);

        // Emit .js if there are resource builders (GVK) or schema builders
        let has_js_content = schemas.iter().any(|s| s.gvk.is_some())
            || schemas.iter().any(|s| emitter::should_generate_builder(s));
        if has_js_content {
            let js_content = emitter::emit_group_version_js(schemas);
            let js_path = format!("k8s/{group}/{version}.js");
            files.insert(js_path, js_content);
        }
    }

    // Generate _schema.json
    let schema_store = schema_store::generate_schema_store(&options.specs);
    let schema_json = serde_json::to_string_pretty(&schema_store)
        .map_err(|e| DtsError::Schema(format!("serialize _schema.json: {e}")))?;
    files.insert("k8s/_schema.json".to_string(), schema_json);

    Ok(GenerateResult { files })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn mock_apps_v1_spec() -> serde_json::Value {
        json!({
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
                            "replicas": {"type": "integer"},
                            "selector": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelector"}
                        },
                        "required": ["selector"]
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
                            "matchLabels": {"type": "object", "additionalProperties": {"type": "string"}}
                        }
                    }
                }
            }
        })
    }

    #[test]
    fn generate_produces_files() {
        let specs = HashMap::from([("apis/apps/v1".to_string(), mock_apps_v1_spec())]);

        let options = GenerateOptions { specs };

        let result = generate(&options).unwrap();

        // Should have _common.d.ts, apps/v1.d.ts, and apps/v1.js
        assert!(result.files.contains_key("k8s/_common.d.ts"));
        assert!(result.files.contains_key("k8s/apps/v1.d.ts"));
        assert!(result.files.contains_key("k8s/apps/v1.js"));

        // _common should have ObjectMeta and LabelSelector
        let common = &result.files["k8s/_common.d.ts"];
        assert!(common.contains("ObjectMeta"));
        assert!(common.contains("LabelSelector"));

        // apps/v1.d.ts should have Deployment builder and DeploymentSpec interface
        let apps_dts = &result.files["k8s/apps/v1.d.ts"];
        assert!(apps_dts.contains("class Deployment"));
        assert!(apps_dts.contains("interface DeploymentSpec"));
        assert!(apps_dts.contains("_ResourceBuilder"));

        // apps/v1.js should have Deployment class
        let apps_js = &result.files["k8s/apps/v1.js"];
        assert!(apps_js.contains("class Deployment"));
        assert!(apps_js.contains("_ResourceBuilder"));
        assert!(apps_js.contains("\"apps/v1\""));
    }

    #[test]
    fn generate_snapshot() {
        let specs = HashMap::from([("apis/apps/v1".to_string(), mock_apps_v1_spec())]);

        let options = GenerateOptions { specs };

        let result = generate(&options).unwrap();

        // Snapshot _common.d.ts
        insta::assert_snapshot!("common_dts", &result.files["k8s/_common.d.ts"]);

        // Snapshot apps/v1.d.ts
        insta::assert_snapshot!("apps_v1_dts", &result.files["k8s/apps/v1.d.ts"]);

        // Snapshot apps/v1.js
        insta::assert_snapshot!("apps_v1_js", &result.files["k8s/apps/v1.js"]);
    }

    #[test]
    fn generate_empty_specs_errors() {
        let options = GenerateOptions {
            specs: HashMap::from([("api/v1".to_string(), json!({"openapi": "3.0.0"}))]),
        };

        let err = generate(&options).unwrap_err();
        assert!(matches!(err, DtsError::NoSchemas(_)));
    }

    #[test]
    fn crd_reclassification() {
        let specs = HashMap::from([(
            "apis/postgresql.cnpg.io/v1".to_string(),
            json!({
                "components": {
                    "schemas": {
                        "io.cnpg.postgresql.v1.Cluster": {
                            "description": "Cluster is the PostgreSQL cluster CRD.",
                            "properties": {
                                "apiVersion": {"type": "string"},
                                "kind": {"type": "string"},
                                "spec": {"type": "object"}
                            },
                            "x-kubernetes-group-version-kind": [
                                {"group": "postgresql.cnpg.io", "version": "v1", "kind": "Cluster"}
                            ]
                        }
                    }
                }
            }),
        )]);

        let options = GenerateOptions { specs };
        let result = generate(&options).unwrap();

        // CRD should be reclassified into its GVK group-version
        assert!(result.files.contains_key("k8s/postgresql.cnpg.io/v1.d.ts"));
        assert!(result.files.contains_key("k8s/postgresql.cnpg.io/v1.js"));

        let dts = &result.files["k8s/postgresql.cnpg.io/v1.d.ts"];
        assert!(dts.contains("interface Cluster"));

        let js = &result.files["k8s/postgresql.cnpg.io/v1.js"];
        assert!(js.contains("class _Cluster"));
        assert!(js.contains("\"postgresql.cnpg.io/v1\""));
    }
}
