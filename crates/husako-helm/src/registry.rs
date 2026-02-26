use std::io::Read;
use std::path::Path;

use crate::HelmError;

/// Resolve a Helm chart from a Helm repository.
///
/// Flow:
/// 1. Check cache
/// 2. OCI `repo` URLs delegate to `oci::resolve`
/// 3. Fetch `{repo}/index.yaml`
/// 4. Find chart entry → match version → get archive URL
/// 5. If archive URL is OCI (some registries list `oci://` in their index),
///    delegate to `oci::resolve`
/// 6. Download `.tgz` archive
/// 7. Extract `values.schema.json` from archive
/// 8. Cache and return
pub async fn resolve(
    name: &str,
    repo: &str,
    chart: &str,
    version: &str,
    cache_dir: &Path,
    on_progress: Option<&crate::ProgressCb>,
) -> Result<serde_json::Value, HelmError> {
    // Delegate OCI registries to the dedicated OCI resolver
    if repo.starts_with("oci://") {
        return crate::oci::resolve(name, repo, chart, version, cache_dir, on_progress).await;
    }

    // Check cache
    let cache_key = crate::cache_hash(&format!("{repo}/{chart}"));
    let cache_path = cache_dir.join(format!("helm/registry/{cache_key}/{version}.json"));
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

    let client = reqwest::Client::new();

    // Fetch index.yaml
    let index_url = format!("{}/index.yaml", repo.trim_end_matches('/'));
    let index_yaml = fetch_url(&client, name, &index_url).await?;
    let archive_url = find_chart_archive_url(name, chart, version, &index_yaml)?;

    // Some Helm registry index.yaml files list OCI URLs as archive URLs
    // (e.g. Bitnami moved their HTTP registry to OCI). Delegate to oci::resolve.
    if archive_url.starts_with("oci://") {
        return crate::oci::resolve(name, &archive_url, chart, version, cache_dir, on_progress)
            .await;
    }

    // Download and extract
    let archive_bytes = fetch_url_bytes(&client, name, &archive_url, on_progress).await?;
    let schema = extract_values_schema(name, chart, &archive_bytes)?;

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

/// Fetch a URL as text.
async fn fetch_url(client: &reqwest::Client, name: &str, url: &str) -> Result<String, HelmError> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| HelmError::Io(format!("chart '{name}': fetch {url}: {e}")))?;

    if !resp.status().is_success() {
        return Err(HelmError::Io(format!(
            "chart '{name}': fetch {url} returned {}",
            resp.status()
        )));
    }

    resp.text()
        .await
        .map_err(|e| HelmError::Io(format!("chart '{name}': read response from {url}: {e}")))
}

/// Fetch a URL as bytes, streaming chunks and reporting progress.
async fn fetch_url_bytes(
    client: &reqwest::Client,
    name: &str,
    url: &str,
    on_progress: Option<&crate::ProgressCb>,
) -> Result<Vec<u8>, HelmError> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| HelmError::Io(format!("chart '{name}': fetch {url}: {e}")))?;

    if !resp.status().is_success() {
        return Err(HelmError::Io(format!(
            "chart '{name}': fetch {url} returned {}",
            resp.status()
        )));
    }

    let total_bytes = resp.content_length();
    let mut received: u64 = 0;
    let mut buf = Vec::new();
    let mut resp = resp;
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| HelmError::Io(format!("chart '{name}': read bytes from {url}: {e}")))?
    {
        received += chunk.len() as u64;
        if let Some(cb) = on_progress {
            cb(received, total_bytes, None);
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

/// Parse index.yaml and find the archive URL for a specific chart and version.
fn find_chart_archive_url(
    name: &str,
    chart: &str,
    version: &str,
    index_yaml: &str,
) -> Result<String, HelmError> {
    let index: serde_json::Value = serde_yaml_ng::from_str(index_yaml)
        .map_err(|e| HelmError::Io(format!("chart '{name}': parse index.yaml: {e}")))?;

    let entries = index
        .get("entries")
        .and_then(|e| e.as_object())
        .ok_or_else(|| {
            HelmError::Io(format!(
                "chart '{name}': index.yaml has no 'entries' section"
            ))
        })?;

    let chart_versions = entries
        .get(chart)
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            HelmError::NotFound(format!(
                "chart '{name}': chart '{chart}' not found in repository index"
            ))
        })?;

    for entry in chart_versions {
        let entry_version = entry.get("version").and_then(|v| v.as_str()).unwrap_or("");
        if entry_version == version
            && let Some(urls) = entry.get("urls").and_then(|u| u.as_array())
            && let Some(url) = urls.first().and_then(|u| u.as_str())
        {
            return Ok(url.to_string());
        }
    }

    Err(HelmError::NotFound(format!(
        "chart '{name}': version '{version}' of chart '{chart}' not found in repository"
    )))
}

