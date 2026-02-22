mod artifacthub;
mod file;
mod git;
mod registry;

use std::collections::HashMap;
use std::path::Path;

use husako_config::ChartSource;

#[derive(Debug, thiserror::Error)]
pub enum HelmError {
    #[error("chart I/O error: {0}")]
    Io(String),
    #[error("invalid values schema: {0}")]
    InvalidSchema(String),
    #[error("chart not found: {0}")]
    NotFound(String),
}

/// Simple hash for cache directory naming (djb2).
pub(crate) fn cache_hash(s: &str) -> String {
    let mut hash: u64 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    format!("{hash:016x}")
}

/// Resolve a single chart source to its `values.schema.json` content.
pub fn resolve(
    name: &str,
    source: &ChartSource,
    project_root: &Path,
    cache_dir: &Path,
) -> Result<serde_json::Value, HelmError> {
    match source {
        ChartSource::File { path } => file::resolve(name, path, project_root),
        ChartSource::Registry {
            repo,
            chart,
            version,
        } => registry::resolve(name, repo, chart, version, cache_dir),
        ChartSource::ArtifactHub { package, version } => {
            artifacthub::resolve(name, package, version, cache_dir)
        }
        ChartSource::Git { repo, tag, path } => git::resolve(name, repo, tag, path, cache_dir),
    }
}

/// Resolve all chart sources from config.
///
/// Returns `chart_name → JSON Schema value`.
pub fn resolve_all(
    charts: &HashMap<String, ChartSource>,
    project_root: &Path,
    cache_dir: &Path,
) -> Result<HashMap<String, serde_json::Value>, HelmError> {
    let mut result = HashMap::new();
    for (name, source) in charts {
        let schema = resolve(name, source, project_root, cache_dir)?;
        result.insert(name.clone(), schema);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_hash_deterministic() {
        let h1 = cache_hash("https://kubernetes.github.io/ingress-nginx");
        let h2 = cache_hash("https://kubernetes.github.io/ingress-nginx");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16);
    }

    #[test]
    fn cache_hash_different_inputs() {
        let h1 = cache_hash("repo-a");
        let h2 = cache_hash("repo-b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn resolve_all_empty() {
        let charts = HashMap::new();
        let tmp = tempfile::tempdir().unwrap();
        let result = resolve_all(&charts, tmp.path(), &tmp.path().join("cache")).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn resolve_all_file_source() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("values.schema.json"),
            r#"{"type": "object", "properties": {"replicas": {"type": "integer"}}}"#,
        )
        .unwrap();

        let mut charts = HashMap::new();
        charts.insert(
            "my-chart".to_string(),
            ChartSource::File {
                path: "values.schema.json".to_string(),
            },
        );

        let result = resolve_all(&charts, tmp.path(), &tmp.path().join("cache")).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("my-chart"));
    }
}
