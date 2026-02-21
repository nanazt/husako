mod emitter;
mod schema;

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

/// A registered kind that should get a builder class.
pub struct RegisteredKind {
    /// API group (empty string for core).
    pub group: String,
    pub version: String,
    pub kind: String,
}

pub struct GenerateOptions {
    /// Map of discovery path (e.g. "api/v1", "apis/apps/v1") to OpenAPI spec JSON.
    pub specs: HashMap<String, serde_json::Value>,
    /// Kinds that have runtime builders.
    pub registered_kinds: Vec<RegisteredKind>,
}

#[derive(Debug)]
pub struct GenerateResult {
    /// Map of relative file path to file content.
    pub files: HashMap<String, String>,
}

pub fn generate(options: &GenerateOptions) -> Result<GenerateResult, DtsError> {
    let mut files = HashMap::new();

    // Build a lookup of registered kinds: (dts_group, version) -> Set<kind>
    // dts_group maps "" to "core" for matching against schema classification
    let mut registered_by_gv: HashMap<(String, String), HashSet<String>> = HashMap::new();
    for rk in &options.registered_kinds {
        let dts_group = if rk.group.is_empty() {
            "core".to_string()
        } else {
            rk.group.clone()
        };
        registered_by_gv
            .entry((dts_group, rk.version.clone()))
            .or_default()
            .insert(rk.kind.clone());
    }

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

    // Separate common vs group-version schemas
    let common: Vec<&SchemaInfo> = all_schemas
        .iter()
        .filter(|s| s.location == SchemaLocation::Common)
        .collect();

    let common_names: HashSet<String> = common.iter().map(|s| s.ts_name.clone()).collect();

    // Emit _common.d.ts
    if !common.is_empty() {
        files.insert(
            "k8s/_common.d.ts".to_string(),
            emitter::emit_common(&common),
        );
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

    // Emit per-group-version .d.ts files
    for ((group, version), schemas) in &by_gv {
        let registered = registered_by_gv
            .get(&(group.clone(), version.clone()))
            .cloned()
            .unwrap_or_default();

        let content = emitter::emit_group_version(schemas, &registered, &common_names);
        let path = format!("k8s/{group}/{version}.d.ts");
        files.insert(path, content);
    }

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

        let options = GenerateOptions {
            specs,
            registered_kinds: vec![RegisteredKind {
                group: "apps".to_string(),
                version: "v1".to_string(),
                kind: "Deployment".to_string(),
            }],
        };

        let result = generate(&options).unwrap();

        // Should have _common.d.ts and apps/v1.d.ts
        assert!(result.files.contains_key("k8s/_common.d.ts"));
        assert!(result.files.contains_key("k8s/apps/v1.d.ts"));

        // _common should have ObjectMeta and LabelSelector
        let common = &result.files["k8s/_common.d.ts"];
        assert!(common.contains("ObjectMeta"));
        assert!(common.contains("LabelSelector"));

        // apps/v1 should have Deployment builder and DeploymentSpec interface
        let apps = &result.files["k8s/apps/v1.d.ts"];
        assert!(apps.contains("class Deployment"));
        assert!(apps.contains("interface DeploymentSpec"));
        assert!(apps.contains("_ResourceBuilder"));
    }

    #[test]
    fn generate_snapshot() {
        let specs = HashMap::from([("apis/apps/v1".to_string(), mock_apps_v1_spec())]);

        let options = GenerateOptions {
            specs,
            registered_kinds: vec![RegisteredKind {
                group: "apps".to_string(),
                version: "v1".to_string(),
                kind: "Deployment".to_string(),
            }],
        };

        let result = generate(&options).unwrap();

        // Snapshot _common.d.ts
        insta::assert_snapshot!("common_dts", &result.files["k8s/_common.d.ts"]);

        // Snapshot apps/v1.d.ts
        insta::assert_snapshot!("apps_v1_dts", &result.files["k8s/apps/v1.d.ts"]);
    }

    #[test]
    fn generate_empty_specs_errors() {
        let options = GenerateOptions {
            specs: HashMap::from([("api/v1".to_string(), json!({"openapi": "3.0.0"}))]),
            registered_kinds: vec![],
        };

        let err = generate(&options).unwrap_err();
        assert!(matches!(err, DtsError::NoSchemas(_)));
    }
}
