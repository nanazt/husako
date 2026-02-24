mod artifacthub;
mod file;
mod git;
pub mod oci;
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
        ChartSource::Oci { reference, version } => {
            let chart = crate::oci::chart_name_from_reference(reference);
            crate::oci::resolve(name, reference, chart, version, cache_dir)
        }
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
    fn resolve_oci_uses_chart_name_from_reference() {
        // Verify the dispatch builds cache path using the last path component as chart name.
        // We use the cache-hit path to avoid network access.
        let tmp = tempfile::tempdir().unwrap();
        let reference = "oci://registry-1.docker.io/bitnamicharts/postgresql";
        let chart = crate::oci::chart_name_from_reference(reference);
        assert_eq!(chart, "postgresql");

        // Seed cache
        let cache_key = cache_hash(reference);
        let cache_sub = tmp.path().join(format!("helm/oci/{cache_key}"));
        std::fs::create_dir_all(&cache_sub).unwrap();
        std::fs::write(
            cache_sub.join("16.4.0.json"),
            r#"{"type":"object","properties":{"replicaCount":{"type":"integer"}}}"#,
        )
        .unwrap();

        let source = husako_config::ChartSource::Oci {
            reference: reference.to_string(),
            version: "16.4.0".to_string(),
        };
        let result = resolve("test", &source, tmp.path(), tmp.path()).unwrap();
        assert_eq!(result["type"], "object");
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
