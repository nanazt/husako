use std::path::Path;

use crate::HelmError;

/// Resolve a Helm chart from a git repository.
///
/// Flow:
/// 1. Check cache
/// 2. Shallow-clone the repo at the specified tag
/// 3. Read `values.schema.json` from the specified path within the repo
/// 4. Cache and return
pub async fn resolve(
    name: &str,
    repo: &str,
    tag: &str,
    path: &str,
    cache_dir: &Path,
) -> Result<serde_json::Value, HelmError> {
    // Check cache
    let cache_key = crate::cache_hash(&format!("{repo}/{path}"));
    let cache_path = cache_dir.join(format!("helm/git/{cache_key}/{tag}.json"));
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

    // Clone repo at specific tag
    let temp_dir = tempfile::tempdir()
        .map_err(|e| HelmError::Io(format!("chart '{name}': create temp dir: {e}")))?;

    let output = tokio::process::Command::new("git")
        .args(["clone", "--depth", "1", "--branch", tag, repo])
        .arg(temp_dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|e| HelmError::Io(format!("chart '{name}': git clone failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(HelmError::Io(format!(
            "chart '{name}': git clone {repo} at tag {tag} failed: {stderr}"
        )));
    }

    // Read values.schema.json from the specified path
    let schema_path = temp_dir.path().join(path);
    if !schema_path.exists() {
        return Err(HelmError::NotFound(format!(
            "chart '{name}': path '{path}' not found in repository {repo} at tag {tag}"
        )));
    }

    let content = std::fs::read_to_string(&schema_path).map_err(|e| {
        HelmError::Io(format!(
            "chart '{name}': read {}: {e}",
            schema_path.display()
        ))
    })?;

    let schema: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        HelmError::InvalidSchema(format!("chart '{name}': parse values.schema.json: {e}"))
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

    #[tokio::test]
    async fn cache_hit_returns_cached_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path();
        let cache_key = crate::cache_hash(
            "https://github.com/example/chart/charts/my-chart/values.schema.json",
        );
        let cache_sub = cache_dir.join(format!("helm/git/{cache_key}"));
        std::fs::create_dir_all(&cache_sub).unwrap();
        std::fs::write(
            cache_sub.join("v1.0.0.json"),
            r#"{"type":"object","properties":{"replicas":{"type":"integer"}}}"#,
        )
        .unwrap();

        let result = resolve(
            "test",
            "https://github.com/example/chart",
            "v1.0.0",
            "charts/my-chart/values.schema.json",
            cache_dir,
        )
        .await
        .unwrap();
        assert_eq!(result["type"], "object");
        assert!(result["properties"]["replicas"].is_object());
    }

    #[tokio::test]
    async fn cache_invalid_json_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path();
        let cache_key = crate::cache_hash(
            "https://github.com/example/chart/charts/my-chart/values.schema.json",
        );
        let cache_sub = cache_dir.join(format!("helm/git/{cache_key}"));
        std::fs::create_dir_all(&cache_sub).unwrap();
        std::fs::write(cache_sub.join("v1.0.0.json"), "not json").unwrap();

        let err = resolve(
            "test",
            "https://github.com/example/chart",
            "v1.0.0",
            "charts/my-chart/values.schema.json",
            cache_dir,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("parse cached schema"));
    }
}
