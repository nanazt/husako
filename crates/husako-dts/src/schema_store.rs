use std::collections::HashMap;

use serde_json::Value;

const QUANTITY_SCHEMA_NAME: &str = "io.k8s.apimachinery.pkg.api.resource.Quantity";
const REF_PREFIX: &str = "#/components/schemas/";

/// Generate `_schema.json` content from raw OpenAPI spec JSON values.
///
/// Merges all `components.schemas` from all specs, simplifies `$ref` values,
/// annotates the Quantity schema with `"format": "quantity"`, and builds a
/// GVK index from `x-kubernetes-group-version-kind` annotations.
pub fn generate_schema_store(specs: &HashMap<String, Value>) -> Value {
    let mut schemas = serde_json::Map::new();
    let mut gvk_index = serde_json::Map::new();

    // 1. Collect all schemas from all specs
    for spec in specs.values() {
        if let Some(spec_schemas) = spec
            .get("components")
            .and_then(|c| c.get("schemas"))
            .and_then(Value::as_object)
        {
            for (name, schema) in spec_schemas {
                let mut schema = schema.clone();
                simplify_refs(&mut schema);

                // Build GVK index entries
                if let Some(gvk_arr) = schema
                    .get("x-kubernetes-group-version-kind")
                    .and_then(Value::as_array)
                {
                    for gvk in gvk_arr {
                        let group = gvk.get("group").and_then(Value::as_str).unwrap_or("");
                        let version = gvk.get("version").and_then(Value::as_str).unwrap_or("");
                        let kind = gvk.get("kind").and_then(Value::as_str).unwrap_or("");

                        let key = if group.is_empty() {
                            format!("{version}:{kind}")
                        } else {
                            format!("{group}/{version}:{kind}")
                        };

                        gvk_index.insert(key, Value::String(name.clone()));
                    }
                }

                schemas.insert(name.clone(), schema);
            }
        }
    }

    // 2. Annotate Quantity schema with format: "quantity"
    if let Some(quantity) = schemas.get_mut(QUANTITY_SCHEMA_NAME)
        && let Some(obj) = quantity.as_object_mut()
    {
        obj.insert("format".to_string(), Value::String("quantity".to_string()));
    }

    // 3. Sort keys for deterministic output
    let sorted_gvk: serde_json::Map<String, Value> = {
        let mut keys: Vec<_> = gvk_index.keys().cloned().collect();
        keys.sort();
        keys.into_iter()
            .map(|k| {
                let v = gvk_index.remove(&k).unwrap();
                (k, v)
            })
            .collect()
    };

    let sorted_schemas: serde_json::Map<String, Value> = {
        let mut keys: Vec<_> = schemas.keys().cloned().collect();
        keys.sort();
        keys.into_iter()
            .map(|k| {
                let v = schemas.remove(&k).unwrap();
                (k, v)
            })
            .collect()
    };

    serde_json::json!({
        "version": 2,
        "gvk_index": sorted_gvk,
        "schemas": sorted_schemas
    })
}

