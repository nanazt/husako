use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

use crate::OpenApiError;

type ProgressCb = dyn Fn(u64, Option<u64>, Option<u8>) + Sync;

/// Fetch Kubernetes OpenAPI specs from a GitHub release tag.
///
/// `version` can be `"1.35"` (mapped to `v1.35.0`) or a full semver like `"1.35.1"`.
/// Results are cached under `cache_dir/release/{tag}/`.
///
/// `on_progress` is called with `(bytes_received, total_bytes, None)` during the download.
/// `total_bytes` is `Some` when the sum of file sizes is known from the GitHub API listing.
pub async fn fetch_release_specs(
    version: &str,
    cache_dir: &Path,
    on_progress: Option<&ProgressCb>,
) -> Result<HashMap<String, Value>, OpenApiError> {
    let base = std::env::var("HUSAKO_GITHUB_API_URL")
        .unwrap_or_else(|_| "https://api.github.com".to_string());
    fetch_release_specs_from(version, cache_dir, on_progress, &base).await
}

async fn fetch_release_specs_from(
    version: &str,
    cache_dir: &Path,
    on_progress: Option<&ProgressCb>,
    api_base: &str,
) -> Result<HashMap<String, Value>, OpenApiError> {
    let tag = version_to_tag(version);
    let tag_cache = cache_dir.join(format!("release/{tag}"));

    // Check cache first — tag-based caching is deterministic (skip download entirely)
    if tag_cache.exists() {
        return load_cached_specs(&tag_cache);
    }

    // List spec files from the GitHub API
    let client = build_http_client()?;
    let contents_url =
        format!("{api_base}/repos/kubernetes/kubernetes/contents/api/openapi-spec/v3?ref={tag}");

    let resp = client
        .get(&contents_url)
        .header("User-Agent", "husako")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| release_err(format!("GitHub API request failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(release_err(format!(
            "GitHub API returned {} for tag '{tag}'",
            resp.status()
        )));
    }

    let entries: Vec<GithubContent> = resp
        .json()
        .await
        .map_err(|e| release_err(format!("parse GitHub response: {e}")))?;

    // Filter for OpenAPI spec files
    let spec_files: Vec<&GithubContent> = entries
        .iter()
        .filter(|e| is_openapi_spec_file(&e.name))
        .collect();

    if spec_files.is_empty() {
        return Err(release_err(format!(
            "no OpenAPI spec files found for tag '{tag}'"
        )));
    }

    // Download each spec
    let mut specs = HashMap::new();
    std::fs::create_dir_all(&tag_cache).map_err(|e| {
        OpenApiError::Cache(format!("create cache dir {}: {e}", tag_cache.display()))
    })?;

    // Sum known file sizes for progress reporting (size=0 means unknown)
    let total_bytes: Option<u64> = {
        let sum: u64 = spec_files.iter().map(|e| e.size).sum();
        if sum > 0 { Some(sum) } else { None }
    };
    let mut received_bytes: u64 = 0;

    for entry in &spec_files {
        let download_url = entry
            .download_url
            .as_deref()
            .ok_or_else(|| release_err(format!("no download_url for {}", entry.name)))?;

        let spec_resp = client
            .get(download_url)
            .header("User-Agent", "husako")
            .send()
            .await
            .map_err(|e| release_err(format!("download {}: {e}", entry.name)))?;

        if !spec_resp.status().is_success() {
            return Err(release_err(format!(
                "download {} returned {}",
                entry.name,
                spec_resp.status()
            )));
        }

        // Stream body with progress reporting
        let mut buf = Vec::new();
        let mut resp = spec_resp;
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| release_err(format!("read {}: {e}", entry.name)))?
        {
            received_bytes += chunk.len() as u64;
            if let Some(cb) = on_progress {
                cb(received_bytes, total_bytes, None);
            }
            buf.extend_from_slice(&chunk);
        }

        let spec: Value = serde_json::from_slice(&buf)
            .map_err(|e| release_err(format!("parse {}: {e}", entry.name)))?;

        let discovery_key = filename_to_discovery_key(&entry.name);

        // Cache individual spec file
        let cache_file = tag_cache.join(&entry.name);
        let _ = std::fs::write(
            &cache_file,
            serde_json::to_string(&spec).unwrap_or_default(),
        );

        specs.insert(discovery_key, spec);
    }

    // Write a manifest for quick cache loading
    let manifest: Vec<(String, String)> = spec_files
        .iter()
        .map(|e| (filename_to_discovery_key(&e.name), e.name.clone()))
        .collect();
    let manifest_json = serde_json::to_string(&manifest).unwrap_or_default();
    let _ = std::fs::write(tag_cache.join("_manifest.json"), manifest_json);

    Ok(specs)
}

