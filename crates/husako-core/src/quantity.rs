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

/// A single quantity validation error (used by the fallback heuristic).
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

// --- Fallback heuristic ---

/// Recursively searches for `resources.requests.*` and `resources.limits.*`
/// at any depth, validating leaf values as quantities.
pub(crate) fn validate_doc_fallback(
    doc: &Value,
    doc_index: usize,
    errors: &mut Vec<QuantityError>,
) {
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
}