/// Recursively strip `#/components/schemas/` prefix from all `$ref` values.
fn simplify_refs(value: &mut Value) {
    match value {
        Value::Object(obj) => {
            if let Some(Value::String(ref_str)) = obj.get_mut("$ref")
                && let Some(stripped) = ref_str.strip_prefix(REF_PREFIX)
            {
                *ref_str = stripped.to_string();
            }
            for v in obj.values_mut() {
                simplify_refs(v);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                simplify_refs(v);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn deployment_spec() -> Value {
        json!({
            "components": {
                "schemas": {
                    "io.k8s.api.apps.v1.Deployment": {
                        "description": "Deployment",
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
                            "selector": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelector"},
                            "template": {"$ref": "#/components/schemas/io.k8s.api.core.v1.PodTemplateSpec"}
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
                            }
                        }
                    },
                    "io.k8s.api.core.v1.Container": {
                        "properties": {
                            "name": {"type": "string"},
                            "imagePullPolicy": {
                                "type": "string",
                                "enum": ["Always", "IfNotPresent", "Never"]
                            },
                            "resources": {"$ref": "#/components/schemas/io.k8s.api.core.v1.ResourceRequirements"}
                        }
                    },
                    "io.k8s.api.core.v1.ResourceRequirements": {
                        "properties": {
                            "limits": {
                                "type": "object",
                                "additionalProperties": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.api.resource.Quantity"}
                            },
                            "requests": {
                                "type": "object",
                                "additionalProperties": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.api.resource.Quantity"}
                            }
                        }
                    },
                    "io.k8s.apimachinery.pkg.api.resource.Quantity": {
                        "description": "Quantity is a representation of a decimal number.",
                        "type": "string"
                    },
                    "io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta": {
                        "properties": {
                            "name": {"type": "string"},
                            "namespace": {"type": "string"}
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

    fn namespace_spec() -> Value {
        json!({
            "components": {
                "schemas": {
                    "io.k8s.api.core.v1.Namespace": {
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
                        "properties": {
                            "name": {"type": "string"}
                        }
                    }
                }
            }
        })
    }

    #[test]
    fn simplifies_refs() {
        let specs = HashMap::from([("apis/apps/v1".to_string(), deployment_spec())]);
        let result = generate_schema_store(&specs);

        let deploy = &result["schemas"]["io.k8s.api.apps.v1.Deployment"];
        let spec_ref = deploy["properties"]["spec"]["$ref"].as_str().unwrap();
        assert_eq!(spec_ref, "io.k8s.api.apps.v1.DeploymentSpec");

        // No more #/components/schemas/ prefix
        assert!(
            !serde_json::to_string(&result)
                .unwrap()
                .contains("#/components/schemas/")
        );
    }

    #[test]
    fn annotates_quantity_format() {
        let specs = HashMap::from([("apis/apps/v1".to_string(), deployment_spec())]);
        let result = generate_schema_store(&specs);

        let quantity = &result["schemas"]["io.k8s.apimachinery.pkg.api.resource.Quantity"];
        assert_eq!(quantity["format"], "quantity");
        assert_eq!(quantity["type"], "string");
    }

    #[test]
    fn builds_gvk_index() {
        let specs = HashMap::from([
            ("apis/apps/v1".to_string(), deployment_spec()),
            ("api/v1".to_string(), namespace_spec()),
        ]);
        let result = generate_schema_store(&specs);

        let gvk_index = result["gvk_index"].as_object().unwrap();
        assert_eq!(
            gvk_index["apps/v1:Deployment"],
            "io.k8s.api.apps.v1.Deployment"
        );
        assert_eq!(gvk_index["v1:Namespace"], "io.k8s.api.core.v1.Namespace");
    }

    #[test]
    fn gvk_index_core_group_no_prefix() {
        let specs = HashMap::from([("api/v1".to_string(), namespace_spec())]);
        let result = generate_schema_store(&specs);

        let gvk_index = result["gvk_index"].as_object().unwrap();
        // Core group (empty) maps to "v1:Kind", not "/v1:Kind"
        assert!(gvk_index.contains_key("v1:Namespace"));
        assert!(!gvk_index.keys().any(|k| k.starts_with('/')));
    }

    #[test]
    fn merges_multiple_specs() {
        let specs = HashMap::from([
            ("apis/apps/v1".to_string(), deployment_spec()),
            ("api/v1".to_string(), namespace_spec()),
        ]);
        let result = generate_schema_store(&specs);

        let schemas = result["schemas"].as_object().unwrap();
        // From deployment_spec
        assert!(schemas.contains_key("io.k8s.api.apps.v1.Deployment"));
        // From namespace_spec
        assert!(schemas.contains_key("io.k8s.api.core.v1.Namespace"));
    }

    #[test]
    fn version_is_2() {
        let specs = HashMap::from([("apis/apps/v1".to_string(), deployment_spec())]);
        let result = generate_schema_store(&specs);
        assert_eq!(result["version"], 2);
    }

    #[test]
    fn preserves_required_and_enum() {
        let specs = HashMap::from([("apis/apps/v1".to_string(), deployment_spec())]);
        let result = generate_schema_store(&specs);

        let deploy_spec = &result["schemas"]["io.k8s.api.apps.v1.DeploymentSpec"];
        let required = deploy_spec["required"].as_array().unwrap();
        assert!(required.contains(&Value::String("selector".to_string())));

        let container = &result["schemas"]["io.k8s.api.core.v1.Container"];
        let pull_policy = &container["properties"]["imagePullPolicy"];
        let enum_vals = pull_policy["enum"].as_array().unwrap();
        assert_eq!(enum_vals.len(), 3);
    }

    #[test]
    fn snapshot_schema_store() {
        let specs = HashMap::from([("apis/apps/v1".to_string(), deployment_spec())]);
        let result = generate_schema_store(&specs);
        let formatted = serde_json::to_string_pretty(&result).unwrap();
        insta::assert_snapshot!("schema_store_json", formatted);
    }
}