/// Convert version string to a git tag.
/// `"1.35"` → `"v1.35.0"`, `"1.35.1"` → `"v1.35.1"`, `"v1.35.0"` → `"v1.35.0"`
pub fn version_to_tag(version: &str) -> String {
    let v = version.strip_prefix('v').unwrap_or(version);
    let parts: Vec<&str> = v.split('.').collect();
    match parts.len() {
        2 => format!("v{}.0", v),
        3 => format!("v{v}"),
        _ => format!("v{v}"),
    }
}

/// Convert a spec filename to a discovery key.
/// `apis__apps__v1_openapi.json` → `apis/apps/v1`
pub fn filename_to_discovery_key(filename: &str) -> String {
    filename
        .trim_end_matches("_openapi.json")
        .replace("__", "/")
}

/// Check if a filename looks like an OpenAPI spec file.
fn is_openapi_spec_file(name: &str) -> bool {
    name.ends_with("_openapi.json") && name != "api_openapi.json"
}

fn load_cached_specs(tag_cache: &Path) -> Result<HashMap<String, Value>, OpenApiError> {
    let manifest_path = tag_cache.join("_manifest.json");
    if manifest_path.exists() {
        let manifest_data = std::fs::read_to_string(&manifest_path)
            .map_err(|e| OpenApiError::Cache(format!("read manifest: {e}")))?;
        let manifest: Vec<(String, String)> = serde_json::from_str(&manifest_data)
            .map_err(|e| OpenApiError::Cache(format!("parse manifest: {e}")))?;

        let mut specs = HashMap::new();
        for (key, filename) in manifest {
            let path = tag_cache.join(&filename);
            let data = std::fs::read_to_string(&path)
                .map_err(|e| OpenApiError::Cache(format!("read {}: {e}", path.display())))?;
            let spec: Value = serde_json::from_str(&data)
                .map_err(|e| OpenApiError::Cache(format!("parse {}: {e}", path.display())))?;
            specs.insert(key, spec);
        }
        return Ok(specs);
    }

    // Fallback: scan JSON files
    let mut specs = HashMap::new();
    let entries = std::fs::read_dir(tag_cache)
        .map_err(|e| OpenApiError::Cache(format!("read cache dir: {e}")))?;
    for entry in entries {
        let entry = entry.map_err(|e| OpenApiError::Cache(format!("read entry: {e}")))?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json")
            && path.file_name().is_some_and(|n| n != "_manifest.json")
        {
            let filename = path.file_name().unwrap().to_string_lossy();
            if is_openapi_spec_file(&filename) {
                let data = std::fs::read_to_string(&path)
                    .map_err(|e| OpenApiError::Cache(format!("read {}: {e}", path.display())))?;
                let spec: Value = serde_json::from_str(&data)
                    .map_err(|e| OpenApiError::Cache(format!("parse {}: {e}", path.display())))?;
                specs.insert(filename_to_discovery_key(&filename), spec);
            }
        }
    }
    Ok(specs)
}

fn build_http_client() -> Result<reqwest::Client, OpenApiError> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| release_err(format!("build HTTP client: {e}")))
}

fn release_err(msg: String) -> OpenApiError {
    OpenApiError::Release(msg)
}

#[derive(Debug, serde::Deserialize)]
struct GithubContent {
    name: String,
    download_url: Option<String>,
    #[serde(default)]
    size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_mapping() {
        assert_eq!(version_to_tag("1.35"), "v1.35.0");
        assert_eq!(version_to_tag("1.35.1"), "v1.35.1");
        assert_eq!(version_to_tag("v1.35.0"), "v1.35.0");
        assert_eq!(version_to_tag("1.30"), "v1.30.0");
    }

