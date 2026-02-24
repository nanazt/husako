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
pub fn resolve(
    name: &str,
    repo: &str,
    chart: &str,
    version: &str,
    cache_dir: &Path,
) -> Result<serde_json::Value, HelmError> {
    // Delegate OCI registries to the dedicated OCI resolver
    if repo.starts_with("oci://") {
        return crate::oci::resolve(name, repo, chart, version, cache_dir);
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

    // Fetch index.yaml
    let index_url = format!("{}/index.yaml", repo.trim_end_matches('/'));
    let index_yaml = fetch_url(name, &index_url)?;
    let archive_url = find_chart_archive_url(name, chart, version, &index_yaml)?;

    // Some Helm registry index.yaml files list OCI URLs as archive URLs
    // (e.g. Bitnami moved their HTTP registry to OCI). Delegate to oci::resolve.
    if archive_url.starts_with("oci://") {
        return crate::oci::resolve(name, &archive_url, chart, version, cache_dir);
    }

    // Download and extract
    let archive_bytes = fetch_url_bytes(name, &archive_url)?;
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
fn fetch_url(name: &str, url: &str) -> Result<String, HelmError> {
    let resp = reqwest::blocking::get(url)
        .map_err(|e| HelmError::Io(format!("chart '{name}': fetch {url}: {e}")))?;

    if !resp.status().is_success() {
        return Err(HelmError::Io(format!(
            "chart '{name}': fetch {url} returned {}",
            resp.status()
        )));
    }

    resp.text()
        .map_err(|e| HelmError::Io(format!("chart '{name}': read response from {url}: {e}")))
}

/// Fetch a URL as bytes.
fn fetch_url_bytes(name: &str, url: &str) -> Result<Vec<u8>, HelmError> {
    let resp = reqwest::blocking::get(url)
        .map_err(|e| HelmError::Io(format!("chart '{name}': fetch {url}: {e}")))?;

    if !resp.status().is_success() {
        return Err(HelmError::Io(format!(
            "chart '{name}': fetch {url} returned {}",
            resp.status()
        )));
    }

    resp.bytes()
        .map(|b| b.to_vec())
        .map_err(|e| HelmError::Io(format!("chart '{name}': read bytes from {url}: {e}")))
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

    /// OCI URLs are delegated to oci::resolve. Pre-populate the OCI cache so
    /// no network call is made — this proves the delegation path is exercised.
    #[test]
    fn oci_registry_delegates_via_cache() {
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

        let result = resolve("test", oci_url, "my-chart", "1.0.0", tmp.path()).unwrap();
        assert_eq!(result["type"], "object");
        assert!(result["properties"]["replicas"].is_object());
    }

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
    fn extract_schema_from_tgz() {
        // Create a .tgz in memory with a values.schema.json
        let mut builder = tar::Builder::new(Vec::new());

        let schema = r#"{"type":"object","properties":{"replicas":{"type":"integer"}}}"#;
        let mut header = tar::Header::new_gnu();
        header.set_path("my-chart/values.schema.json").unwrap();
        header.set_size(schema.len() as u64);
        header.set_cksum();
        builder.append(&header, schema.as_bytes()).unwrap();
        let tar_data = builder.into_inner().unwrap();

        // Gzip it
        use flate2::write::GzEncoder;
        use std::io::Write;
        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&tar_data).unwrap();
        let gz_data = encoder.finish().unwrap();

        let result = extract_values_schema("test", "my-chart", &gz_data).unwrap();
        assert_eq!(result["type"], "object");
        assert!(result["properties"]["replicas"].is_object());
    }

    #[test]
    fn extract_schema_missing_in_archive() {
        // Create a .tgz without values.schema.json
        let mut builder = tar::Builder::new(Vec::new());
        let content = "# Chart.yaml\nname: my-chart";
        let mut header = tar::Header::new_gnu();
        header.set_path("my-chart/Chart.yaml").unwrap();
        header.set_size(content.len() as u64);
        header.set_cksum();
        builder.append(&header, content.as_bytes()).unwrap();
        let tar_data = builder.into_inner().unwrap();

        use flate2::write::GzEncoder;
        use std::io::Write;
        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&tar_data).unwrap();
        let gz_data = encoder.finish().unwrap();

        let err = extract_values_schema("test", "my-chart", &gz_data).unwrap_err();
        assert!(
            err.to_string()
                .contains("does not include values.schema.json")
        );
    }
}
