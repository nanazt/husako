use serde_json::Value;

/// Where a schema belongs in the generated output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaLocation {
    /// Shared types from `io.k8s.apimachinery.*` → `_common.d.ts`
    Common,
    /// Per-group-version types from `io.k8s.api.<group>.<version>.*`
    GroupVersion { group: String, version: String },
    /// Unknown/other schemas we skip.
    Other,
}

/// Extracted group-version-kind from `x-kubernetes-group-version-kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupVersionKind {
    pub group: String,
    pub version: String,
    pub kind: String,
}

/// A TypeScript type representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TsType {
    String,
    Number,
    Boolean,
    IntOrString,
    Array(Box<TsType>),
    Map(Box<TsType>),
    Ref(String),
    Any,
}

/// A single property within a schema.
#[derive(Debug, Clone)]
pub struct PropertyInfo {
    pub name: String,
    pub ts_type: TsType,
    pub required: bool,
    pub description: Option<String>,
}

/// A parsed OpenAPI schema.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SchemaInfo {
    /// Original full name, e.g. `io.k8s.api.apps.v1.Deployment`
    pub full_name: String,
    /// Short TypeScript name, e.g. `Deployment`
    pub ts_name: String,
    /// Where this schema belongs.
    pub location: SchemaLocation,
    /// Properties of the schema.
    pub properties: Vec<PropertyInfo>,
    /// GVK info if present (top-level resources).
    pub gvk: Option<GroupVersionKind>,
    /// Description from the spec.
    pub description: Option<String>,
}

/// Extract the short TS name from a full OpenAPI schema name.
/// `io.k8s.api.apps.v1.Deployment` → `Deployment`
pub fn ts_name_from_full(full_name: &str) -> String {
    full_name
        .rsplit('.')
        .next()
        .unwrap_or(full_name)
        .to_string()
}

/// Classify a schema into Common, GroupVersion, or Other.
pub fn classify_schema(full_name: &str) -> SchemaLocation {
    if full_name.starts_with("io.k8s.apimachinery.") {
        return SchemaLocation::Common;
    }

    // io.k8s.api.<group>.<version>.<Type>
    if let Some(rest) = full_name.strip_prefix("io.k8s.api.") {
        let parts: Vec<&str> = rest.splitn(3, '.').collect();
        if parts.len() == 3 {
            return SchemaLocation::GroupVersion {
                group: parts[0].to_string(),
                version: parts[1].to_string(),
            };
        }
    }

    SchemaLocation::Other
}

/// Map an OpenAPI schema value to a TsType.
pub fn ts_type_from_schema(schema: &Value) -> TsType {
    // Handle $ref
    if let Some(ref_str) = schema.get("$ref").and_then(Value::as_str) {
        let ref_name = ref_str
            .strip_prefix("#/components/schemas/")
            .unwrap_or(ref_str);
        return TsType::Ref(ts_name_from_full(ref_name));
    }

    // Handle allOf with a single entry (k8s 1.35+ wraps $ref in allOf for array items)
    // e.g. "items": {"allOf": [{"$ref": "..."}], "default": {}}
    if let Some(all_of) = schema.get("allOf").and_then(Value::as_array)
        && let Some(first) = all_of.first()
    {
        return ts_type_from_schema(first);
    }

    // x-kubernetes-int-or-string
    if schema
        .get("x-kubernetes-int-or-string")
        .and_then(Value::as_bool)
        == Some(true)
    {
        return TsType::IntOrString;
    }

    let type_str = schema.get("type").and_then(Value::as_str).unwrap_or("");

    match type_str {
        "string" => TsType::String,
        "integer" | "number" => TsType::Number,
        "boolean" => TsType::Boolean,
        "array" => {
            let items_type = schema
                .get("items")
                .map(ts_type_from_schema)
                .unwrap_or(TsType::Any);
            TsType::Array(Box::new(items_type))
        }
        "object" => {
            if let Some(additional) = schema.get("additionalProperties") {
                let val_type = ts_type_from_schema(additional);
                TsType::Map(Box::new(val_type))
            } else if schema.get("properties").is_some() {
                // Inline object with properties — treat as Any for simplicity.
                // Full inline objects would need recursive handling.
                TsType::Any
            } else {
                TsType::Map(Box::new(TsType::Any))
            }
        }
        _ => TsType::Any,
    }
}

/// Check if a schema has at least one property with a `Ref` or `Array(Ref)` type.
/// Such schemas benefit from builder generation (they have deep nesting).
pub fn has_complex_property(schema: &SchemaInfo) -> bool {
    schema
        .properties
        .iter()
        .any(|p| is_complex_type(&p.ts_type))
}

