use std::path::Path;

use crate::HelmError;

/// Resolve a Helm chart from the ArtifactHub API.
///
/// Flow:
/// 1. Check cache
/// 2. Fetch `https://artifacthub.io/api/v1/packages/helm/{package}/{version}`
/// 3. Extract `values_schema` field from response JSON
/// 4. Cache and return
pub fn resolve(
    name: &str,
    package: &str,
    version: &str,
    cache_dir: &Path,
) -> Result<serde_json::Value, HelmError> {
    // Check cache
    let cache_key = crate::cache_hash(package);
    let cache_path = cache_dir.join(format!("helm/artifacthub/{cache_key}/{version}.json"));
    if cache_path.exists() {
        let content = std::fs::read_to_string(&cache_path).map_err(|e| {
            HelmError::Io(format!(
                "chart '{name}': read cache {}: {e}",
                cache_path.display()
            ))
        })?;
        return serde_json::from_str(&content).map_err(|e| {
            HelmError::InvalidSchema(format!("chart '{name}': parse cached schema: {e}"))
        });
    }

    // Fetch from ArtifactHub API
    let url = format!("https://artifacthub.io/api/v1/packages/helm/{package}/{version}");
    let resp = reqwest::blocking::get(&url)
        .map_err(|e| HelmError::Io(format!("chart '{name}': fetch {url}: {e}")))?;

    if !resp.status().is_success() {
        return Err(HelmError::Io(format!(
            "chart '{name}': fetch {url} returned {}",
            resp.status()
        )));
    }

    let body: serde_json::Value = resp
        .json()
        .map_err(|e| HelmError::Io(format!("chart '{name}': parse response from {url}: {e}")))?;

    // The `values_schema` field contains the JSON string of the schema
    let schema_str = body
        .get("values_schema")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            HelmError::NotFound(format!(
                "chart '{name}': package does not include values_schema"
            ))
        })?;

    let schema: serde_json::Value = serde_json::from_str(schema_str).map_err(|e| {
        HelmError::InvalidSchema(format!("chart '{name}': parse values_schema: {e}"))
    })?;

    // Cache
    if let Some(parent) = cache_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        &cache_path,
        serde_json::to_string_pretty(&schema).unwrap_or_default(),
    );

    Ok(schema)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_hit_returns_cached_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path();
        let cache_key = crate::cache_hash("my-org/my-chart");
        let cache_sub = cache_dir.join(format!("helm/artifacthub/{cache_key}"));
        std::fs::create_dir_all(&cache_sub).unwrap();
        std::fs::write(
            cache_sub.join("1.0.0.json"),
            r#"{"type":"object","properties":{"replicas":{"type":"integer"}}}"#,
        )
        .unwrap();

        let result = resolve("test", "my-org/my-chart", "1.0.0", cache_dir).unwrap();
        assert_eq!(result["type"], "object");
        assert!(result["properties"]["replicas"].is_object());
    }

    #[test]
    fn cache_invalid_json_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path();
        let cache_key = crate::cache_hash("my-org/my-chart");
        let cache_sub = cache_dir.join(format!("helm/artifacthub/{cache_key}"));
        std::fs::create_dir_all(&cache_sub).unwrap();
        std::fs::write(cache_sub.join("1.0.0.json"), "not json").unwrap();

        let err = resolve("test", "my-org/my-chart", "1.0.0", cache_dir).unwrap_err();
        assert!(err.to_string().contains("parse cached schema"));
    }
}
