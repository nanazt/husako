use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::{DiscoveryIndex, OpenApiError};

/// Sanitize a base URL into a filesystem-safe directory name.
/// e.g. "https://localhost:6443" → "localhost_6443"
pub fn server_key(base_url: &str) -> String {
    let stripped = base_url
        .strip_prefix("https://")
        .or_else(|| base_url.strip_prefix("http://"))
        .unwrap_or(base_url);
    stripped
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Convert a group-version path to a filename.
/// e.g. "apis/apps/v1" → "apis__apps__v1.json"
pub fn spec_filename(group_version: &str) -> String {
    let sanitized = group_version.replace('/', "__");
    format!("{sanitized}.json")
}

fn cache_dir(base: &Path, key: &str) -> PathBuf {
    base.join(key)
}

fn specs_dir(base: &Path, key: &str) -> PathBuf {
    cache_dir(base, key).join("specs")
}

pub fn read_discovery(cache_dir_path: &Path, key: &str) -> Result<DiscoveryIndex, OpenApiError> {
    let path = cache_dir(cache_dir_path, key).join("discovery.json");
    let data = std::fs::read_to_string(&path)
        .map_err(|e| OpenApiError::Cache(format!("read {}: {e}", path.display())))?;
    serde_json::from_str(&data)
        .map_err(|e| OpenApiError::Parse(format!("parse {}: {e}", path.display())))
}

pub fn write_discovery(
    cache_dir_path: &Path,
    key: &str,
    index: &DiscoveryIndex,
) -> Result<(), OpenApiError> {
    let dir = cache_dir(cache_dir_path, key);
    std::fs::create_dir_all(&dir)
        .map_err(|e| OpenApiError::Cache(format!("create {}: {e}", dir.display())))?;
    let path = dir.join("discovery.json");
    let data = serde_json::to_string_pretty(index)
        .map_err(|e| OpenApiError::Parse(format!("serialize discovery: {e}")))?;
    std::fs::write(&path, data)
        .map_err(|e| OpenApiError::Cache(format!("write {}: {e}", path.display())))
}

pub fn read_hashes(
    cache_dir_path: &Path,
    key: &str,
) -> Result<HashMap<String, String>, OpenApiError> {
    let path = cache_dir(cache_dir_path, key).join("hashes.json");
    let data = std::fs::read_to_string(&path)
        .map_err(|e| OpenApiError::Cache(format!("read {}: {e}", path.display())))?;
    serde_json::from_str(&data)
        .map_err(|e| OpenApiError::Parse(format!("parse {}: {e}", path.display())))
}

pub fn write_hashes(
    cache_dir_path: &Path,
    key: &str,
    hashes: &HashMap<String, String>,
) -> Result<(), OpenApiError> {
    let dir = cache_dir(cache_dir_path, key);
    std::fs::create_dir_all(&dir)
        .map_err(|e| OpenApiError::Cache(format!("create {}: {e}", dir.display())))?;
    let path = dir.join("hashes.json");
    let data = serde_json::to_string_pretty(hashes)
        .map_err(|e| OpenApiError::Parse(format!("serialize hashes: {e}")))?;
    std::fs::write(&path, data)
        .map_err(|e| OpenApiError::Cache(format!("write {}: {e}", path.display())))
}

pub fn read_spec(
    cache_dir_path: &Path,
    key: &str,
    group_version: &str,
) -> Result<serde_json::Value, OpenApiError> {
    let path = specs_dir(cache_dir_path, key).join(spec_filename(group_version));
    let data = std::fs::read_to_string(&path)
        .map_err(|e| OpenApiError::Cache(format!("read {}: {e}", path.display())))?;
    serde_json::from_str(&data)
        .map_err(|e| OpenApiError::Parse(format!("parse {}: {e}", path.display())))
}

pub fn write_spec(
    cache_dir_path: &Path,
    key: &str,
    group_version: &str,
    spec: &serde_json::Value,
) -> Result<(), OpenApiError> {
    let dir = specs_dir(cache_dir_path, key);
    std::fs::create_dir_all(&dir)
        .map_err(|e| OpenApiError::Cache(format!("create {}: {e}", dir.display())))?;
    let path = dir.join(spec_filename(group_version));
    let data = serde_json::to_string_pretty(spec)
        .map_err(|e| OpenApiError::Parse(format!("serialize spec: {e}")))?;
    std::fs::write(&path, data)
        .map_err(|e| OpenApiError::Cache(format!("write {}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_key_strips_scheme_and_sanitizes() {
        assert_eq!(server_key("https://localhost:6443"), "localhost_6443");
        assert_eq!(server_key("http://10.0.0.1:8080"), "10.0.0.1_8080");
        assert_eq!(
            server_key("https://my-cluster.example.com"),
            "my-cluster.example.com"
        );
    }

    #[test]
    fn spec_filename_replaces_slashes() {
        assert_eq!(spec_filename("api/v1"), "api__v1.json");
        assert_eq!(spec_filename("apis/apps/v1"), "apis__apps__v1.json");
    }

    #[test]
    fn discovery_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let index = DiscoveryIndex {
            paths: HashMap::from([(
                "api/v1".to_string(),
                crate::DiscoveryPath {
                    server_relative_url: "/openapi/v3/api/v1?hash=ABC".to_string(),
                },
            )]),
        };
        write_discovery(tmp.path(), "test_server", &index).unwrap();
        let loaded = read_discovery(tmp.path(), "test_server").unwrap();
        assert_eq!(loaded.paths.len(), 1);
        assert_eq!(
            loaded.paths["api/v1"].server_relative_url,
            "/openapi/v3/api/v1?hash=ABC"
        );
    }

    #[test]
    fn hashes_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let hashes = HashMap::from([("api/v1".to_string(), "ABC123".to_string())]);
        write_hashes(tmp.path(), "test_server", &hashes).unwrap();
        let loaded = read_hashes(tmp.path(), "test_server").unwrap();
        assert_eq!(loaded["api/v1"], "ABC123");
    }

    #[test]
    fn spec_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = serde_json::json!({"openapi": "3.0.0"});
        write_spec(tmp.path(), "test_server", "apis/apps/v1", &spec).unwrap();
        let loaded = read_spec(tmp.path(), "test_server", "apis/apps/v1").unwrap();
        assert_eq!(loaded, spec);
    }

    #[test]
    fn read_missing_file_returns_cache_error() {
        let tmp = tempfile::tempdir().unwrap();
        let err = read_discovery(tmp.path(), "nonexistent").unwrap_err();
        assert!(matches!(err, OpenApiError::Cache(_)));
    }
}
