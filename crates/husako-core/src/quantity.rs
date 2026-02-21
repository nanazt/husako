use std::collections::HashMap;

use serde_json::Value;

/// Validates a string against the Kubernetes quantity grammar.
///
/// Grammar: `sign? digits (. digits?)? (exponent | suffix)?`
/// - DecimalSI suffixes: n, u, m, k, M, G, T, P, E
/// - BinarySI suffixes: Ki, Mi, Gi, Ti, Pi, Ei
/// - Exponent: e/E followed by optional sign and digits
///
/// Disambiguation: `"1E"` is suffix Exa, not an incomplete exponent.
pub fn is_valid_quantity(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return false;
    }

    let mut i = 0;

    // Optional sign
    if bytes[i] == b'+' || bytes[i] == b'-' {
        i += 1;
        if i >= bytes.len() {
            return false;
        }
    }

    // Must have at least one digit or a leading dot followed by digit
    let digits_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let has_integer_part = i > digits_start;

    // Optional decimal point
    let mut has_fractional_part = false;
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        has_fractional_part = i > frac_start;
    }

    // Must have at least some numeric content
    if !has_integer_part && !has_fractional_part {
        return false;
    }

    // Nothing left — bare number is valid
    if i >= bytes.len() {
        return true;
    }

    let rest = &s[i..];

    // Try suffix match first (before exponent, to handle "1E" as Exa)
    if is_suffix(rest) {
        return true;
    }

    // Try exponent: e/E followed by optional sign and digits
    if rest.starts_with('e') || rest.starts_with('E') {
        let mut j = 1;
        let rest_bytes = rest.as_bytes();
        if j < rest_bytes.len() && (rest_bytes[j] == b'+' || rest_bytes[j] == b'-') {
            j += 1;
        }
        let exp_digits_start = j;
        while j < rest_bytes.len() && rest_bytes[j].is_ascii_digit() {
            j += 1;
        }
        // Must have at least one digit after e/E (and optional sign)
        if j > exp_digits_start && j == rest_bytes.len() {
            return true;
        }
    }

    false
}

fn is_suffix(s: &str) -> bool {
    matches!(
        s,
        "n" | "u"
            | "m"
            | "k"
            | "M"
            | "G"
            | "T"
            | "P"
            | "E"
            | "Ki"
            | "Mi"
            | "Gi"
            | "Ti"
            | "Pi"
            | "Ei"
    )
}

// --- Path matching ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSegment {
    Field(String),
    Wildcard,
}

#[derive(Debug, Clone)]
pub struct QuantityPath {
    segments: Vec<PathSegment>,
}

impl QuantityPath {
    /// Parse a JSONPath-like pattern: `$.spec.containers[*].resources.limits[*]`
    pub fn parse(pattern: &str) -> Self {
        let stripped = pattern.strip_prefix("$.").unwrap_or(pattern);
        let mut segments = Vec::new();

        for part in stripped.split('.') {
            if part.is_empty() {
                continue;
            }
            if let Some(field) = part.strip_suffix("[*]") {
                if !field.is_empty() {
                    segments.push(PathSegment::Field(field.to_string()));
                }
                segments.push(PathSegment::Wildcard);
            } else {
                segments.push(PathSegment::Field(part.to_string()));
            }
        }

        Self { segments }
    }
}

/// Pre-parsed validation map: maps `<apiVersion>:<kind>` to quantity paths.
#[derive(Debug, Clone)]
pub struct ValidationMap {
    entries: HashMap<String, Vec<QuantityPath>>,
}

impl ValidationMap {
    /// Load from parsed `_validation.json` content.
    pub fn from_json(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;

        // Check version
        let version = obj.get("version")?.as_u64()?;
        if version != 1 {
            return None;
        }

        let quantities = obj.get("quantities")?.as_object()?;
        let mut entries = HashMap::new();

        for (key, paths_val) in quantities {
            let paths_arr = paths_val.as_array()?;
            let mut paths = Vec::new();
            for p in paths_arr {
                let pattern = p.as_str()?;
                paths.push(QuantityPath::parse(pattern));
            }
            entries.insert(key.clone(), paths);
        }

        Some(Self { entries })
    }

    fn paths_for(&self, api_version: &str, kind: &str) -> Option<&[QuantityPath]> {
        let key = format!("{api_version}:{kind}");
        self.entries.get(&key).map(|v| v.as_slice())
    }
}

/// A single quantity validation error.
#[derive(Debug)]
pub struct QuantityError {
    pub doc_index: usize,
    pub path: String,
    pub value: String,
}

impl std::fmt::Display for QuantityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "doc[{}] at {}: invalid quantity {:?}",
            self.doc_index, self.path, self.value
        )
    }
}

