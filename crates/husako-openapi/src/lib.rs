mod cache;
pub mod crd;
mod fetch;
pub mod kubeconfig;
pub mod release;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum OpenApiError {
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("failed to read/write cache: {0}")]
    Cache(String),
    #[error("failed to parse OpenAPI response: {0}")]
    Parse(String),
    #[error("group-version not found: {0}")]
    NotFound(String),
    #[error("no cached data available for offline use")]
    NoCachedData,
    #[error("CRD parse error: {0}")]
    Crd(String),
    #[error("kubeconfig error: {0}")]
    Kubeconfig(String),
    #[error("GitHub release error: {0}")]
    Release(String),
}

pub enum OpenApiSource {
    Url {
        base_url: String,
        bearer_token: Option<String>,
    },
    Directory(PathBuf),
}

pub struct FetchOptions {
    pub source: OpenApiSource,
    pub cache_dir: PathBuf,
    pub offline: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryIndex {
    pub paths: HashMap<String, DiscoveryPath>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryPath {
    #[serde(rename = "serverRelativeURL")]
    pub server_relative_url: String,
}

pub struct OpenApiClient {
    http_client: Option<reqwest::blocking::Client>,
    base_url: Option<String>,
    bearer_token: Option<String>,
    directory: Option<PathBuf>,
    cache_dir: PathBuf,
    offline: bool,
}

impl OpenApiClient {
    pub fn new(options: FetchOptions) -> Result<Self, OpenApiError> {
        match options.source {
            OpenApiSource::Url {
                base_url,
                bearer_token,
            } => {
                let client = reqwest::blocking::Client::builder()
                    .timeout(Duration::from_secs(30))
                    .build()
                    .map_err(|e| OpenApiError::Http(format!("failed to build HTTP client: {e}")))?;
                Ok(Self {
                    http_client: Some(client),
                    base_url: Some(base_url),
                    bearer_token,
                    directory: None,
                    cache_dir: options.cache_dir,
                    offline: options.offline,
                })
            }
            OpenApiSource::Directory(path) => Ok(Self {
                http_client: None,
                base_url: None,
                bearer_token: None,
                directory: Some(path),
                cache_dir: options.cache_dir,
                offline: true,
            }),
        }
    }

    pub fn discover(&self) -> Result<DiscoveryIndex, OpenApiError> {
        if let Some(dir) = &self.directory {
            return self.discover_from_directory(dir);
        }

        let base_url = self.base_url.as_deref().unwrap();
        let key = cache::server_key(base_url);

        if self.offline {
            return cache::read_discovery(&self.cache_dir, &key)
                .map_err(|_| OpenApiError::NoCachedData);
        }

        let client = self.http_client.as_ref().unwrap();
        let token = self.bearer_token.as_deref();

        match fetch::fetch_discovery(client, base_url, token) {
            Ok(index) => {
                let _ = cache::write_discovery(&self.cache_dir, &key, &index);
                Ok(index)
            }
            Err(e) => {
                // Fallback to cache on network failure
                cache::read_discovery(&self.cache_dir, &key).map_err(|_| e)
            }
        }
    }

    pub fn fetch_spec(&self, group_version: &str) -> Result<serde_json::Value, OpenApiError> {
        if let Some(dir) = &self.directory {
            return self.read_spec_from_directory(dir, group_version);
        }

        let base_url = self.base_url.as_deref().unwrap();
        let key = cache::server_key(base_url);

        if self.offline {
            return cache::read_spec(&self.cache_dir, &key, group_version)
                .map_err(|_| OpenApiError::NoCachedData);
        }

        // Get discovery to find the server-relative URL and current hash
        let index = self.discover()?;
        let discovery_path = index
            .paths
            .get(group_version)
            .ok_or_else(|| OpenApiError::NotFound(group_version.to_string()))?;

        let new_hash = fetch::extract_hash(&discovery_path.server_relative_url);

        // Check if cached hash matches
        if let Some(ref new_h) = new_hash
            && let Ok(cached_hashes) = cache::read_hashes(&self.cache_dir, &key)
            && cached_hashes.get(group_version) == Some(new_h)
            && let Ok(spec) = cache::read_spec(&self.cache_dir, &key, group_version)
        {
            return Ok(spec);
        }

        // Fetch from server
        let client = self.http_client.as_ref().unwrap();
        let token = self.bearer_token.as_deref();

        match fetch::fetch_spec(client, base_url, &discovery_path.server_relative_url, token) {
            Ok(spec) => {
                let _ = cache::write_spec(&self.cache_dir, &key, group_version, &spec);
                // Update hash
                if let Some(new_h) = new_hash {
                    let mut hashes = cache::read_hashes(&self.cache_dir, &key).unwrap_or_default();
                    hashes.insert(group_version.to_string(), new_h);
                    let _ = cache::write_hashes(&self.cache_dir, &key, &hashes);
                }
                Ok(spec)
            }
            Err(e) => {
                // Fallback to cache
                cache::read_spec(&self.cache_dir, &key, group_version).map_err(|_| e)
            }
        }
    }

    pub fn fetch_all_specs(&self) -> Result<HashMap<String, serde_json::Value>, OpenApiError> {
        let index = self.discover()?;
        let mut specs = HashMap::new();
        for group_version in index.paths.keys() {
            let spec = self.fetch_spec(group_version)?;
            specs.insert(group_version.clone(), spec);
        }
        Ok(specs)
    }

    fn discover_from_directory(
        &self,
        dir: &std::path::Path,
    ) -> Result<DiscoveryIndex, OpenApiError> {
        let discovery_path = dir.join("discovery.json");
        if discovery_path.exists() {
            let data = std::fs::read_to_string(&discovery_path).map_err(|e| {
                OpenApiError::Cache(format!("read {}: {e}", discovery_path.display()))
            })?;
            return serde_json::from_str(&data).map_err(|e| {
                OpenApiError::Parse(format!("parse {}: {e}", discovery_path.display()))
            });
        }

        // Build discovery from spec files in the directory
        let mut paths = HashMap::new();
        self.scan_spec_files(dir, dir, &mut paths)?;
        Ok(DiscoveryIndex { paths })
    }

    fn scan_spec_files(
        &self,
        base: &std::path::Path,
        dir: &std::path::Path,
        paths: &mut HashMap<String, DiscoveryPath>,
    ) -> Result<(), OpenApiError> {
        let entries = std::fs::read_dir(dir)
            .map_err(|e| OpenApiError::Cache(format!("read dir {}: {e}", dir.display())))?;
        for entry in entries {
            let entry = entry.map_err(|e| OpenApiError::Cache(format!("read entry: {e}")))?;
            let path = entry.path();
            if path.is_dir() {
                self.scan_spec_files(base, &path, paths)?;
            } else if path.extension().is_some_and(|ext| ext == "json")
                && path.file_name().is_some_and(|n| n != "discovery.json")
            {
                let rel = path.strip_prefix(base).unwrap_or(&path).with_extension("");
                let gv = rel.to_string_lossy().replace('\\', "/");
                paths.insert(
                    gv,
                    DiscoveryPath {
                        server_relative_url: String::new(),
                    },
                );
            }
        }
        Ok(())
    }

    fn read_spec_from_directory(
        &self,
        dir: &std::path::Path,
        group_version: &str,
    ) -> Result<serde_json::Value, OpenApiError> {
        let path = dir.join(format!("{group_version}.json"));
        let data = std::fs::read_to_string(&path)
            .map_err(|e| OpenApiError::Cache(format!("read {}: {e}", path.display())))?;
        serde_json::from_str(&data)
            .map_err(|e| OpenApiError::Parse(format!("parse {}: {e}", path.display())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_discovery_json() -> serde_json::Value {
        serde_json::json!({
            "paths": {
                "api/v1": {
                    "serverRelativeURL": "/openapi/v3/api/v1?hash=HASH_A"
                },
                "apis/apps/v1": {
                    "serverRelativeURL": "/openapi/v3/apis/apps/v1?hash=HASH_B"
                }
            }
        })
    }

    fn mock_spec_json() -> serde_json::Value {
        serde_json::json!({
            "openapi": "3.0.0",
            "info": { "title": "Kubernetes", "version": "v1.30.0" }
        })
    }

    #[test]
    fn discover_from_server() {
        let mut server = mockito::Server::new();
        let discovery = mock_discovery_json();
        let mock = server
            .mock("GET", "/openapi/v3")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(discovery.to_string())
            .create();

        let tmp = tempfile::tempdir().unwrap();
        let client = OpenApiClient::new(FetchOptions {
            source: OpenApiSource::Url {
                base_url: server.url(),
                bearer_token: None,
            },
            cache_dir: tmp.path().to_path_buf(),
            offline: false,
        })
        .unwrap();

        let index = client.discover().unwrap();
        assert_eq!(index.paths.len(), 2);
        assert!(index.paths.contains_key("api/v1"));
        assert!(index.paths.contains_key("apis/apps/v1"));
        mock.assert();
    }

    #[test]
    fn fetch_spec_from_server() {
        let mut server = mockito::Server::new();
        let discovery = mock_discovery_json();
        let spec = mock_spec_json();

        let _discovery_mock = server
            .mock("GET", "/openapi/v3")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(discovery.to_string())
            .create();

        let _spec_mock = server
            .mock("GET", "/openapi/v3/api/v1?hash=HASH_A")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(spec.to_string())
            .create();

        let tmp = tempfile::tempdir().unwrap();
        let client = OpenApiClient::new(FetchOptions {
            source: OpenApiSource::Url {
                base_url: server.url(),
                bearer_token: None,
            },
            cache_dir: tmp.path().to_path_buf(),
            offline: false,
        })
        .unwrap();

        let result = client.fetch_spec("api/v1").unwrap();
        assert_eq!(result["openapi"], "3.0.0");
    }

    #[test]
    fn cache_reuse_same_hash() {
        let mut server = mockito::Server::new();
        let discovery = mock_discovery_json();
        let spec = mock_spec_json();

        let discovery_mock = server
            .mock("GET", "/openapi/v3")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(discovery.to_string())
            .expect_at_least(1)
            .create();

        let spec_mock = server
            .mock("GET", "/openapi/v3/api/v1?hash=HASH_A")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(spec.to_string())
            .expect(1)
            .create();

        let tmp = tempfile::tempdir().unwrap();
        let client = OpenApiClient::new(FetchOptions {
            source: OpenApiSource::Url {
                base_url: server.url(),
                bearer_token: None,
            },
            cache_dir: tmp.path().to_path_buf(),
            offline: false,
        })
        .unwrap();

        // First fetch — hits server
        let result1 = client.fetch_spec("api/v1").unwrap();
        assert_eq!(result1["openapi"], "3.0.0");

        // Second fetch — same hash, should use cache for spec
        let result2 = client.fetch_spec("api/v1").unwrap();
        assert_eq!(result2["openapi"], "3.0.0");

        // Spec endpoint should only be hit once
        spec_mock.assert();
        discovery_mock.assert();
    }

    #[test]
    fn cache_invalidation_hash_change() {
        let mut server = mockito::Server::new();
        let spec = mock_spec_json();
        let updated_spec = serde_json::json!({
            "openapi": "3.1.0",
            "info": { "title": "Kubernetes", "version": "v1.31.0" }
        });

        // First discovery with HASH_A
        let discovery_v1 = serde_json::json!({
            "paths": {
                "api/v1": { "serverRelativeURL": "/openapi/v3/api/v1?hash=HASH_A" }
            }
        });

        let _discovery_mock_v1 = server
            .mock("GET", "/openapi/v3")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(discovery_v1.to_string())
            .expect(1)
            .create();

        let _spec_mock_v1 = server
            .mock("GET", "/openapi/v3/api/v1?hash=HASH_A")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(spec.to_string())
            .expect(1)
            .create();

        let tmp = tempfile::tempdir().unwrap();
        let client = OpenApiClient::new(FetchOptions {
            source: OpenApiSource::Url {
                base_url: server.url(),
                bearer_token: None,
            },
            cache_dir: tmp.path().to_path_buf(),
            offline: false,
        })
        .unwrap();

        let result1 = client.fetch_spec("api/v1").unwrap();
        assert_eq!(result1["openapi"], "3.0.0");

        // Remove old mocks by dropping them, create new ones with changed hash
        drop(_discovery_mock_v1);
        drop(_spec_mock_v1);

        let discovery_v2 = serde_json::json!({
            "paths": {
                "api/v1": { "serverRelativeURL": "/openapi/v3/api/v1?hash=HASH_NEW" }
            }
        });

        let _discovery_mock_v2 = server
            .mock("GET", "/openapi/v3")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(discovery_v2.to_string())
            .expect(1)
            .create();

        let _spec_mock_v2 = server
            .mock("GET", "/openapi/v3/api/v1?hash=HASH_NEW")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(updated_spec.to_string())
            .expect(1)
            .create();

        // Second fetch — hash changed, should re-fetch
        let result2 = client.fetch_spec("api/v1").unwrap();
        assert_eq!(result2["openapi"], "3.1.0");
    }

    #[test]
    fn offline_mode_with_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let key = cache::server_key("https://localhost:6443");

        // Pre-populate cache
        let index = DiscoveryIndex {
            paths: HashMap::from([(
                "api/v1".to_string(),
                DiscoveryPath {
                    server_relative_url: "/openapi/v3/api/v1?hash=CACHED".to_string(),
                },
            )]),
        };
        let spec = mock_spec_json();
        cache::write_discovery(tmp.path(), &key, &index).unwrap();
        cache::write_spec(tmp.path(), &key, "api/v1", &spec).unwrap();

        let client = OpenApiClient::new(FetchOptions {
            source: OpenApiSource::Url {
                base_url: "https://localhost:6443".to_string(),
                bearer_token: None,
            },
            cache_dir: tmp.path().to_path_buf(),
            offline: true,
        })
        .unwrap();

        let result_index = client.discover().unwrap();
        assert_eq!(result_index.paths.len(), 1);

        let result_spec = client.fetch_spec("api/v1").unwrap();
        assert_eq!(result_spec["openapi"], "3.0.0");
    }

    #[test]
    fn offline_mode_no_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let client = OpenApiClient::new(FetchOptions {
            source: OpenApiSource::Url {
                base_url: "https://localhost:6443".to_string(),
                bearer_token: None,
            },
            cache_dir: tmp.path().to_path_buf(),
            offline: true,
        })
        .unwrap();

        let err = client.discover().unwrap_err();
        assert!(matches!(err, OpenApiError::NoCachedData));
    }

    #[test]
    fn network_failure_cache_fallback() {
        let mut server = mockito::Server::new();

        // First, populate cache via successful fetch
        let discovery = mock_discovery_json();
        let spec = mock_spec_json();

        let _dm = server
            .mock("GET", "/openapi/v3")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(discovery.to_string())
            .create();

        let _sm = server
            .mock("GET", "/openapi/v3/api/v1?hash=HASH_A")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(spec.to_string())
            .create();

        let tmp = tempfile::tempdir().unwrap();
        let client = OpenApiClient::new(FetchOptions {
            source: OpenApiSource::Url {
                base_url: server.url(),
                bearer_token: None,
            },
            cache_dir: tmp.path().to_path_buf(),
            offline: false,
        })
        .unwrap();

        // Populate cache
        client.fetch_spec("api/v1").unwrap();

        // Now make server return 500
        drop(_dm);
        drop(_sm);

        let _dm_fail = server.mock("GET", "/openapi/v3").with_status(500).create();

        // Should fall back to cache
        let result = client.discover().unwrap();
        assert_eq!(result.paths.len(), 2);
    }

    #[test]
    fn network_failure_no_cache() {
        let mut server = mockito::Server::new();
        let _mock = server.mock("GET", "/openapi/v3").with_status(500).create();

        let tmp = tempfile::tempdir().unwrap();
        let client = OpenApiClient::new(FetchOptions {
            source: OpenApiSource::Url {
                base_url: server.url(),
                bearer_token: None,
            },
            cache_dir: tmp.path().to_path_buf(),
            offline: false,
        })
        .unwrap();

        let err = client.discover().unwrap_err();
        assert!(matches!(err, OpenApiError::Http(_)));
    }

    #[test]
    fn directory_source() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();

        // Create spec files
        std::fs::create_dir_all(dir.join("api")).unwrap();
        std::fs::create_dir_all(dir.join("apis/apps")).unwrap();
        std::fs::write(dir.join("api/v1.json"), mock_spec_json().to_string()).unwrap();
        std::fs::write(dir.join("apis/apps/v1.json"), mock_spec_json().to_string()).unwrap();

        let cache_tmp = tempfile::tempdir().unwrap();
        let client = OpenApiClient::new(FetchOptions {
            source: OpenApiSource::Directory(dir.to_path_buf()),
            cache_dir: cache_tmp.path().to_path_buf(),
            offline: true,
        })
        .unwrap();

        let index = client.discover().unwrap();
        assert_eq!(index.paths.len(), 2);

        let spec = client.fetch_spec("api/v1").unwrap();
        assert_eq!(spec["openapi"], "3.0.0");
    }

    #[test]
    fn fetch_all_specs_integration() {
        let mut server = mockito::Server::new();
        let discovery = mock_discovery_json();
        let spec = mock_spec_json();

        let _dm = server
            .mock("GET", "/openapi/v3")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(discovery.to_string())
            .create();

        let _sm1 = server
            .mock("GET", "/openapi/v3/api/v1?hash=HASH_A")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(spec.to_string())
            .create();

        let _sm2 = server
            .mock("GET", "/openapi/v3/apis/apps/v1?hash=HASH_B")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(spec.to_string())
            .create();

        let tmp = tempfile::tempdir().unwrap();
        let client = OpenApiClient::new(FetchOptions {
            source: OpenApiSource::Url {
                base_url: server.url(),
                bearer_token: None,
            },
            cache_dir: tmp.path().to_path_buf(),
            offline: false,
        })
        .unwrap();

        let all = client.fetch_all_specs().unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.contains_key("api/v1"));
        assert!(all.contains_key("apis/apps/v1"));
    }
}
