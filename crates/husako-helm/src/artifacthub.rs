use std::path::Path;

use crate::HelmError;

const ARTIFACTHUB_BASE: &str = "https://artifacthub.io/api/v1/packages/helm";

/// Resolve a Helm chart from the ArtifactHub API.
///
/// Flow:
/// 1. Check cache
/// 2. Fetch `https://artifacthub.io/api/v1/packages/helm/{package}/{version}`
/// 3. Use `values_schema` if present — cache and return
/// 4. Otherwise attempt a registry fallback using `repository.url`
///    (delegates to `registry::resolve`, which handles both HTTP and OCI)
pub fn resolve(
    name: &str,
    package: &str,
    version: &str,
    cache_dir: &Path,
) -> Result<serde_json::Value, HelmError> {
    resolve_from(name, package, version, cache_dir, ARTIFACTHUB_BASE)
}

fn resolve_from(
    name: &str,
    package: &str,
    version: &str,
    cache_dir: &Path,
    base_url: &str,
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
    let url = format!("{base_url}/{package}/{version}");
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

    // Phase 1: use values_schema if the chart author published it
    if let Some(schema_str) = body.get("values_schema").and_then(|v| v.as_str()) {
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

        return Ok(schema);
    }

    // Phase 2: no values_schema — try a registry fallback using repository.url
    let repo_url = body
        .get("repository")
        .and_then(|r| r.get("url"))
        .and_then(|u| u.as_str())
        .unwrap_or("");

    let chart_name = package.rsplit('/').next().unwrap_or(package);

    if !repo_url.is_empty() {
        return crate::registry::resolve(name, repo_url, chart_name, version, cache_dir).map_err(
            |e| {
                HelmError::NotFound(format!(
                    "chart '{name}': no values_schema on ArtifactHub; \
                     registry fallback ({repo_url}) also failed: {e}"
                ))
            },
        );
    }

    Err(HelmError::NotFound(format!(
        "chart '{name}': no values_schema on ArtifactHub and no repository URL available. \
         Download values.schema.json manually and use source = \"file\" instead."
    )))
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

    /// When `values_schema` is present in the API response, it is used directly.
    #[test]
    fn values_schema_present_returned_directly() {
        let schema_json = r#"{"type":"object","properties":{"replicas":{"type":"integer"}}}"#;
        let api_body = serde_json::json!({ "values_schema": schema_json });

        let mut server = mockito::Server::new();
        let _m = server
            .mock("GET", "/my-org/my-chart/1.0.0")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(api_body.to_string())
            .create();

        let tmp = tempfile::tempdir().unwrap();
        let result = resolve_from(
            "test",
            "my-org/my-chart",
            "1.0.0",
            tmp.path(),
            &server.url(),
        )
        .unwrap();
        assert_eq!(result["type"], "object");
        assert!(result["properties"]["replicas"].is_object());
    }

    /// When `values_schema` is absent and `repository.url` is empty, resolve
    /// returns an error guiding the user to `source = "file"`.
    #[test]
    fn no_values_schema_no_repo_url_returns_clear_error() {
        let api_body = serde_json::json!({ "name": "some-chart" });

        let mut server = mockito::Server::new();
        let _m = server
            .mock("GET", "/my-org/some-chart/1.0.0")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(api_body.to_string())
            .create();

        let tmp = tempfile::tempdir().unwrap();
        let err = resolve_from(
            "some-chart",
            "my-org/some-chart",
            "1.0.0",
            tmp.path(),
            &server.url(),
        )
        .unwrap_err();

        let msg = err.to_string();
        assert!(msg.contains("source = \"file\""), "got: {msg}");
    }

    /// When `values_schema` is absent and `repository.url` is an OCI URL,
    /// resolve now delegates to oci::resolve via registry::resolve.
    /// The OCI cache pre-populated here proves the delegation path is exercised.
    #[test]
    fn no_values_schema_oci_repo_delegates_to_oci_resolver() {
        let oci_url = "oci://registry-1.docker.io/bitnamicharts/postgresql";
        let api_body = serde_json::json!({
            "name": "postgresql",
            "repository": { "url": oci_url }
        });

        let mut server = mockito::Server::new();
        let _m = server
            .mock("GET", "/bitnami/postgresql/16.4.0")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(api_body.to_string())
            .create();

        let tmp = tempfile::tempdir().unwrap();

        // Pre-populate the OCI cache so no real network call is made
        let cache_key = crate::cache_hash(oci_url);
        let cache_sub = tmp.path().join(format!("helm/oci/{cache_key}"));
        std::fs::create_dir_all(&cache_sub).unwrap();
        std::fs::write(
            cache_sub.join("16.4.0.json"),
            r#"{"type":"object","properties":{"replicaCount":{"type":"integer"}}}"#,
        )
        .unwrap();

        let result = resolve_from(
            "postgresql",
            "bitnami/postgresql",
            "16.4.0",
            tmp.path(),
            &server.url(),
        )
        .unwrap();
        assert_eq!(result["type"], "object");
        assert!(result["properties"]["replicaCount"].is_object());
    }

    /// When `values_schema` is absent and `repository.url` is an HTTP registry,
    /// resolve delegates to registry::resolve. Registry cache pre-populated here
    /// proves the delegation path is exercised.
    #[test]
    fn no_values_schema_http_repo_delegates_to_registry() {
        let registry_url = "https://charts.example.com";
        let api_body = serde_json::json!({
            "name": "my-chart",
            "repository": { "url": registry_url }
        });

        let mut server = mockito::Server::new();
        let _m = server
            .mock("GET", "/my-org/my-chart/1.0.0")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(api_body.to_string())
            .create();

        let tmp = tempfile::tempdir().unwrap();

        // Pre-populate the registry cache so no real network call is made
        let cache_key = crate::cache_hash(&format!("{registry_url}/my-chart"));
        let cache_sub = tmp.path().join(format!("helm/registry/{cache_key}"));
        std::fs::create_dir_all(&cache_sub).unwrap();
        std::fs::write(
            cache_sub.join("1.0.0.json"),
            r#"{"type":"object","properties":{"replicas":{"type":"integer"}}}"#,
        )
        .unwrap();

        let result = resolve_from(
            "test",
            "my-org/my-chart",
            "1.0.0",
            tmp.path(),
            &server.url(),
        )
        .unwrap();
        assert_eq!(result["type"], "object");
        assert!(result["properties"]["replicas"].is_object());
    }
}