/// Extract `values.schema.json` from a `.tgz` archive.
pub(crate) fn extract_values_schema(
    name: &str,
    chart: &str,
    archive_bytes: &[u8],
) -> Result<serde_json::Value, HelmError> {
    let gz = flate2::read::GzDecoder::new(archive_bytes);
    let mut archive = tar::Archive::new(gz);

    let target_path = format!("{chart}/values.schema.json");

    for entry in archive
        .entries()
        .map_err(|e| HelmError::Io(format!("chart '{name}': read archive entries: {e}")))?
    {
        let mut entry =
            entry.map_err(|e| HelmError::Io(format!("chart '{name}': read archive entry: {e}")))?;

        let path = entry
            .path()
            .map_err(|e| HelmError::Io(format!("chart '{name}': read entry path: {e}")))?
            .to_string_lossy()
            .to_string();

        if path == target_path {
            let mut content = String::new();
            entry.read_to_string(&mut content).map_err(|e| {
                HelmError::Io(format!(
                    "chart '{name}': read values.schema.json from archive: {e}"
                ))
            })?;

            return serde_json::from_str(&content).map_err(|e| {
                HelmError::InvalidSchema(format!("chart '{name}': parse values.schema.json: {e}"))
            });
        }
    }

    Err(HelmError::NotFound(format!(
        "chart '{name}': chart does not include values.schema.json"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal `.tgz` archive containing `{chart}/values.schema.json`
    /// with the given JSON content.
    fn build_tgz(chart: &str, schema_json: &str) -> Vec<u8> {
        use flate2::write::GzEncoder;
        use std::io::Write;

        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header
            .set_path(format!("{chart}/values.schema.json"))
            .unwrap();
        header.set_size(schema_json.len() as u64);
        header.set_cksum();
        builder.append(&header, schema_json.as_bytes()).unwrap();
        let tar_data = builder.into_inner().unwrap();

        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&tar_data).unwrap();
        encoder.finish().unwrap()
    }

    // ── OCI delegation ────────────────────────────────────────────────────────

    /// OCI URLs are delegated to oci::resolve. Pre-populate the OCI cache so
    /// no network call is made — this proves the delegation path is exercised.
    #[tokio::test]
    async fn oci_registry_delegates_via_cache() {
        let oci_url = "oci://registry.example.com/charts";
        let tmp = tempfile::tempdir().unwrap();

        let cache_key = crate::cache_hash(oci_url);
        let cache_sub = tmp.path().join(format!("helm/oci/{cache_key}"));
        std::fs::create_dir_all(&cache_sub).unwrap();
        std::fs::write(
            cache_sub.join("1.0.0.json"),
            r#"{"type":"object","properties":{"replicas":{"type":"integer"}}}"#,
        )
        .unwrap();

        let result = resolve("test", oci_url, "my-chart", "1.0.0", tmp.path(), None)
            .await
            .unwrap();
        assert_eq!(result["type"], "object");
        assert!(result["properties"]["replicas"].is_object());
    }

    // ── find_chart_archive_url ────────────────────────────────────────────────

    #[test]
    fn find_chart_url_from_index() {
        let index = r#"
apiVersion: v1
entries:
  ingress-nginx:
    - version: "4.12.0"
      urls:
        - https://example.com/charts/ingress-nginx-4.12.0.tgz
    - version: "4.11.0"
      urls:
        - https://example.com/charts/ingress-nginx-4.11.0.tgz
"#;
        let url = find_chart_archive_url("test", "ingress-nginx", "4.12.0", index).unwrap();
        assert_eq!(url, "https://example.com/charts/ingress-nginx-4.12.0.tgz");
    }

    #[test]
    fn find_chart_url_version_not_found() {
        let index = r#"
apiVersion: v1
entries:
  ingress-nginx:
    - version: "4.11.0"
      urls:
        - https://example.com/charts/ingress-nginx-4.11.0.tgz
"#;
        let err = find_chart_archive_url("test", "ingress-nginx", "4.12.0", index).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn find_chart_url_chart_not_found() {
        let index = r#"
apiVersion: v1
entries:
  other-chart:
    - version: "1.0.0"
      urls:
        - https://example.com/charts/other-1.0.0.tgz
"#;
        let err = find_chart_archive_url("test", "ingress-nginx", "4.12.0", index).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn find_chart_url_missing_entries_section() {
        let index = "apiVersion: v1\n";
        let err = find_chart_archive_url("test", "my-chart", "1.0.0", index).unwrap_err();
        assert!(err.to_string().contains("no 'entries' section"));
    }

    // ── extract_values_schema ─────────────────────────────────────────────────

    #[test]
    fn extract_schema_from_tgz() {
        let schema = r#"{"type":"object","properties":{"replicas":{"type":"integer"}}}"#;
        let gz_data = build_tgz("my-chart", schema);

        let result = extract_values_schema("test", "my-chart", &gz_data).unwrap();
        assert_eq!(result["type"], "object");
        assert!(result["properties"]["replicas"].is_object());
    }

    #[test]
    fn extract_schema_missing_in_archive() {
        // Build a .tgz with Chart.yaml but no values.schema.json
        use flate2::write::GzEncoder;
        use std::io::Write;

        let content = "# Chart.yaml\nname: my-chart";
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_path("my-chart/Chart.yaml").unwrap();
        header.set_size(content.len() as u64);
        header.set_cksum();
        builder.append(&header, content.as_bytes()).unwrap();
        let tar_data = builder.into_inner().unwrap();

        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&tar_data).unwrap();
        let gz_data = encoder.finish().unwrap();

        let err = extract_values_schema("test", "my-chart", &gz_data).unwrap_err();
        assert!(
            err.to_string()
                .contains("does not include values.schema.json")
        );
    }

    // ── resolve() integration (mockito) ──────────────────────────────────────

    /// Cache hit: pre-populate cache, then call resolve() with an invalid repo URL.
    /// No HTTP request should be made.
    #[tokio::test]
    async fn cache_hit_skips_http() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = "http://127.0.0.1:1"; // port 1 — connection refused if actually called
        let chart = "my-chart";
        let version = "1.0.0";

        let cache_key = crate::cache_hash(&format!("{repo}/{chart}"));
        let cache_sub = tmp.path().join(format!("helm/registry/{cache_key}"));
        std::fs::create_dir_all(&cache_sub).unwrap();
        std::fs::write(
            cache_sub.join(format!("{version}.json")),
            r#"{"type":"object","properties":{"cached":{"type":"boolean"}}}"#,
        )
        .unwrap();

        let result = resolve("test", repo, chart, version, tmp.path(), None)
            .await
            .unwrap();
        assert_eq!(result["type"], "object");
        assert!(result["properties"]["cached"].is_object());
    }

    /// Full HTTP flow: mockito serves index.yaml and the .tgz archive.
    #[tokio::test]
    async fn resolve_fetches_index_and_archive() {
        let mut server = mockito::Server::new_async().await;
        let host_url = server.url();

        let chart = "my-chart";
        let version = "1.0.0";
        let archive_path = "/my-chart-1.0.0.tgz";

        let index = format!(
            "apiVersion: v1\nentries:\n  my-chart:\n    - version: \"{version}\"\n      urls:\n        - {host_url}{archive_path}\n"
        );
        let _m_index = server
            .mock("GET", "/index.yaml")
            .with_status(200)
            .with_body(index)
            .create_async()
            .await;

        let tgz = build_tgz(
            chart,
            r#"{"type":"object","properties":{"replicas":{"type":"integer"}}}"#,
        );
        let _m_archive = server
            .mock("GET", archive_path)
            .with_status(200)
            .with_body(tgz)
            .create_async()
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let result = resolve("test", &host_url, chart, version, tmp.path(), None)
            .await
            .unwrap();
        assert_eq!(result["type"], "object");
        assert!(result["properties"]["replicas"].is_object());
    }

    /// When index.yaml lists an `oci://` archive URL, resolve() delegates to oci::resolve.
    /// Pre-populate the OCI cache so no real OCI network call is made.
    #[tokio::test]
    async fn resolve_oci_archive_url_in_index_delegates_to_oci() {
        let mut server = mockito::Server::new_async().await;
        let host_url = server.url();

        let chart = "my-chart";
        let version = "2.0.0";
        let oci_url = "oci://ghcr.io/myorg/my-chart";

        let index = format!(
            "apiVersion: v1\nentries:\n  my-chart:\n    - version: \"{version}\"\n      urls:\n        - {oci_url}\n"
        );
        let _m_index = server
            .mock("GET", "/index.yaml")
            .with_status(200)
            .with_body(index)
            .create_async()
            .await;

        // Seed the OCI cache so oci::resolve returns immediately
        let tmp = tempfile::tempdir().unwrap();
        let cache_key = crate::cache_hash(oci_url);
        let cache_sub = tmp.path().join(format!("helm/oci/{cache_key}"));
        std::fs::create_dir_all(&cache_sub).unwrap();
        std::fs::write(
            cache_sub.join(format!("{version}.json")),
            r#"{"type":"object","properties":{"ociField":{"type":"string"}}}"#,
        )
        .unwrap();

        let result = resolve("test", &host_url, chart, version, tmp.path(), None)
            .await
            .unwrap();
        assert_eq!(result["type"], "object");
        assert!(result["properties"]["ociField"].is_object());
    }

    /// HTTP 404 on index.yaml → error message includes the status code.
    #[tokio::test]
    async fn resolve_index_http_error_returns_err() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/index.yaml")
            .with_status(404)
            .create_async()
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let err = resolve("test", &server.url(), "my-chart", "1.0.0", tmp.path(), None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("404"));
    }
}
