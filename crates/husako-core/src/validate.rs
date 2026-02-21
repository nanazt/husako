use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use serde_json::Value;

use crate::quantity;

const MAX_DEPTH: usize = 64;

// ---------------------------------------------------------------------------
// SchemaStore
// ---------------------------------------------------------------------------

/// Loaded `_schema.json` with resolved GVK index.
#[derive(Debug, Clone)]
pub struct SchemaStore {
    gvk_index: HashMap<String, String>,
    schemas: HashMap<String, Value>,
}

impl SchemaStore {
    /// Load from parsed `_schema.json` content.
    pub fn from_json(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;

        let version = obj.get("version")?.as_u64()?;
        if version != 2 {
            return None;
        }

        let gvk_index: HashMap<String, String> = obj
            .get("gvk_index")?
            .as_object()?
            .iter()
            .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
            .collect();

        let schemas: HashMap<String, Value> = obj
            .get("schemas")?
            .as_object()?
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        Some(Self { gvk_index, schemas })
    }

    fn schema_for_gvk(&self, api_version: &str, kind: &str) -> Option<&Value> {
        let key = format!("{api_version}:{kind}");
        let schema_name = self.gvk_index.get(&key)?;
        self.schemas.get(schema_name)
    }

    fn resolve_ref(&self, ref_name: &str) -> Option<&Value> {
        self.schemas.get(ref_name)
    }
}

/// Load a `SchemaStore` from `.husako/types/k8s/_schema.json` if it exists.
pub fn load_schema_store(project_root: &Path) -> Option<SchemaStore> {
    let path = project_root.join(".husako/types/k8s/_schema.json");
    let content = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&content).ok()?;
    SchemaStore::from_json(&value)
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ValidationError {
    pub doc_index: usize,
    pub path: String,
    pub kind: ValidationErrorKind,
}