    #[test]
    fn filename_conversion() {
        assert_eq!(
            filename_to_discovery_key("apis__apps__v1_openapi.json"),
            "apis/apps/v1"
        );
        assert_eq!(filename_to_discovery_key("api__v1_openapi.json"), "api/v1");
        assert_eq!(
            filename_to_discovery_key("apis__batch__v1_openapi.json"),
            "apis/batch/v1"
        );
        assert_eq!(
            filename_to_discovery_key("apis__networking.k8s.io__v1_openapi.json"),
            "apis/networking.k8s.io/v1"
        );
    }

    #[test]
    fn filter_spec_files() {
        assert!(is_openapi_spec_file("apis__apps__v1_openapi.json"));
        assert!(is_openapi_spec_file("api__v1_openapi.json"));
        assert!(!is_openapi_spec_file("api_openapi.json")); // excluded
        assert!(!is_openapi_spec_file("README.md"));
        assert!(!is_openapi_spec_file("swagger.json"));
    }

    #[test]
    fn cache_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let tag_cache = tmp.path().join("release/v1.35.0");
        std::fs::create_dir_all(&tag_cache).unwrap();

        // Write a fake spec + manifest
        let spec = serde_json::json!({"openapi": "3.0.0"});
        std::fs::write(
            tag_cache.join("apis__apps__v1_openapi.json"),
            serde_json::to_string(&spec).unwrap(),
        )
        .unwrap();
        let manifest = vec![(
            "apis/apps/v1".to_string(),
            "apis__apps__v1_openapi.json".to_string(),
        )];
        std::fs::write(
            tag_cache.join("_manifest.json"),
            serde_json::to_string(&manifest).unwrap(),
        )
        .unwrap();

        let result = load_cached_specs(&tag_cache).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("apis/apps/v1"));
        assert_eq!(result["apis/apps/v1"]["openapi"], "3.0.0");
    }

    #[tokio::test]
    async fn download_and_cache_specs_from_github_api() {
        let mut server = mockito::Server::new_async().await;
        let spec_json = serde_json::json!({"openapi": "3.0.0"});

        // Mock 1: GitHub directory listing
        let _dir_mock = server
            .mock(
                "GET",
                "/repos/kubernetes/kubernetes/contents/api/openapi-spec/v3?ref=v1.35.0",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!([{
                    "name": "apis__apps__v1_openapi.json",
                    "download_url": format!("{}/apis__apps__v1_openapi.json", server.url()),
                    "size": 42
                }])
                .to_string(),
            )
            .create_async()
            .await;

        // Mock 2: spec file download
        let _spec_mock = server
            .mock("GET", "/apis__apps__v1_openapi.json")
            .with_status(200)
            .with_body(spec_json.to_string())
            .create_async()
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let result = fetch_release_specs_from("1.35", tmp.path(), None, &server.url())
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert!(result.contains_key("apis/apps/v1"));

        // Verify cache was written
        let cache = tmp.path().join("release/v1.35.0");
        assert!(cache.join("apis__apps__v1_openapi.json").exists());
        assert!(cache.join("_manifest.json").exists());
    }

    #[tokio::test]
    async fn github_api_403_returns_error() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock(
                "GET",
                "/repos/kubernetes/kubernetes/contents/api/openapi-spec/v3?ref=v1.35.0",
            )
            .with_status(403)
            .create_async()
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let result = fetch_release_specs_from("1.35", tmp.path(), None, &server.url()).await;

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("403"), "error should mention 403: {msg}");
    }

    #[tokio::test]
    async fn cache_hit_skips_network() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path();

        // Pre-populate cache
        let tag_cache = cache_dir.join("release/v1.35.0");
        std::fs::create_dir_all(&tag_cache).unwrap();

        let spec = serde_json::json!({"openapi": "3.0.0", "info": {"title": "cached"}});
        std::fs::write(
            tag_cache.join("api__v1_openapi.json"),
            serde_json::to_string(&spec).unwrap(),
        )
        .unwrap();
        let manifest = vec![("api/v1".to_string(), "api__v1_openapi.json".to_string())];
        std::fs::write(
            tag_cache.join("_manifest.json"),
            serde_json::to_string(&manifest).unwrap(),
        )
        .unwrap();

        // This should hit cache and NOT make any network requests
        let result = fetch_release_specs("1.35", cache_dir, None).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result["api/v1"]["info"]["title"], "cached");
    }
}
