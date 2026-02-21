use std::collections::{HashMap, HashSet};

use serde_json::Value;

const QUANTITY_REF: &str = "io.k8s.apimachinery.pkg.api.resource.Quantity";

/// Generate `_validation.json` content from raw OpenAPI spec JSON values.
///
/// Algorithm:
/// 1. Collect all schemas from all specs into a single lookup map
/// 2. For each schema with `x-kubernetes-group-version-kind`, DFS the schema graph
/// 3. When a `$ref` points to `Quantity`, record the path
/// 4. Handle arrays (items -> `[*]`), maps (additionalProperties -> `[*]`), nested $refs
/// 5. Cycle detection via backtracking visited set
pub fn generate_validation_map(specs: &HashMap<String, Value>) -> Value {
    // 1. Build a global schema lookup: full_name -> schema JSON
    let mut all_schemas: HashMap<String, &Value> = HashMap::new();
    for spec in specs.values() {
        if let Some(schemas) = spec
            .get("components")
            .and_then(|c| c.get("schemas"))
            .and_then(Value::as_object)
        {
            for (name, schema) in schemas {
                all_schemas.insert(name.clone(), schema);
            }
        }
    }

    // 2. Find all top-level resources (schemas with x-kubernetes-group-version-kind)
    let mut quantities: HashMap<String, Vec<String>> = HashMap::new();

    for (full_name, schema) in &all_schemas {
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

                // DFS from this schema
                let mut paths = Vec::new();
                let mut visited = HashSet::new();
                visited.insert(full_name.clone());
                walk_schema(schema, "$", &all_schemas, &mut visited, &mut paths);

                if !paths.is_empty() {
                    paths.sort();
                    paths.dedup();
                    quantities.insert(key, paths);
                }
            }
        }
    }

    // 3. Build the JSON output
    let mut quantities_json = serde_json::Map::new();
    let mut keys: Vec<_> = quantities.keys().cloned().collect();
    keys.sort();
    for key in keys {
        let paths = quantities.remove(&key).unwrap();
        let paths_json: Vec<Value> = paths.into_iter().map(Value::String).collect();
        quantities_json.insert(key, Value::Array(paths_json));
    }

    serde_json::json!({
        "version": 1,
        "quantities": quantities_json
    })
}

fn walk_schema(
    schema: &Value,
    current_path: &str,
    all_schemas: &HashMap<String, &Value>,
    visited: &mut HashSet<String>,
    paths: &mut Vec<String>,
) {
    let properties = match schema.get("properties").and_then(Value::as_object) {
        Some(p) => p,
        None => return,
    };

    for (prop_name, prop_schema) in properties {
        let prop_path = format!("{current_path}.{prop_name}");
        walk_property(prop_schema, &prop_path, all_schemas, visited, paths);
    }
}