/// Validate all quantity fields in the build output.
///
/// `value` is the top-level array from `husako.build()`.
/// If `validation_map` is `Some`, uses schema-aware paths.
/// Otherwise, falls back to heuristic search for `resources.requests.*` / `resources.limits.*`.
pub fn validate_quantities(
    value: &Value,
    validation_map: Option<&ValidationMap>,
) -> Result<(), Vec<QuantityError>> {
    let docs = match value.as_array() {
        Some(arr) => arr,
        None => return Ok(()),
    };

    let mut errors = Vec::new();

    for (idx, doc) in docs.iter().enumerate() {
        if let Some(map) = validation_map {
            validate_doc_with_map(doc, idx, map, &mut errors);
        } else {
            validate_doc_fallback(doc, idx, &mut errors);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_doc_with_map(
    doc: &Value,
    doc_index: usize,
    map: &ValidationMap,
    errors: &mut Vec<QuantityError>,
) {
    let api_version = doc.get("apiVersion").and_then(Value::as_str).unwrap_or("");
    let kind = doc.get("kind").and_then(Value::as_str).unwrap_or("");

    if let Some(paths) = map.paths_for(api_version, kind) {
        for path in paths {
            walk_and_validate(doc, &path.segments, 0, "$", doc_index, errors);
        }
    } else {
        // Fall back to heuristic for unknown kinds
        validate_doc_fallback(doc, doc_index, errors);
    }
}

fn walk_and_validate(
    value: &Value,
    segments: &[PathSegment],
    seg_idx: usize,
    current_path: &str,
    doc_index: usize,
    errors: &mut Vec<QuantityError>,
) {
    if seg_idx >= segments.len() {
        // Reached a leaf — validate
        validate_leaf(value, current_path, doc_index, errors);
        return;
    }

    match &segments[seg_idx] {
        PathSegment::Field(name) => {
            if let Some(child) = value.get(name.as_str()) {
                let next_path = format!("{current_path}.{name}");
                walk_and_validate(child, segments, seg_idx + 1, &next_path, doc_index, errors);
            }
            // Field doesn't exist — skip silently (optional field)
        }
        PathSegment::Wildcard => match value {
            Value::Array(arr) => {
                for (i, item) in arr.iter().enumerate() {
                    let next_path = format!("{current_path}[{i}]");
                    walk_and_validate(item, segments, seg_idx + 1, &next_path, doc_index, errors);
                }
            }
            Value::Object(obj) => {
                for (key, child) in obj {
                    let next_path = format!("{current_path}.{key}");
                    walk_and_validate(child, segments, seg_idx + 1, &next_path, doc_index, errors);
                }
            }
            _ => {}
        },
    }
}

fn validate_leaf(value: &Value, path: &str, doc_index: usize, errors: &mut Vec<QuantityError>) {
    match value {
        Value::String(s) => {
            if !is_valid_quantity(s) {
                errors.push(QuantityError {
                    doc_index,
                    path: path.to_string(),
                    value: s.clone(),
                });
            }
        }
        Value::Number(_) | Value::Null => {
            // Valid: numbers are implicit quantities, null is optional
        }
        _ => {
            errors.push(QuantityError {
                doc_index,
                path: path.to_string(),
                value: format!("{value}"),
            });
        }
    }
}

// --- Fallback heuristic ---

/// Recursively searches for `resources.requests.*` and `resources.limits.*`
/// at any depth, validating leaf values as quantities.
fn validate_doc_fallback(doc: &Value, doc_index: usize, errors: &mut Vec<QuantityError>) {
    search_resources(doc, "$", doc_index, errors);
}

fn search_resources(
    value: &Value,
    current_path: &str,
    doc_index: usize,
    errors: &mut Vec<QuantityError>,
) {
    let obj = match value.as_object() {
        Some(o) => o,
        None => return,
    };

    // Check if this object has a "resources" key
    if let Some(resources) = obj.get("resources")
        && let Some(res_obj) = resources.as_object()
    {
        let res_path = format!("{current_path}.resources");
        for target in &["requests", "limits"] {
            if let Some(target_val) = res_obj.get(*target) {
                let target_path = format!("{res_path}.{target}");
                validate_quantity_map(target_val, &target_path, doc_index, errors);
            }
        }
    }

    // Recurse into all object/array children
    for (key, child) in obj {
        let child_path = format!("{current_path}.{key}");
        match child {
            Value::Object(_) => {
                search_resources(child, &child_path, doc_index, errors);
            }
            Value::Array(arr) => {
                for (i, item) in arr.iter().enumerate() {
                    let item_path = format!("{child_path}[{i}]");
                    search_resources(item, &item_path, doc_index, errors);
                }
            }
            _ => {}
        }
    }
}

fn validate_quantity_map(
    value: &Value,
    path: &str,
    doc_index: usize,
    errors: &mut Vec<QuantityError>,
) {
    if let Some(obj) = value.as_object() {
        for (key, val) in obj {
            let leaf_path = format!("{path}.{key}");
            validate_leaf(val, &leaf_path, doc_index, errors);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- is_valid_quantity ---

    #[test]
    fn valid_bare_number() {
        assert!(is_valid_quantity("1"));
        assert!(is_valid_quantity("0"));
        assert!(is_valid_quantity("100"));
    }

    #[test]
    fn valid_decimal() {
        assert!(is_valid_quantity(".5"));
        assert!(is_valid_quantity("2.5"));
        assert!(is_valid_quantity("0.1"));
    }

    #[test]
    fn valid_with_sign() {
        assert!(is_valid_quantity("+1"));
        assert!(is_valid_quantity("-1"));
    }

    #[test]
    fn valid_millicores() {
        assert!(is_valid_quantity("500m"));
        assert!(is_valid_quantity("100m"));
    }

    #[test]
    fn valid_binary_si() {
        assert!(is_valid_quantity("1Gi"));
        assert!(is_valid_quantity("100Mi"));
        assert!(is_valid_quantity("2.5Gi"));
        assert!(is_valid_quantity("1Ki"));
    }

    #[test]
    fn valid_decimal_si() {
        assert!(is_valid_quantity("1k"));
        assert!(is_valid_quantity("1M"));
        assert!(is_valid_quantity("1G"));
        assert!(is_valid_quantity("1n"));
        assert!(is_valid_quantity("1u"));
    }

    #[test]
    fn valid_exa_suffix() {
        // "1E" should be Exa suffix, not an incomplete exponent
        assert!(is_valid_quantity("1E"));
        assert!(is_valid_quantity("1Ei"));
    }

    #[test]
    fn valid_exponent() {
        assert!(is_valid_quantity("1e3"));
        assert!(is_valid_quantity("1E3"));
        assert!(is_valid_quantity("1e+3"));
        assert!(is_valid_quantity("1e-3"));
    }

    #[test]
    fn invalid_empty() {
        assert!(!is_valid_quantity(""));
    }

    #[test]
    fn invalid_no_digits() {
        assert!(!is_valid_quantity("abc"));
        assert!(!is_valid_quantity("Gi"));
        assert!(!is_valid_quantity("e3"));
    }

    #[test]
    fn invalid_wrong_suffix() {
        assert!(!is_valid_quantity("2gb"));
        assert!(!is_valid_quantity("1gi")); // lowercase gi is not valid
        assert!(!is_valid_quantity("1mm")); // double m
    }

    #[test]
    fn invalid_space() {
        assert!(!is_valid_quantity("1 Gi"));
    }

    #[test]
    fn invalid_multiple_dots() {
        assert!(!is_valid_quantity("1.2.3"));
    }

    // --- QuantityPath ---

    #[test]
    fn parse_simple_path() {
        let p = QuantityPath::parse("$.spec.replicas");
        assert_eq!(p.segments.len(), 2);
        assert_eq!(p.segments[0], PathSegment::Field("spec".into()));
        assert_eq!(p.segments[1], PathSegment::Field("replicas".into()));
    }

    #[test]
    fn parse_wildcard_path() {
        let p = QuantityPath::parse("$.spec.containers[*].resources.limits[*]");
        assert_eq!(
            p.segments,
            vec![
                PathSegment::Field("spec".into()),
                PathSegment::Field("containers".into()),
                PathSegment::Wildcard,
                PathSegment::Field("resources".into()),
                PathSegment::Field("limits".into()),
                PathSegment::Wildcard,
            ]
        );
    }

    // --- walk_and_validate ---

    #[test]
    fn walk_field_path() {
        let doc = json!({"spec": {"capacity": {"storage": "10Gi"}}});
        let path = QuantityPath::parse("$.spec.capacity.storage");
        let mut errors = Vec::new();
        walk_and_validate(&doc, &path.segments, 0, "$", 0, &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn walk_invalid_field() {
        let doc = json!({"spec": {"capacity": {"storage": "10gb"}}});
        let path = QuantityPath::parse("$.spec.capacity.storage");
        let mut errors = Vec::new();
        walk_and_validate(&doc, &path.segments, 0, "$", 0, &mut errors);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].path, "$.spec.capacity.storage");
        assert_eq!(errors[0].value, "10gb");
    }

    #[test]
    fn walk_wildcard_on_map() {
        let doc = json!({"resources": {"limits": {"cpu": "500m", "memory": "1Gi"}}});
        let path = QuantityPath::parse("$.resources.limits[*]");
        let mut errors = Vec::new();
        walk_and_validate(&doc, &path.segments, 0, "$", 0, &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn walk_wildcard_on_array() {
        let doc = json!({
            "spec": {
                "containers": [
                    {"resources": {"limits": {"cpu": "500m"}}},
                    {"resources": {"limits": {"cpu": "bad"}}}
                ]
            }
        });
        let path = QuantityPath::parse("$.spec.containers[*].resources.limits[*]");
        let mut errors = Vec::new();
        walk_and_validate(&doc, &path.segments, 0, "$", 0, &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].path.contains("[1]"));
        assert_eq!(errors[0].value, "bad");
    }

    #[test]
    fn walk_missing_field_is_ok() {
        let doc = json!({"spec": {}});
        let path = QuantityPath::parse("$.spec.capacity.storage");
        let mut errors = Vec::new();
        walk_and_validate(&doc, &path.segments, 0, "$", 0, &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn number_at_quantity_position_is_valid() {
        let doc = json!({"resources": {"limits": {"cpu": 1}}});
        let path = QuantityPath::parse("$.resources.limits[*]");
        let mut errors = Vec::new();
        walk_and_validate(&doc, &path.segments, 0, "$", 0, &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn null_at_quantity_position_is_valid() {
        let doc = json!({"resources": {"limits": {"cpu": null}}});
        let path = QuantityPath::parse("$.resources.limits[*]");
        let mut errors = Vec::new();
        walk_and_validate(&doc, &path.segments, 0, "$", 0, &mut errors);
        assert!(errors.is_empty());
    }

    // --- Fallback heuristic ---

    #[test]
    fn fallback_validates_resources() {
        let doc = json!({
            "spec": {
                "template": {
                    "spec": {
                        "containers": [{
                            "resources": {
                                "requests": {"cpu": "bad"},
                                "limits": {"memory": "1Gi"}
                            }
                        }]
                    }
                }
            }
        });
        let mut errors = Vec::new();
        validate_doc_fallback(&doc, 0, &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].path.contains("requests.cpu"));
        assert_eq!(errors[0].value, "bad");
    }

    #[test]
    fn fallback_valid_resources() {
        let doc = json!({
            "spec": {
                "template": {
                    "spec": {
                        "containers": [{
                            "resources": {
                                "requests": {"cpu": "500m", "memory": "1Gi"},
                                "limits": {"cpu": "1", "memory": "2Gi"}
                            }
                        }]
                    }
                }
            }
        });
        let mut errors = Vec::new();
        validate_doc_fallback(&doc, 0, &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn fallback_no_resources_is_ok() {
        let doc = json!({"apiVersion": "v1", "kind": "Namespace", "metadata": {"name": "test"}});
        let mut errors = Vec::new();
        validate_doc_fallback(&doc, 0, &mut errors);
        assert!(errors.is_empty());
    }

    // --- ValidationMap ---

    #[test]
    fn validation_map_from_json() {
        let json = json!({
            "version": 1,
            "quantities": {
                "apps/v1:Deployment": [
                    "$.spec.template.spec.containers[*].resources.limits[*]",
                    "$.spec.template.spec.containers[*].resources.requests[*]"
                ]
            }
        });
        let map = ValidationMap::from_json(&json).unwrap();
        let paths = map.paths_for("apps/v1", "Deployment").unwrap();
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn validation_map_unknown_kind_uses_fallback() {
        let json = json!({
            "version": 1,
            "quantities": {}
        });
        let map = ValidationMap::from_json(&json).unwrap();
        assert!(map.paths_for("v1", "Namespace").is_none());
    }

    // --- validate_quantities (integration) ---

    #[test]
    fn validate_quantities_schema_aware() {
        let map_json = json!({
            "version": 1,
            "quantities": {
                "v1:PersistentVolume": ["$.spec.capacity[*]"]
            }
        });
        let map = ValidationMap::from_json(&map_json).unwrap();

        let value = json!([{
            "apiVersion": "v1",
            "kind": "PersistentVolume",
            "spec": {"capacity": {"storage": "10Gi"}}
        }]);
        assert!(validate_quantities(&value, Some(&map)).is_ok());

        let bad = json!([{
            "apiVersion": "v1",
            "kind": "PersistentVolume",
            "spec": {"capacity": {"storage": "10gb"}}
        }]);
        let errs = validate_quantities(&bad, Some(&map)).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].path.contains("capacity.storage"));
    }

    #[test]
    fn validate_quantities_fallback() {
        let value = json!([{
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
        let errs = validate_quantities(&value, None).unwrap_err();
        assert_eq!(errs.len(), 1);
    }
}