fn is_complex_type(ty: &TsType) -> bool {
    match ty {
        TsType::Ref(_) => true,
        TsType::Array(inner) => matches!(inner.as_ref(), TsType::Ref(_)),
        _ => false,
    }
}

/// Extract GVK from `x-kubernetes-group-version-kind` extension.
fn extract_gvk(schema: &Value) -> Option<GroupVersionKind> {
    let gvk_array = schema.get("x-kubernetes-group-version-kind")?;
    let first = gvk_array.as_array()?.first()?;

    let group = first.get("group")?.as_str()?.to_string();
    let version = first.get("version")?.as_str()?.to_string();
    let kind = first.get("kind")?.as_str()?.to_string();

    Some(GroupVersionKind {
        group,
        version,
        kind,
    })
}

/// Parse a single schema definition into SchemaInfo.
fn parse_schema(full_name: &str, schema: &Value) -> SchemaInfo {
    let ts_name = ts_name_from_full(full_name);
    let location = classify_schema(full_name);
    let gvk = extract_gvk(schema);
    let description = schema
        .get("description")
        .and_then(Value::as_str)
        .map(String::from);

    let required_set: Vec<String> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|props| {
            props
                .iter()
                .map(|(name, prop_schema)| {
                    let ts_type = ts_type_from_schema(prop_schema);
                    let prop_desc = prop_schema
                        .get("description")
                        .and_then(Value::as_str)
                        .map(String::from);
                    PropertyInfo {
                        name: name.clone(),
                        ts_type,
                        required: required_set.contains(name),
                        description: prop_desc,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    SchemaInfo {
        full_name: full_name.to_string(),
        ts_name,
        location,
        properties,
        gvk,
        description,
    }
}

/// Parse all schemas from an OpenAPI v3 spec.
pub fn parse_spec(spec: &Value) -> Vec<SchemaInfo> {
    let schemas = match spec
        .get("components")
        .and_then(|c| c.get("schemas"))
        .and_then(Value::as_object)
    {
        Some(s) => s,
        None => return Vec::new(),
    };

    schemas
        .iter()
        .map(|(full_name, schema)| parse_schema(full_name, schema))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ts_name_extraction() {
        assert_eq!(
            ts_name_from_full("io.k8s.api.apps.v1.Deployment"),
            "Deployment"
        );
        assert_eq!(
            ts_name_from_full("io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta"),
            "ObjectMeta"
        );
    }

    #[test]
    fn classify_common() {
        assert_eq!(
            classify_schema("io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta"),
            SchemaLocation::Common
        );
    }

    #[test]
    fn classify_group_version() {
        assert_eq!(
            classify_schema("io.k8s.api.apps.v1.Deployment"),
            SchemaLocation::GroupVersion {
                group: "apps".to_string(),
                version: "v1".to_string(),
            }
        );
        assert_eq!(
            classify_schema("io.k8s.api.core.v1.Pod"),
            SchemaLocation::GroupVersion {
                group: "core".to_string(),
                version: "v1".to_string(),
            }
        );
    }

    #[test]
    fn classify_other() {
        assert_eq!(
            classify_schema("io.k8s.something.else"),
            SchemaLocation::Other
        );
    }

    #[test]
    fn ts_type_string() {
        let schema = json!({"type": "string"});
        assert_eq!(ts_type_from_schema(&schema), TsType::String);
    }

    #[test]
    fn ts_type_integer() {
        let schema = json!({"type": "integer"});
        assert_eq!(ts_type_from_schema(&schema), TsType::Number);
    }

    #[test]
    fn ts_type_boolean() {
        let schema = json!({"type": "boolean"});
        assert_eq!(ts_type_from_schema(&schema), TsType::Boolean);
    }

    #[test]
    fn ts_type_int_or_string() {
        let schema = json!({"x-kubernetes-int-or-string": true});
        assert_eq!(ts_type_from_schema(&schema), TsType::IntOrString);
    }

    #[test]
    fn ts_type_ref() {
        let schema =
            json!({"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta"});
        assert_eq!(
            ts_type_from_schema(&schema),
            TsType::Ref("ObjectMeta".to_string())
        );
    }

    #[test]
    fn ts_type_array() {
        let schema = json!({"type": "array", "items": {"type": "string"}});
        assert_eq!(
            ts_type_from_schema(&schema),
            TsType::Array(Box::new(TsType::String))
        );
    }

    #[test]
    fn ts_type_map() {
        let schema = json!({"type": "object", "additionalProperties": {"type": "string"}});
        assert_eq!(
            ts_type_from_schema(&schema),
            TsType::Map(Box::new(TsType::String))
        );
    }

    #[test]
    fn ts_type_all_of_single_ref() {
        // k8s 1.35+ wraps $ref in allOf for array items
        // e.g. "items": {"allOf": [{"$ref": "...LabelSelectorRequirement"}], "default": {}}
        let schema = json!({
            "allOf": [
                {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelectorRequirement"}
            ],
            "default": {}
        });
        assert_eq!(
            ts_type_from_schema(&schema),
            TsType::Ref("LabelSelectorRequirement".to_string())
        );
    }

    #[test]
    fn ts_type_array_with_all_of_items() {
        // Array whose items use allOf pattern should produce Array(Ref(...))
        let schema = json!({
            "type": "array",
            "items": {
                "allOf": [
                    {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelectorRequirement"}
                ],
                "default": {}
            }
        });
        assert_eq!(
            ts_type_from_schema(&schema),
            TsType::Array(Box::new(TsType::Ref(
                "LabelSelectorRequirement".to_string()
            )))
        );
    }

    #[test]
    fn gvk_extraction() {
        let schema = json!({
            "x-kubernetes-group-version-kind": [
                {"group": "apps", "version": "v1", "kind": "Deployment"}
            ]
        });
        let gvk = extract_gvk(&schema).unwrap();
        assert_eq!(gvk.group, "apps");
        assert_eq!(gvk.version, "v1");
        assert_eq!(gvk.kind, "Deployment");
    }

    #[test]
    fn gvk_missing() {
        let schema = json!({"type": "object"});
        assert!(extract_gvk(&schema).is_none());
    }

    #[test]
    fn parse_schema_with_properties() {
        let spec = json!({
            "components": {
                "schemas": {
                    "io.k8s.api.apps.v1.DeploymentSpec": {
                        "description": "DeploymentSpec defines the desired state",
                        "properties": {
                            "replicas": {"type": "integer", "description": "Number of replicas"},
                            "selector": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelector"},
                            "paused": {"type": "boolean"}
                        },
                        "required": ["selector"]
                    }
                }
            }
        });

        let schemas = parse_spec(&spec);
        assert_eq!(schemas.len(), 1);

        let s = &schemas[0];
        assert_eq!(s.ts_name, "DeploymentSpec");
        assert_eq!(s.properties.len(), 3);

        let selector = s.properties.iter().find(|p| p.name == "selector").unwrap();
        assert!(selector.required);
        assert_eq!(selector.ts_type, TsType::Ref("LabelSelector".to_string()));

        let replicas = s.properties.iter().find(|p| p.name == "replicas").unwrap();
        assert!(!replicas.required);
        assert_eq!(replicas.ts_type, TsType::Number);
    }

    #[test]
    fn parse_empty_spec() {
        let spec = json!({"openapi": "3.0.0"});
        let schemas = parse_spec(&spec);
        assert!(schemas.is_empty());
    }

    #[test]
    fn has_complex_property_with_ref() {
        let schema = SchemaInfo {
            full_name: "io.k8s.api.core.v1.Container".to_string(),
            ts_name: "Container".to_string(),
            location: SchemaLocation::GroupVersion {
                group: "core".to_string(),
                version: "v1".to_string(),
            },
            properties: vec![
                PropertyInfo {
                    name: "name".to_string(),
                    ts_type: TsType::String,
                    required: false,
                    description: None,
                },
                PropertyInfo {
                    name: "resources".to_string(),
                    ts_type: TsType::Ref("ResourceRequirements".to_string()),
                    required: false,
                    description: None,
                },
            ],
            gvk: None,
            description: None,
        };
        assert!(has_complex_property(&schema));
    }

    #[test]
    fn has_complex_property_with_array_ref() {
        let schema = SchemaInfo {
            full_name: "io.k8s.api.core.v1.PodSpec".to_string(),
            ts_name: "PodSpec".to_string(),
            location: SchemaLocation::GroupVersion {
                group: "core".to_string(),
                version: "v1".to_string(),
            },
            properties: vec![PropertyInfo {
                name: "containers".to_string(),
                ts_type: TsType::Array(Box::new(TsType::Ref("Container".to_string()))),
                required: false,
                description: None,
            }],
            gvk: None,
            description: None,
        };
        assert!(has_complex_property(&schema));
    }

    #[test]
    fn has_complex_property_simple_schema() {
        let schema = SchemaInfo {
            full_name: "io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelector".to_string(),
            ts_name: "LabelSelector".to_string(),
            location: SchemaLocation::Common,
            properties: vec![PropertyInfo {
                name: "matchLabels".to_string(),
                ts_type: TsType::Map(Box::new(TsType::String)),
                required: false,
                description: None,
            }],
            gvk: None,
            description: None,
        };
        assert!(!has_complex_property(&schema));
    }
}