fn walk_property(
    schema: &Value,
    current_path: &str,
    all_schemas: &HashMap<String, &Value>,
    visited: &mut HashSet<String>,
    paths: &mut Vec<String>,
) {
    // Check for $ref
    if let Some(ref_str) = schema.get("$ref").and_then(Value::as_str) {
        let ref_name = ref_str
            .strip_prefix("#/components/schemas/")
            .unwrap_or(ref_str);

        // Check if it's a Quantity ref
        if ref_name == QUANTITY_REF {
            paths.push(current_path.to_string());
            return;
        }

        // Follow the ref if not visited (cycle detection)
        if visited.insert(ref_name.to_string()) {
            if let Some(ref_schema) = all_schemas.get(ref_name) {
                walk_schema(ref_schema, current_path, all_schemas, visited, paths);
            }
            visited.remove(ref_name);
        }
        return;
    }

    let type_str = schema.get("type").and_then(Value::as_str).unwrap_or("");

    match type_str {
        "array" => {
            // items -> [*]
            if let Some(items) = schema.get("items") {
                let arr_path = format!("{current_path}[*]");
                walk_property(items, &arr_path, all_schemas, visited, paths);
            }
        }
        "object" => {
            if let Some(additional) = schema.get("additionalProperties") {
                // Map type -> [*]
                let map_path = format!("{current_path}[*]");
                walk_property(additional, &map_path, all_schemas, visited, paths);
            } else if schema.get("properties").is_some() {
                // Inline object with properties
                walk_schema(schema, current_path, all_schemas, visited, paths);
            }
        }
        _ => {
            // Check for inline properties (some schemas don't have "type" set)
            if schema.get("properties").is_some() {
                walk_schema(schema, current_path, all_schemas, visited, paths);
            }
        }
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
                            "template": {"$ref": "#/components/schemas/io.k8s.api.core.v1.PodTemplateSpec"}
                        }
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
                    }
                }
            }
        })
    }

    #[test]
    fn detects_quantity_paths_in_deployment() {
        let specs = HashMap::from([("apis/apps/v1".to_string(), deployment_spec())]);
        let result = generate_validation_map(&specs);

        let quantities = result["quantities"].as_object().unwrap();
        assert!(quantities.contains_key("apps/v1:Deployment"));

        let paths = quantities["apps/v1:Deployment"].as_array().unwrap();
        let path_strs: Vec<&str> = paths.iter().filter_map(Value::as_str).collect();
        assert!(path_strs.contains(&"$.spec.template.spec.containers[*].resources.limits[*]"));
        assert!(path_strs.contains(&"$.spec.template.spec.containers[*].resources.requests[*]"));
    }

    #[test]
    fn pv_capacity_detection() {
        let spec = json!({
            "components": {
                "schemas": {
                    "io.k8s.api.core.v1.PersistentVolume": {
                        "properties": {
                            "apiVersion": {"type": "string"},
                            "kind": {"type": "string"},
                            "spec": {"$ref": "#/components/schemas/io.k8s.api.core.v1.PersistentVolumeSpec"}
                        },
                        "x-kubernetes-group-version-kind": [
                            {"group": "", "version": "v1", "kind": "PersistentVolume"}
                        ]
                    },
                    "io.k8s.api.core.v1.PersistentVolumeSpec": {
                        "properties": {
                            "capacity": {
                                "type": "object",
                                "additionalProperties": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.api.resource.Quantity"}
                            }
                        }
                    },
                    "io.k8s.apimachinery.pkg.api.resource.Quantity": {
                        "type": "string"
                    }
                }
            }
        });

        let specs = HashMap::from([("api/v1".to_string(), spec)]);
        let result = generate_validation_map(&specs);

        let quantities = result["quantities"].as_object().unwrap();
        assert!(quantities.contains_key("v1:PersistentVolume"));

        let paths = quantities["v1:PersistentVolume"].as_array().unwrap();
        let path_strs: Vec<&str> = paths.iter().filter_map(Value::as_str).collect();
        assert!(path_strs.contains(&"$.spec.capacity[*]"));
    }

    #[test]
    fn cycle_detection() {
        // Schema A references B, B references A
        let spec = json!({
            "components": {
                "schemas": {
                    "io.k8s.api.test.v1.CyclicResource": {
                        "properties": {
                            "apiVersion": {"type": "string"},
                            "spec": {"$ref": "#/components/schemas/io.k8s.api.test.v1.SpecA"}
                        },
                        "x-kubernetes-group-version-kind": [
                            {"group": "test", "version": "v1", "kind": "CyclicResource"}
                        ]
                    },
                    "io.k8s.api.test.v1.SpecA": {
                        "properties": {
                            "nested": {"$ref": "#/components/schemas/io.k8s.api.test.v1.SpecB"}
                        }
                    },
                    "io.k8s.api.test.v1.SpecB": {
                        "properties": {
                            "back": {"$ref": "#/components/schemas/io.k8s.api.test.v1.SpecA"},
                            "quantity": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.api.resource.Quantity"}
                        }
                    },
                    "io.k8s.apimachinery.pkg.api.resource.Quantity": {
                        "type": "string"
                    }
                }
            }
        });

        let specs = HashMap::from([("apis/test/v1".to_string(), spec)]);
        let result = generate_validation_map(&specs);

        // Should not infinite loop, and should find the quantity path
        let quantities = result["quantities"].as_object().unwrap();
        assert!(quantities.contains_key("test/v1:CyclicResource"));

        let paths = quantities["test/v1:CyclicResource"].as_array().unwrap();
        let path_strs: Vec<&str> = paths.iter().filter_map(Value::as_str).collect();
        assert!(path_strs.contains(&"$.spec.nested.quantity"));
    }

    #[test]
    fn no_quantities_produces_empty() {
        let spec = json!({
            "components": {
                "schemas": {
                    "io.k8s.api.core.v1.Namespace": {
                        "properties": {
                            "apiVersion": {"type": "string"},
                            "kind": {"type": "string"},
                            "metadata": {"type": "object"}
                        },
                        "x-kubernetes-group-version-kind": [
                            {"group": "", "version": "v1", "kind": "Namespace"}
                        ]
                    }
                }
            }
        });

        let specs = HashMap::from([("api/v1".to_string(), spec)]);
        let result = generate_validation_map(&specs);

        let quantities = result["quantities"].as_object().unwrap();
        // Namespace has no quantity fields
        assert!(!quantities.contains_key("v1:Namespace"));
    }

    #[test]
    fn snapshot_validation_json() {
        let specs = HashMap::from([("apis/apps/v1".to_string(), deployment_spec())]);
        let result = generate_validation_map(&specs);
        let formatted = serde_json::to_string_pretty(&result).unwrap();
        insta::assert_snapshot!("validation_json", formatted);
    }
}
