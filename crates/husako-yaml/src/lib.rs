#[derive(Debug, thiserror::Error)]
pub enum EmitError {
    #[error("YAML serialization failed: {0}")]
    Serialize(String),
}

pub fn emit_yaml(value: &serde_json::Value) -> Result<String, EmitError> {
    match value {
        serde_json::Value::Array(docs) => {
            let mut parts = Vec::with_capacity(docs.len());
            for doc in docs {
                let yaml = serde_yaml_ng::to_string(doc)
                    .map_err(|e| EmitError::Serialize(e.to_string()))?;
                parts.push(yaml);
            }
            Ok(parts.join("---\n"))
        }
        _ => serde_yaml_ng::to_string(value).map_err(|e| EmitError::Serialize(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn single_object() {
        let val = json!({"apiVersion": "v1", "kind": "Namespace"});
        let yaml = emit_yaml(&val).unwrap();
        assert!(yaml.contains("apiVersion: v1"));
        assert!(yaml.contains("kind: Namespace"));
    }

    #[test]
    fn multi_document() {
        let val = json!([
            {"apiVersion": "v1", "kind": "Namespace"},
            {"apiVersion": "v1", "kind": "Service"}
        ]);
        let yaml = emit_yaml(&val).unwrap();
        assert!(yaml.contains("---"));
        assert!(yaml.contains("Namespace"));
        assert!(yaml.contains("Service"));
    }

    #[test]
    fn empty_array() {
        let val = json!([]);
        let yaml = emit_yaml(&val).unwrap();
        assert!(yaml.is_empty());
    }
}
