use std::path::Path;

use crate::HelmError;

/// Resolve a file-based chart source.
///
/// Reads a local `values.schema.json` file and validates it as JSON.
pub fn resolve(
    name: &str,
    path: &str,
    project_root: &Path,
) -> Result<serde_json::Value, HelmError> {
    let resolved = project_root.join(path);

    if !resolved.exists() {
        return Err(HelmError::NotFound(format!(
            "chart '{name}': file not found: {}",
            resolved.display()
        )));
    }

    let content = std::fs::read_to_string(&resolved)
        .map_err(|e| HelmError::Io(format!("chart '{name}': read {}: {e}", resolved.display())))?;

    let schema: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        HelmError::InvalidSchema(format!(
            "chart '{name}': invalid JSON in {}: {e}",
            resolved.display()
        ))
    })?;

    // Basic validation: must be an object type
    if schema.get("type").and_then(|t| t.as_str()) != Some("object")
        && schema.get("properties").is_none()
    {
        return Err(HelmError::InvalidSchema(format!(
            "chart '{name}': schema must have type \"object\" or \"properties\""
        )));
    }

    Ok(schema)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_valid_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let schema = r#"{
            "type": "object",
            "properties": {
                "replicaCount": { "type": "integer", "default": 1 },
                "image": {
                    "type": "object",
                    "properties": {
                        "repository": { "type": "string" },
                        "tag": { "type": "string" }
                    }
                }
            }
        }"#;
        std::fs::write(tmp.path().join("values.schema.json"), schema).unwrap();

        let result = resolve("test", "values.schema.json", tmp.path()).unwrap();
        assert_eq!(result["type"], "object");
        assert!(result["properties"]["replicaCount"].is_object());
    }

    #[test]
    fn resolve_file_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let err = resolve("test", "missing.json", tmp.path()).unwrap_err();
        assert!(matches!(err, HelmError::NotFound(_)));
        assert!(err.to_string().contains("file not found"));
    }

    #[test]
    fn resolve_invalid_json() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("bad.json"), "not json").unwrap();
        let err = resolve("test", "bad.json", tmp.path()).unwrap_err();
        assert!(matches!(err, HelmError::InvalidSchema(_)));
    }

    #[test]
    fn resolve_non_object_schema() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("bad.json"), r#"{"type": "string"}"#).unwrap();
        let err = resolve("test", "bad.json", tmp.path()).unwrap_err();
        assert!(matches!(err, HelmError::InvalidSchema(_)));
    }

    #[test]
    fn resolve_schema_with_only_properties() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("schema.json"),
            r#"{"properties": {"name": {"type": "string"}}}"#,
        )
        .unwrap();
        let result = resolve("test", "schema.json", tmp.path()).unwrap();
        assert!(result["properties"]["name"].is_object());
    }
}