#[derive(Debug)]
pub enum ValidationErrorKind {
    TypeMismatch { expected: &'static str, got: String },
    MissingRequired { field: String },
    InvalidEnum { value: String, allowed: Vec<String> },
    InvalidQuantity { value: String },
    PatternMismatch { value: String, pattern: String },
    BelowMinimum { value: f64, minimum: f64 },
    AboveMaximum { value: f64, maximum: f64 },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "doc[{}] at {}: ", self.doc_index, self.path)?;
        match &self.kind {
            ValidationErrorKind::TypeMismatch { expected, got } => {
                write!(f, "expected type {expected}, got {got}")
            }
            ValidationErrorKind::MissingRequired { field } => {
                write!(f, "missing required field \"{field}\"")
            }
            ValidationErrorKind::InvalidEnum { value, allowed } => {
                let opts = allowed.join(", ");
                write!(f, "invalid value \"{value}\", expected one of: {opts}")
            }
            ValidationErrorKind::InvalidQuantity { value } => {
                write!(f, "invalid quantity \"{value}\"")
            }
            ValidationErrorKind::PatternMismatch { value, pattern } => {
                write!(f, "value \"{value}\" does not match pattern \"{pattern}\"")
            }
            ValidationErrorKind::BelowMinimum { value, minimum } => {
                write!(f, "value {value} is below minimum {minimum}")
            }
            ValidationErrorKind::AboveMaximum { value, maximum } => {
                write!(f, "value {value} is above maximum {maximum}")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Validation entry point
// ---------------------------------------------------------------------------

/// Validate all documents in the build output.
///
/// If a `SchemaStore` is available, validates each document against its
/// schema (looked up by apiVersion + kind). Falls back to quantity-only
/// heuristic validation when no store is available or no schema matches.
pub fn validate(value: &Value, store: Option<&SchemaStore>) -> Result<(), Vec<ValidationError>> {
    let docs = match value.as_array() {
        Some(arr) => arr,
        None => return Ok(()),
    };

    let mut errors = Vec::new();

    for (idx, doc) in docs.iter().enumerate() {
        if let Some(store) = store {
            let api_version = doc.get("apiVersion").and_then(Value::as_str).unwrap_or("");
            let kind = doc.get("kind").and_then(Value::as_str).unwrap_or("");

            if let Some(schema) = store.schema_for_gvk(api_version, kind) {
                validate_value(doc, schema, store, "$", idx, 0, &mut errors);
                continue;
            }
        }
        // Fallback: quantity-only heuristic
        validate_doc_fallback(doc, idx, &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_doc_fallback(doc: &Value, doc_index: usize, errors: &mut Vec<ValidationError>) {
    let mut qty_errors = Vec::new();
    quantity::validate_doc_fallback(doc, doc_index, &mut qty_errors);
    for qe in qty_errors {
        errors.push(ValidationError {
            doc_index: qe.doc_index,
            path: qe.path,
            kind: ValidationErrorKind::InvalidQuantity { value: qe.value },
        });
    }
}

// ---------------------------------------------------------------------------
// Recursive schema walker
// ---------------------------------------------------------------------------

fn validate_value(
    value: &Value,
    schema: &Value,
    store: &SchemaStore,
    path: &str,
    doc_index: usize,
    depth: usize,
    errors: &mut Vec<ValidationError>,
) {
    if depth > MAX_DEPTH {
        return;
    }

    // Skip null values (treat as "not set")
    if value.is_null() {
        return;
    }

    // Handle $ref
    if let Some(ref_name) = schema.get("$ref").and_then(Value::as_str) {
        if let Some(resolved) = store.resolve_ref(ref_name) {
            validate_value(value, resolved, store, path, doc_index, depth + 1, errors);
        }
        return;
    }

    // Handle allOf
    if let Some(all_of) = schema.get("allOf").and_then(Value::as_array) {
        for sub in all_of {
            validate_value(value, sub, store, path, doc_index, depth + 1, errors);
        }
        return;
    }

    // Handle x-kubernetes-int-or-string
    if schema
        .get("x-kubernetes-int-or-string")
        .and_then(Value::as_bool)
        == Some(true)
    {
        match value {
            Value::Number(_) | Value::String(_) => {}
            _ => {
                errors.push(ValidationError {
                    doc_index,
                    path: path.to_string(),
                    kind: ValidationErrorKind::TypeMismatch {
                        expected: "integer or string",
                        got: json_type_name(value).to_string(),
                    },
                });
            }
        }
        return;
    }

    // Check format (dispatch before generic type check)
    if let Some(format) = schema.get("format").and_then(Value::as_str)
        && format == "quantity"
    {
        validate_quantity(value, path, doc_index, errors);
        return;
    }

    // Check type
    if let Some(type_str) = schema.get("type").and_then(Value::as_str)
        && !check_type(value, type_str)
    {
        errors.push(ValidationError {
            doc_index,
            path: path.to_string(),
            kind: ValidationErrorKind::TypeMismatch {
                expected: type_str_to_label(type_str),
                got: json_type_name(value).to_string(),
            },
        });
        return;
    }

    // Check enum
    if let Some(enum_vals) = schema.get("enum").and_then(Value::as_array)
        && let Value::String(s) = value
    {
        let allowed: Vec<String> = enum_vals
            .iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect();
        if !allowed.iter().any(|a| a == s) {
            errors.push(ValidationError {
                doc_index,
                path: path.to_string(),
                kind: ValidationErrorKind::InvalidEnum {
                    value: s.clone(),
                    allowed,
                },
            });
            return;
        }
    }

    // Check numeric bounds
    if let Some(n) = value_as_f64(value) {
        if let Some(min) = schema.get("minimum").and_then(value_as_f64_ref)
            && n < min
        {
            errors.push(ValidationError {
                doc_index,
                path: path.to_string(),
                kind: ValidationErrorKind::BelowMinimum {
                    value: n,
                    minimum: min,
                },
            });
        }
        if let Some(max) = schema.get("maximum").and_then(value_as_f64_ref)
            && n > max
        {
            errors.push(ValidationError {
                doc_index,
                path: path.to_string(),
                kind: ValidationErrorKind::AboveMaximum {
                    value: n,
                    maximum: max,
                },
            });
        }
    }

    // Check pattern
    if let Some(pattern) = schema.get("pattern").and_then(Value::as_str)
        && let Value::String(s) = value
        && let Ok(re) = regex_lite::Regex::new(pattern)
        && !re.is_match(s)
    {
        errors.push(ValidationError {
            doc_index,
            path: path.to_string(),
            kind: ValidationErrorKind::PatternMismatch {
                value: s.clone(),
                pattern: pattern.to_string(),
            },
        });
    }

    // Check required fields + recurse into properties/additionalProperties
    if let Value::Object(obj) = value {
        if let Some(required) = schema.get("required").and_then(Value::as_array) {
            for req in required {
                if let Some(field) = req.as_str()
                    && !obj.contains_key(field)
                {
                    errors.push(ValidationError {
                        doc_index,
                        path: path.to_string(),
                        kind: ValidationErrorKind::MissingRequired {
                            field: field.to_string(),
                        },
                    });
                }
            }
        }

        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            for (prop_name, prop_schema) in properties {
                if let Some(child) = obj.get(prop_name) {
                    let child_path = format!("{path}.{prop_name}");
                    validate_value(
                        child,
                        prop_schema,
                        store,
                        &child_path,
                        doc_index,
                        depth + 1,
                        errors,
                    );
                }
            }
        }

        if let Some(additional) = schema.get("additionalProperties") {
            let known_props: std::collections::HashSet<&str> = schema
                .get("properties")
                .and_then(Value::as_object)
                .map(|p| p.keys().map(String::as_str).collect())
                .unwrap_or_default();

            for (key, child) in obj {
                if !known_props.contains(key.as_str()) {
                    let child_path = format!("{path}.{key}");
                    validate_value(
                        child,
                        additional,
                        store,
                        &child_path,
                        doc_index,
                        depth + 1,
                        errors,
                    );
                }
            }
        }
    }

    // Recurse into array items
    if let Value::Array(arr) = value
        && let Some(items) = schema.get("items")
    {
        for (i, item) in arr.iter().enumerate() {
            let item_path = format!("{path}[{i}]");
            validate_value(item, items, store, &item_path, doc_index, depth + 1, errors);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn validate_quantity(
    value: &Value,
    path: &str,
    doc_index: usize,
    errors: &mut Vec<ValidationError>,
) {
    match value {
        Value::String(s) => {
            if !quantity::is_valid_quantity(s) {
                errors.push(ValidationError {
                    doc_index,
                    path: path.to_string(),
                    kind: ValidationErrorKind::InvalidQuantity { value: s.clone() },
                });
            }
        }
        Value::Number(_) | Value::Null => {} // valid
        _ => {
            errors.push(ValidationError {
                doc_index,
                path: path.to_string(),
                kind: ValidationErrorKind::TypeMismatch {
                    expected: "string or number (quantity)",
                    got: json_type_name(value).to_string(),
                },
            });
        }
    }
}

fn check_type(value: &Value, type_str: &str) -> bool {
    match type_str {
        "string" => value.is_string(),
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        _ => true, // unknown type → pass
    }
}

fn type_str_to_label(s: &str) -> &'static str {
    match s {
        "string" => "string",
        "integer" => "integer",
        "number" => "number",
        "boolean" => "boolean",
        "array" => "array",
        "object" => "object",
        _ => "unknown",
    }
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn value_as_f64(value: &Value) -> Option<f64> {
    value.as_f64()
}

fn value_as_f64_ref(value: &Value) -> Option<f64> {
    value.as_f64()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_store(schemas_json: Value, gvk_json: Value) -> SchemaStore {
        let store_json = json!({
            "version": 2,
            "gvk_index": gvk_json,
            "schemas": schemas_json
        });
        SchemaStore::from_json(&store_json).unwrap()
    }

    fn simple_store() -> SchemaStore {
        make_store(
            json!({
                "io.k8s.api.apps.v1.Deployment": {
                    "properties": {
                        "apiVersion": {"type": "string"},
                        "kind": {"type": "string"},
                        "metadata": {"$ref": "io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta"},
                        "spec": {"$ref": "io.k8s.api.apps.v1.DeploymentSpec"}
                    },
                    "required": ["spec"]
                },
                "io.k8s.api.apps.v1.DeploymentSpec": {
                    "properties": {
                        "replicas": {"type": "integer"},
                        "selector": {"$ref": "io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelector"},
                        "strategy": {"$ref": "io.k8s.api.apps.v1.DeploymentStrategy"},
                        "template": {"$ref": "io.k8s.api.core.v1.PodTemplateSpec"}
                    },
                    "required": ["selector"]
                },
                "io.k8s.api.apps.v1.DeploymentStrategy": {
                    "properties": {
                        "type": {
                            "type": "string",
                            "enum": ["Recreate", "RollingUpdate"]
                        }
                    }
                },
                "io.k8s.api.core.v1.PodTemplateSpec": {
                    "properties": {
                        "spec": {"$ref": "io.k8s.api.core.v1.PodSpec"}
                    }
                },
                "io.k8s.api.core.v1.PodSpec": {
                    "properties": {
                        "containers": {
                            "type": "array",
                            "items": {"$ref": "io.k8s.api.core.v1.Container"}
                        }
                    }
                },
                "io.k8s.api.core.v1.Container": {
                    "properties": {
                        "name": {"type": "string"},
                        "image": {"type": "string"},
                        "imagePullPolicy": {
                            "type": "string",
                            "enum": ["Always", "IfNotPresent", "Never"]
                        },
                        "ports": {
                            "type": "array",
                            "items": {"$ref": "io.k8s.api.core.v1.ContainerPort"}
                        },
                        "resources": {"$ref": "io.k8s.api.core.v1.ResourceRequirements"}
                    }
                },
                "io.k8s.api.core.v1.ContainerPort": {
                    "properties": {
                        "containerPort": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 65535
                        },
                        "protocol": {
                            "type": "string",
                            "enum": ["TCP", "UDP", "SCTP"]
                        }
                    }
                },
                "io.k8s.api.core.v1.ResourceRequirements": {
                    "properties": {
                        "limits": {
                            "type": "object",
                            "additionalProperties": {"$ref": "io.k8s.apimachinery.pkg.api.resource.Quantity"}
                        },
                        "requests": {
                            "type": "object",
                            "additionalProperties": {"$ref": "io.k8s.apimachinery.pkg.api.resource.Quantity"}
                        }
                    }
                },
                "io.k8s.apimachinery.pkg.api.resource.Quantity": {
                    "type": "string",
                    "format": "quantity"
                },
                "io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta": {
                    "properties": {
                        "name": {"type": "string"},
                        "namespace": {"type": "string"},
                        "labels": {
                            "type": "object",
                            "additionalProperties": {"type": "string"}
                        }
                    }
                },
                "io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelector": {
                    "properties": {
                        "matchLabels": {
                            "type": "object",
                            "additionalProperties": {"type": "string"}
                        }
                    }
                }
            }),
            json!({
                "apps/v1:Deployment": "io.k8s.api.apps.v1.Deployment"
            }),
        )
    }

    // --- Type checks ---

    #[test]
    fn type_mismatch_string_at_integer() {
        let store = simple_store();
        let doc = json!([{
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "spec": {
                "selector": {},
                "replicas": "abc"
            }
        }]);
        let errs = validate(&doc, Some(&store)).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].path.contains("replicas"));
        assert!(matches!(
            &errs[0].kind,
            ValidationErrorKind::TypeMismatch {
                expected: "integer",
                ..
            }
        ));
        assert!(errs[0].to_string().contains("expected type integer"));
        assert!(errs[0].to_string().contains("string"));
    }

    // --- Required ---

    #[test]
    fn missing_required_field() {
        let store = simple_store();
        let doc = json!([{
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "spec": {
                "replicas": 3
            }
        }]);
        let errs = validate(&doc, Some(&store)).unwrap_err();
        assert!(errs.iter().any(|e| matches!(
            &e.kind,
            ValidationErrorKind::MissingRequired { field } if field == "selector"
        )));
    }

    // --- Enum ---

    #[test]
    fn invalid_enum_value() {
        let store = simple_store();
        let doc = json!([{
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "spec": {
                "selector": {},
                "strategy": {
                    "type": "bluegreen"
                }
            }
        }]);
        let errs = validate(&doc, Some(&store)).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(
            matches!(&errs[0].kind, ValidationErrorKind::InvalidEnum { value, allowed }
                if value == "bluegreen" && allowed.contains(&"Recreate".to_string())
            )
        );
        assert!(errs[0].to_string().contains("bluegreen"));
    }

    #[test]
    fn valid_enum_value() {
        let store = simple_store();
        let doc = json!([{
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "spec": {
                "selector": {},
                "template": {
                    "spec": {
                        "containers": [{
                            "imagePullPolicy": "Always"
                        }]
                    }
                }
            }
        }]);
        assert!(validate(&doc, Some(&store)).is_ok());
    }

    // --- Format: quantity ---

    #[test]
    fn valid_quantity() {
        let store = simple_store();
        let doc = json!([{
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "spec": {
                "selector": {},
                "template": {
                    "spec": {
                        "containers": [{
                            "resources": {
                                "requests": {"cpu": "500m", "memory": "1Gi"}
                            }
                        }]
                    }
                }
            }
        }]);
        assert!(validate(&doc, Some(&store)).is_ok());
    }

    #[test]
    fn invalid_quantity() {
        let store = simple_store();
        let doc = json!([{
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "spec": {
                "selector": {},
                "template": {
                    "spec": {
                        "containers": [{
                            "resources": {
                                "requests": {"cpu": "2gb"}
                            }
                        }]
                    }
                }
            }
        }]);
        let errs = validate(&doc, Some(&store)).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(
            matches!(&errs[0].kind, ValidationErrorKind::InvalidQuantity { value } if value == "2gb")
        );
    }

    #[test]
    fn number_at_quantity_is_valid() {
        let store = simple_store();
        let doc = json!([{
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "spec": {
                "selector": {},
                "template": {
                    "spec": {
                        "containers": [{
                            "resources": {
                                "limits": {"cpu": 1}
                            }
                        }]
                    }
                }
            }
        }]);
        assert!(validate(&doc, Some(&store)).is_ok());
    }

    // --- Pattern ---

    #[test]
    fn pattern_match_ok() {
        let store = make_store(
            json!({
                "test.Resource": {
                    "properties": {
                        "name": {
                            "type": "string",
                            "pattern": "^[a-z][a-z0-9-]*$"
                        }
                    }
                }
            }),
            json!({ "v1:Test": "test.Resource" }),
        );
        let doc = json!([{
            "apiVersion": "v1",
            "kind": "Test",
            "name": "my-resource-1"
        }]);
        assert!(validate(&doc, Some(&store)).is_ok());
    }

    #[test]
    fn pattern_mismatch() {
        let store = make_store(
            json!({
                "test.Resource": {
                    "properties": {
                        "name": {
                            "type": "string",
                            "pattern": "^[a-z][a-z0-9-]*$"
                        }
                    }
                }
            }),
            json!({ "v1:Test": "test.Resource" }),
        );
        let doc = json!([{
            "apiVersion": "v1",
            "kind": "Test",
            "name": "INVALID_NAME"
        }]);
        let errs = validate(&doc, Some(&store)).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(matches!(
            &errs[0].kind,
            ValidationErrorKind::PatternMismatch { .. }
        ));
    }

    // --- Bounds ---

    #[test]
    fn port_in_range() {
        let store = simple_store();
        let doc = json!([{
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "spec": {
                "selector": {},
                "template": {
                    "spec": {
                        "containers": [{
                            "ports": [{"containerPort": 80}]
                        }]
                    }
                }
            }
        }]);
        assert!(validate(&doc, Some(&store)).is_ok());
    }

    #[test]
    fn port_below_minimum() {
        let store = simple_store();
        let doc = json!([{
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "spec": {
                "selector": {},
                "template": {
                    "spec": {
                        "containers": [{
                            "ports": [{"containerPort": 0}]
                        }]
                    }
                }
            }
        }]);
        let errs = validate(&doc, Some(&store)).unwrap_err();
        assert!(
            errs.iter()
                .any(|e| matches!(&e.kind, ValidationErrorKind::BelowMinimum { .. }))
        );
    }

    #[test]
    fn port_above_maximum() {
        let store = simple_store();
        let doc = json!([{
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "spec": {
                "selector": {},
                "template": {
                    "spec": {
                        "containers": [{
                            "ports": [{"containerPort": 70000}]
                        }]
                    }
                }
            }
        }]);
        let errs = validate(&doc, Some(&store)).unwrap_err();
        assert!(
            errs.iter()
                .any(|e| matches!(&e.kind, ValidationErrorKind::AboveMaximum { .. }))
        );
    }

    // --- x-kubernetes-int-or-string ---

    #[test]
    fn int_or_string_number_ok() {
        let store = make_store(
            json!({
                "test.Resource": {
                    "properties": {
                        "field": {"x-kubernetes-int-or-string": true}
                    }
                }
            }),
            json!({ "v1:Test": "test.Resource" }),
        );
        let doc = json!([{ "apiVersion": "v1", "kind": "Test", "field": 42 }]);
        assert!(validate(&doc, Some(&store)).is_ok());
    }

    #[test]
    fn int_or_string_string_ok() {
        let store = make_store(
            json!({
                "test.Resource": {
                    "properties": {
                        "field": {"x-kubernetes-int-or-string": true}
                    }
                }
            }),
            json!({ "v1:Test": "test.Resource" }),
        );
        let doc = json!([{ "apiVersion": "v1", "kind": "Test", "field": "50%" }]);
        assert!(validate(&doc, Some(&store)).is_ok());
    }

    #[test]
    fn int_or_string_boolean_error() {
        let store = make_store(
            json!({
                "test.Resource": {
                    "properties": {
                        "field": {"x-kubernetes-int-or-string": true}
                    }
                }
            }),
            json!({ "v1:Test": "test.Resource" }),
        );
        let doc = json!([{ "apiVersion": "v1", "kind": "Test", "field": true }]);
        let errs = validate(&doc, Some(&store)).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(matches!(
            &errs[0].kind,
            ValidationErrorKind::TypeMismatch {
                expected: "integer or string",
                ..
            }
        ));
    }

    // --- allOf ---

    #[test]
    fn allof_validates_all_sub_schemas() {
        let store = make_store(
            json!({
                "test.Resource": {
                    "properties": {
                        "value": {
                            "allOf": [
                                {"type": "integer"},
                                {"minimum": 1, "maximum": 100}
                            ]
                        }
                    }
                }
            }),
            json!({ "v1:Test": "test.Resource" }),
        );

        // Valid
        let doc = json!([{ "apiVersion": "v1", "kind": "Test", "value": 50 }]);
        assert!(validate(&doc, Some(&store)).is_ok());

        // Below minimum
        let doc = json!([{ "apiVersion": "v1", "kind": "Test", "value": 0 }]);
        let errs = validate(&doc, Some(&store)).unwrap_err();
        assert!(
            errs.iter()
                .any(|e| matches!(&e.kind, ValidationErrorKind::BelowMinimum { .. }))
        );
    }

    // --- $ref resolution ---

    #[test]
    fn ref_resolution() {
        let store = simple_store();
        let doc = json!([{
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "spec": {
                "selector": {},
                "template": {
                    "spec": {
                        "containers": [{
                            "name": 123
                        }]
                    }
                }
            }
        }]);
        let errs = validate(&doc, Some(&store)).unwrap_err();
        assert!(errs.iter().any(|e| e.path.contains("name")
            && matches!(
                &e.kind,
                ValidationErrorKind::TypeMismatch {
                    expected: "string",
                    ..
                }
            )));
    }

    // --- Null at optional position ---

    #[test]
    fn null_at_optional_skip() {
        let store = simple_store();
        let doc = json!([{
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "spec": {
                "selector": {},
                "replicas": null
            }
        }]);
        assert!(validate(&doc, Some(&store)).is_ok());
    }

    // --- Depth limit ---

    #[test]
    fn depth_limit_no_stack_overflow() {
        let store = make_store(
            json!({
                "test.Recursive": {
                    "properties": {
                        "nested": {"$ref": "test.Recursive"}
                    }
                }
            }),
            json!({ "v1:Test": "test.Recursive" }),
        );

        // Build a deeply nested doc (won't reach 64 in practice, but the schema refs itself)
        let mut inner = json!({"val": 1});
        for _ in 0..10 {
            inner = json!({"nested": inner});
        }
        let doc = json!([{
            "apiVersion": "v1",
            "kind": "Test",
            "nested": inner
        }]);
        // Should not panic; errors or success both acceptable
        let _ = validate(&doc, Some(&store));
    }

    // --- Fallback ---

    #[test]
    fn fallback_no_schema_store() {
        let doc = json!([{
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "spec": {
                "template": {
                    "spec": {
                        "containers": [{
                            "resources": {
                                "requests": {"cpu": "2gb"}
                            }
                        }]
                    }
                }
            }
        }]);
        let errs = validate(&doc, None).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(matches!(
            &errs[0].kind,
            ValidationErrorKind::InvalidQuantity { .. }
        ));
    }

    #[test]
    fn fallback_unknown_gvk() {
        let store = simple_store();
        let doc = json!([{
            "apiVersion": "unknown/v1",
            "kind": "Custom",
            "spec": {
                "resources": {
                    "requests": {"cpu": "2gb"}
                }
            }
        }]);
        let errs = validate(&doc, Some(&store)).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(matches!(
            &errs[0].kind,
            ValidationErrorKind::InvalidQuantity { .. }
        ));
    }

    // --- SchemaStore loading ---

    #[test]
    fn schema_store_from_json_wrong_version() {
        let json = json!({"version": 1, "gvk_index": {}, "schemas": {}});
        assert!(SchemaStore::from_json(&json).is_none());
    }

    #[test]
    fn schema_store_from_json_valid() {
        let json = json!({"version": 2, "gvk_index": {"v1:Ns": "some.Schema"}, "schemas": {"some.Schema": {"type": "object"}}});
        let store = SchemaStore::from_json(&json).unwrap();
        assert!(store.resolve_ref("some.Schema").is_some());
    }

    // --- Display ---

    #[test]
    fn error_display_format() {
        let err = ValidationError {
            doc_index: 0,
            path: "$.spec.replicas".to_string(),
            kind: ValidationErrorKind::TypeMismatch {
                expected: "integer",
                got: "string".to_string(),
            },
        };
        let s = err.to_string();
        assert_eq!(
            s,
            "doc[0] at $.spec.replicas: expected type integer, got string"
        );
    }

    #[test]
    fn error_display_enum() {
        let err = ValidationError {
            doc_index: 0,
            path: "$.spec.strategy.type".to_string(),
            kind: ValidationErrorKind::InvalidEnum {
                value: "bluegreen".to_string(),
                allowed: vec!["Recreate".to_string(), "RollingUpdate".to_string()],
            },
        };
        let s = err.to_string();
        assert_eq!(
            s,
            "doc[0] at $.spec.strategy.type: invalid value \"bluegreen\", expected one of: Recreate, RollingUpdate"
        );
    }
}
