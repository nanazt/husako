use std::path::Path;

use crate::HelmError;

const MANIFEST_ACCEPT: &str = concat!(
    "application/vnd.oci.image.manifest.v1+json,",
    "application/vnd.docker.distribution.manifest.v2+json,",
    "application/vnd.oci.image.index.v1+json"
);

const HELM_LAYER_MEDIA_TYPE: &str = "application/vnd.cncf.helm.chart.content.v1.tar+gzip";
const TAR_GZ_MEDIA_TYPE: &str = "application/tar+gzip";

/// Resolve a Helm chart from an OCI registry.
///
/// Flow:
/// 1. Check cache
/// 2. Parse reference → (host, repo, tag)
/// 3. Authenticate (anonymous bearer token or no-auth)
/// 4. Fetch OCI manifest (handles image index by picking first manifest)
/// 5. Find Helm chart content layer by media type
/// 6. Download blob (.tgz)
/// 7. Extract `values.schema.json` (reuses registry::extract_values_schema)
/// 8. Cache and return
pub(crate) fn resolve(
    name: &str,
    reference: &str,
    chart: &str,
    version: &str,
    cache_dir: &Path,
) -> Result<serde_json::Value, HelmError> {
    // Check cache
    let cache_key = crate::cache_hash(reference);
    let cache_path = cache_dir.join(format!("helm/oci/{cache_key}/{version}.json"));
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

    let (host, repo, ref_tag) = parse_oci_reference(reference)?;
    let tag = ref_tag.as_deref().unwrap_or(version);

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| HelmError::Io(format!("chart '{name}': build HTTP client: {e}")))?;

    let token = get_token(&client, name, &host, &repo)?;
    let token_ref = token.as_deref();

    let manifest = fetch_manifest(&client, name, &host, &repo, tag, token_ref)?;
    let layer_digest = find_chart_layer_digest(name, &manifest)?;
    let blob_bytes = download_blob(&client, name, &host, &repo, &layer_digest, token_ref)?;

    let schema = crate::registry::extract_values_schema(name, chart, &blob_bytes)?;

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

/// Extract chart name from OCI reference (last path component, before any tag suffix).
///
/// `oci://ghcr.io/org/postgresql`       → `"postgresql"`
/// `oci://ghcr.io/org/postgresql:1.2.3` → `"postgresql"`
pub(crate) fn chart_name_from_reference(reference: &str) -> &str {
    let without_scheme = reference.strip_prefix("oci://").unwrap_or(reference);
    let path_part = without_scheme
        .split('/')
        .next_back()
        .unwrap_or(without_scheme);
    path_part.split(':').next().unwrap_or(path_part)
}

/// Fetch available stable semver tags from an OCI registry for the given reference.
///
/// Uses the same anonymous bearer-token auth flow as `resolve()`.
/// Returns tags sorted descending by semver, up to `limit` starting at `offset`.
pub fn list_tags(reference: &str, limit: usize, offset: usize) -> Result<Vec<String>, HelmError> {
    let (host, repo, _) = parse_oci_reference(reference)?;

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| HelmError::Io(format!("list_tags: build HTTP client: {e}")))?;

    let token = get_token(&client, "list_tags", &host, &repo).ok().flatten();

    let url = format!(
        "{}://{host}/v2/{repo}/tags/list?n=200",
        registry_scheme(&host)
    );
    let mut builder = client.get(&url).header("User-Agent", "husako");
    if let Some(ref tok) = token {
        builder = builder.bearer_auth(tok);
    }

    let body: serde_json::Value = builder
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| HelmError::Io(format!("list_tags: fetch tags for {reference}: {e}")))?
        .json()
        .map_err(|e| HelmError::Io(format!("list_tags: parse tags response: {e}")))?;

    let mut tags: Vec<semver::Version> = body["tags"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|v| v.as_str())
        .filter_map(|tag| {
            let stripped = tag.strip_prefix('v').unwrap_or(tag);
            semver::Version::parse(stripped)
                .ok()
                .filter(|v| v.pre.is_empty())
        })
        .collect();

    tags.sort_by(|a, b| b.cmp(a));

    // Reconstruct the original tag string form (no 'v' prefix added — keep as parsed)
    // Re-fetch the raw strings that match the parsed versions
    let raw_tags: Vec<String> = {
        let matching: std::collections::HashSet<semver::Version> = tags.iter().cloned().collect();
        let mut ordered: Vec<(semver::Version, String)> = body["tags"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| v.as_str())
            .filter_map(|tag| {
                let stripped = tag.strip_prefix('v').unwrap_or(tag);
                semver::Version::parse(stripped)
                    .ok()
                    .filter(|v| v.pre.is_empty() && matching.contains(v))
                    .map(|v| (v, tag.to_owned()))
            })
            .collect();
        ordered.sort_by(|a, b| b.0.cmp(&a.0));
        ordered
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|(_, tag)| tag)
            .collect()
    };

    Ok(raw_tags)
}

/// Parse an OCI reference into (host, repository, optional_tag).
///
/// Examples:
///   `"oci://registry-1.docker.io/bitnamicharts/postgresql"`
///     → `("registry-1.docker.io", "bitnamicharts/postgresql", None)`
///   `"oci://ghcr.io/org/chart:1.0.0"`
///     → `("ghcr.io", "org/chart", Some("1.0.0"))`
fn parse_oci_reference(reference: &str) -> Result<(String, String, Option<String>), HelmError> {
    let without_scheme = reference
        .strip_prefix("oci://")
        .ok_or_else(|| HelmError::Io(format!("not an OCI reference: {reference}")))?;

    let (host, rest) = without_scheme.split_once('/').ok_or_else(|| {
        HelmError::Io(format!(
            "invalid OCI reference (no path after host): {reference}"
        ))
    })?;

    if host.is_empty() || rest.is_empty() {
        return Err(HelmError::Io(format!(
            "invalid OCI reference (empty host or path): {reference}"
        )));
    }

    let (repo, tag) = if let Some((r, t)) = rest.rsplit_once(':') {
        (r.to_string(), Some(t.to_string()))
    } else {
        (rest.to_string(), None)
    };

    Ok((host.to_string(), repo, tag))
}

/// Returns `"http"` for loopback hosts used in tests, `"https"` everywhere else.
fn registry_scheme(host: &str) -> &'static str {
    let bare = host.split(':').next().unwrap_or(host);
    if cfg!(test) && (bare == "127.0.0.1" || bare == "localhost") {
        "http"
    } else {
        "https"
    }
}

/// Attempt to obtain an anonymous bearer token for the registry.
///
/// Returns `Ok(None)` when the registry accepts requests without authentication.
fn get_token(
    client: &reqwest::blocking::Client,
    name: &str,
    host: &str,
    repo: &str,
) -> Result<Option<String>, HelmError> {
    let scheme = registry_scheme(host);
    let ping_url = format!("{scheme}://{host}/v2/");
    let ping_resp = client
        .get(&ping_url)
        .header("User-Agent", "husako")
        .send()
        .map_err(|e| HelmError::Io(format!("chart '{name}': OCI ping {ping_url}: {e}")))?;

    if ping_resp.status().is_success() {
        return Ok(None);
    }

    if ping_resp.status().as_u16() != 401 {
        return Err(HelmError::Io(format!(
            "chart '{name}': OCI ping {ping_url} returned {}",
            ping_resp.status()
        )));
    }

    let www_auth = ping_resp
        .headers()
        .get("www-authenticate")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            HelmError::Io(format!(
                "chart '{name}': OCI registry {host} returned 401 without WWW-Authenticate"
            ))
        })?
        .to_owned();

    let (realm, service, _scope) = parse_www_authenticate(&www_auth).ok_or_else(|| {
        HelmError::Io(format!(
            "chart '{name}': could not parse WWW-Authenticate: {www_auth}"
        ))
    })?;

    let mut token_url = format!("{realm}?scope=repository:{repo}:pull");
    if let Some(svc) = service {
        token_url.push_str(&format!("&service={svc}"));
    }

    let token_resp = client
        .get(&token_url)
        .header("User-Agent", "husako")
        .send()
        .map_err(|e| HelmError::Io(format!("chart '{name}': OCI token fetch {token_url}: {e}")))?;

    if !token_resp.status().is_success() {
        return Err(HelmError::Io(format!(
            "chart '{name}': OCI token fetch returned {}",
            token_resp.status()
        )));
    }

    let body: serde_json::Value = token_resp
        .json()
        .map_err(|e| HelmError::Io(format!("chart '{name}': parse token response: {e}")))?;

    let token = body
        .get("token")
        .or_else(|| body.get("access_token"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| {
            HelmError::Io(format!(
                "chart '{name}': no token field in OCI auth response"
            ))
        })?;

    Ok(Some(token.to_string()))
}

/// Parse a `WWW-Authenticate: Bearer ...` header value.
///
/// Returns `(realm, service, scope)` where service and scope may be absent.
fn parse_www_authenticate(header: &str) -> Option<(String, Option<String>, Option<String>)> {
    let rest = header.strip_prefix("Bearer ")?;

    let mut realm: Option<String> = None;
    let mut service: Option<String> = None;
    let mut scope: Option<String> = None;

    let mut remaining = rest;
    while !remaining.is_empty() {
        remaining = remaining.trim_start_matches([' ', ',']);
        if remaining.is_empty() {
            break;
        }

        let eq_pos = remaining.find('=')?;
        let key = &remaining[..eq_pos];
        remaining = &remaining[eq_pos + 1..];

        let value = if remaining.starts_with('"') {
            remaining = &remaining[1..];
            let close = remaining.find('"')?;
            let v = remaining[..close].to_string();
            remaining = &remaining[close + 1..];
            v
        } else {
            let end = remaining.find([',', ' ']).unwrap_or(remaining.len());
            let v = remaining[..end].to_string();
            remaining = &remaining[end..];
            v
        };

        match key {
            "realm" => realm = Some(value),
            "service" => service = Some(value),
            "scope" => scope = Some(value),
            _ => {}
        }
    }

    realm.map(|r| (r, service, scope))
}

/// Fetch the OCI manifest for the given reference.
///
/// Handles OCI image index by picking the first manifest entry and recursing once.
fn fetch_manifest(
    client: &reqwest::blocking::Client,
    name: &str,
    host: &str,
    repo: &str,
    tag_or_digest: &str,
    token: Option<&str>,
) -> Result<serde_json::Value, HelmError> {
    let scheme = registry_scheme(host);
    let url = format!("{scheme}://{host}/v2/{repo}/manifests/{tag_or_digest}");

    let mut req = client
        .get(&url)
        .header("Accept", MANIFEST_ACCEPT)
        .header("User-Agent", "husako");

    if let Some(tok) = token {
        req = req.bearer_auth(tok);
    }

    let resp = req
        .send()
        .map_err(|e| HelmError::Io(format!("chart '{name}': fetch manifest {url}: {e}")))?;

    if !resp.status().is_success() {
        return Err(HelmError::Io(format!(
            "chart '{name}': fetch manifest {url} returned {}",
            resp.status()
        )));
    }

    let manifest: serde_json::Value = resp
        .json()
        .map_err(|e| HelmError::Io(format!("chart '{name}': parse manifest from {url}: {e}")))?;

    // Handle OCI image index (multi-platform manifest list): pick the first entry
    if manifest.get("manifests").is_some() {
        let digest = manifest
            .get("manifests")
            .and_then(|m| m.as_array())
            .and_then(|arr| arr.first())
            .and_then(|entry| entry.get("digest"))
            .and_then(|d| d.as_str())
            .ok_or_else(|| {
                HelmError::NotFound(format!(
                    "chart '{name}': OCI image index has no manifest entries"
                ))
            })?
            .to_owned();

        return fetch_manifest(client, name, host, repo, &digest, token);
    }

    Ok(manifest)
}

/// Find the digest of the Helm chart content layer in an OCI manifest.
fn find_chart_layer_digest(name: &str, manifest: &serde_json::Value) -> Result<String, HelmError> {
    let layers = manifest
        .get("layers")
        .and_then(|l| l.as_array())
        .ok_or_else(|| {
            HelmError::NotFound(format!(
                "chart '{name}': OCI manifest has no 'layers' array"
            ))
        })?;

    // Primary: Helm chart content media type
    for layer in layers {
        if layer.get("mediaType").and_then(|t| t.as_str()) == Some(HELM_LAYER_MEDIA_TYPE)
            && let Some(digest) = layer.get("digest").and_then(|d| d.as_str())
        {
            return Ok(digest.to_string());
        }
    }

    // Fallback: generic tar+gzip
    for layer in layers {
        if layer.get("mediaType").and_then(|t| t.as_str()) == Some(TAR_GZ_MEDIA_TYPE)
            && let Some(digest) = layer.get("digest").and_then(|d| d.as_str())
        {
            return Ok(digest.to_string());
        }
    }

    Err(HelmError::NotFound(format!(
        "chart '{name}': OCI manifest has no Helm chart content layer \
         (expected media type '{HELM_LAYER_MEDIA_TYPE}')"
    )))
}

/// Download a blob from an OCI registry.
fn download_blob(
    client: &reqwest::blocking::Client,
    name: &str,
    host: &str,
    repo: &str,
    digest: &str,
    token: Option<&str>,
) -> Result<Vec<u8>, HelmError> {
    let scheme = registry_scheme(host);
    let url = format!("{scheme}://{host}/v2/{repo}/blobs/{digest}");

    let mut req = client.get(&url).header("User-Agent", "husako");

    if let Some(tok) = token {
        req = req.bearer_auth(tok);
    }

    let resp = req
        .send()
        .map_err(|e| HelmError::Io(format!("chart '{name}': download blob {url}: {e}")))?;

    if !resp.status().is_success() {
        return Err(HelmError::Io(format!(
            "chart '{name}': download blob {url} returned {}",
            resp.status()
        )));
    }

    resp.bytes()
        .map(|b| b.to_vec())
        .map_err(|e| HelmError::Io(format!("chart '{name}': read blob bytes from {url}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── chart_name_from_reference ─────────────────────────────────────────────

    #[test]
    fn chart_name_from_reference_basic() {
        assert_eq!(
            chart_name_from_reference("oci://ghcr.io/org/postgresql"),
            "postgresql"
        );
    }

    #[test]
    fn chart_name_from_reference_with_tag() {
        assert_eq!(
            chart_name_from_reference("oci://ghcr.io/org/postgresql:1.2.3"),
            "postgresql"
        );
    }

    #[test]
    fn chart_name_from_reference_docker_hub() {
        assert_eq!(
            chart_name_from_reference("oci://registry-1.docker.io/bitnamicharts/postgresql"),
            "postgresql"
        );
    }

    // ── parse_oci_reference ───────────────────────────────────────────────────

    #[test]
    fn parse_oci_reference_basic() {
        let (host, repo, tag) =
            parse_oci_reference("oci://registry-1.docker.io/bitnamicharts/postgresql").unwrap();
        assert_eq!(host, "registry-1.docker.io");
        assert_eq!(repo, "bitnamicharts/postgresql");
        assert_eq!(tag, None);
    }

    #[test]
    fn parse_oci_reference_with_tag() {
        let (host, repo, tag) = parse_oci_reference("oci://ghcr.io/org/chart:1.0.0").unwrap();
        assert_eq!(host, "ghcr.io");
        assert_eq!(repo, "org/chart");
        assert_eq!(tag, Some("1.0.0".to_string()));
    }

    #[test]
    fn parse_oci_reference_with_host_port() {
        let (host, repo, tag) =
            parse_oci_reference("oci://registry.example.com:5000/myorg/mychart").unwrap();
        assert_eq!(host, "registry.example.com:5000");
        assert_eq!(repo, "myorg/mychart");
        assert_eq!(tag, None);
    }

    #[test]
    fn parse_oci_reference_invalid_scheme() {
        assert!(parse_oci_reference("https://registry.io/chart").is_err());
    }

    #[test]
    fn parse_oci_reference_no_path() {
        assert!(parse_oci_reference("oci://registry.io").is_err());
    }

    // ── parse_www_authenticate ────────────────────────────────────────────────

    #[test]
    fn parse_www_authenticate_full_docker_hub() {
        let header = r#"Bearer realm="https://auth.docker.io/token",service="registry.docker.io",scope="repository:bitnamicharts/postgresql:pull""#;
        let (realm, service, scope) = parse_www_authenticate(header).unwrap();
        assert_eq!(realm, "https://auth.docker.io/token");
        assert_eq!(service, Some("registry.docker.io".to_string()));
        assert_eq!(
            scope,
            Some("repository:bitnamicharts/postgresql:pull".to_string())
        );
    }

    #[test]
    fn parse_www_authenticate_realm_only() {
        let header = r#"Bearer realm="https://ghcr.io/token""#;
        let (realm, service, scope) = parse_www_authenticate(header).unwrap();
        assert_eq!(realm, "https://ghcr.io/token");
        assert_eq!(service, None);
        assert_eq!(scope, None);
    }

    #[test]
    fn parse_www_authenticate_not_bearer() {
        assert!(parse_www_authenticate("Basic realm=\"Registry\"").is_none());
    }

    // ── find_chart_layer_digest ───────────────────────────────────────────────

    #[test]
    fn find_chart_layer_helm_media_type_preferred() {
        let manifest = serde_json::json!({
            "layers": [
                {
                    "mediaType": "application/vnd.cncf.helm.chart.provenance.v1",
                    "digest": "sha256:aaa"
                },
                {
                    "mediaType": HELM_LAYER_MEDIA_TYPE,
                    "digest": "sha256:bbb"
                }
            ]
        });
        let digest = find_chart_layer_digest("test", &manifest).unwrap();
        assert_eq!(digest, "sha256:bbb");
    }

    #[test]
    fn find_chart_layer_fallback_tar_gz() {
        let manifest = serde_json::json!({
            "layers": [
                {
                    "mediaType": TAR_GZ_MEDIA_TYPE,
                    "digest": "sha256:ccc"
                }
            ]
        });
        let digest = find_chart_layer_digest("test", &manifest).unwrap();
        assert_eq!(digest, "sha256:ccc");
    }

    #[test]
    fn find_chart_layer_not_found() {
        let manifest = serde_json::json!({
            "layers": [
                {
                    "mediaType": "application/vnd.cncf.helm.chart.provenance.v1",
                    "digest": "sha256:aaa"
                }
            ]
        });
        let err = find_chart_layer_digest("test", &manifest).unwrap_err();
        assert!(err.to_string().contains("no Helm chart content layer"));
    }

    #[test]
    fn find_chart_layer_no_layers_field() {
        let manifest = serde_json::json!({ "mediaType": "something" });
        let err = find_chart_layer_digest("test", &manifest).unwrap_err();
        assert!(err.to_string().contains("no 'layers' array"));
    }

    // ── cache hit ─────────────────────────────────────────────────────────────

    #[test]
    fn cache_hit_returns_cached_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path();
        let reference = "oci://registry-1.docker.io/bitnamicharts/postgresql";
        let cache_key = crate::cache_hash(reference);
        let cache_sub = cache_dir.join(format!("helm/oci/{cache_key}"));
        std::fs::create_dir_all(&cache_sub).unwrap();
        std::fs::write(
            cache_sub.join("16.4.0.json"),
            r#"{"type":"object","properties":{"replicaCount":{"type":"integer"}}}"#,
        )
        .unwrap();

        let result = resolve("test", reference, "postgresql", "16.4.0", cache_dir).unwrap();
        assert_eq!(result["type"], "object");
        assert!(result["properties"]["replicaCount"].is_object());
    }

    // ── integration tests (mockito) ───────────────────────────────────────────

    fn make_chart_tgz(chart_name: &str) -> Vec<u8> {
        use flate2::write::GzEncoder;
        use std::io::Write;

        let schema = r#"{"type":"object","properties":{"replicaCount":{"type":"integer"}}}"#;
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header
            .set_path(format!("{chart_name}/values.schema.json"))
            .unwrap();
        header.set_size(schema.len() as u64);
        header.set_cksum();
        builder.append(&header, schema.as_bytes()).unwrap();
        let tar_data = builder.into_inner().unwrap();

        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&tar_data).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn oci_full_flow_no_auth() {
        let mut server = mockito::Server::new();

        let _m_ping = server.mock("GET", "/v2/").with_status(200).create();

        let manifest = serde_json::json!({
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "schemaVersion": 2,
            "layers": [{
                "mediaType": HELM_LAYER_MEDIA_TYPE,
                "digest": "sha256:fakedigest",
                "size": 1234
            }]
        });
        let _m_manifest = server
            .mock("GET", "/v2/myorg/mychart/manifests/1.0.0")
            .with_status(200)
            .with_header("content-type", "application/vnd.oci.image.manifest.v1+json")
            .with_body(manifest.to_string())
            .create();

        let blob = make_chart_tgz("mychart");
        let _m_blob = server
            .mock("GET", "/v2/myorg/mychart/blobs/sha256:fakedigest")
            .with_status(200)
            .with_body(blob)
            .create();

        let tmp = tempfile::tempdir().unwrap();
        let reference = format!("oci://{}/myorg/mychart", server.host_with_port());
        let result = resolve("test", &reference, "mychart", "1.0.0", tmp.path()).unwrap();
        assert_eq!(result["type"], "object");
        assert!(result["properties"]["replicaCount"].is_object());
    }

    #[test]
    fn oci_full_flow_bearer_auth() {
        let mut server = mockito::Server::new();
        let host = server.host_with_port();

        // Ping returns 401 with WWW-Authenticate pointing back to our mock server
        let token_path = "/token";
        let www_auth =
            format!(r#"Bearer realm="http://{host}{token_path}",service="mock-registry""#);
        let _m_ping = server
            .mock("GET", "/v2/")
            .with_status(401)
            .with_header("www-authenticate", &www_auth)
            .create();

        // Token endpoint — match path + query params separately
        let _m_token = server
            .mock("GET", "/token")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded(
                    "scope".into(),
                    "repository:myorg/mychart:pull".into(),
                ),
                mockito::Matcher::UrlEncoded("service".into(), "mock-registry".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"token":"test-bearer-token"}"#)
            .create();

        let manifest = serde_json::json!({
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "schemaVersion": 2,
            "layers": [{
                "mediaType": HELM_LAYER_MEDIA_TYPE,
                "digest": "sha256:authdigest",
                "size": 1234
            }]
        });
        let _m_manifest = server
            .mock("GET", "/v2/myorg/mychart/manifests/2.0.0")
            .match_header("authorization", "Bearer test-bearer-token")
            .with_status(200)
            .with_header("content-type", "application/vnd.oci.image.manifest.v1+json")
            .with_body(manifest.to_string())
            .create();

        let blob = make_chart_tgz("mychart");
        let _m_blob = server
            .mock("GET", "/v2/myorg/mychart/blobs/sha256:authdigest")
            .match_header("authorization", "Bearer test-bearer-token")
            .with_status(200)
            .with_body(blob)
            .create();

        let tmp = tempfile::tempdir().unwrap();
        let reference = format!("oci://{host}/myorg/mychart");
        let result = resolve("test", &reference, "mychart", "2.0.0", tmp.path()).unwrap();
        assert_eq!(result["type"], "object");
        assert!(result["properties"]["replicaCount"].is_object());
    }

    #[test]
    fn list_tags_returns_sorted_semver_tags() {
        let mut server = mockito::Server::new();

        let _m_ping = server.mock("GET", "/v2/").with_status(200).create();

        let tags_resp = serde_json::json!({
            "name": "myorg/mychart",
            "tags": ["1.0.0", "2.0.0", "1.5.0", "not-semver", "3.0.0-alpha.1"]
        });
        let _m_tags = server
            .mock("GET", "/v2/myorg/mychart/tags/list")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(tags_resp.to_string())
            .create();

        let reference = format!("oci://{}/myorg/mychart", server.host_with_port());
        let tags = list_tags(&reference, 10, 0).unwrap();
        // Sorted descending, only stable semver, max 10
        assert_eq!(tags, vec!["2.0.0", "1.5.0", "1.0.0"]);
    }

    #[test]
    fn list_tags_offset_and_limit() {
        let mut server = mockito::Server::new();

        let _m_ping = server.mock("GET", "/v2/").with_status(200).create();

        let tags_resp = serde_json::json!({
            "name": "myorg/mychart",
            "tags": ["3.0.0", "2.0.0", "1.0.0"]
        });
        let _m_tags = server
            .mock("GET", "/v2/myorg/mychart/tags/list")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(tags_resp.to_string())
            .create();

        let reference = format!("oci://{}/myorg/mychart", server.host_with_port());
        let tags = list_tags(&reference, 1, 1).unwrap();
        assert_eq!(tags, vec!["2.0.0"]);
    }

    #[test]
    fn oci_image_index_picks_first_manifest() {
        let mut server = mockito::Server::new();

        let _m_ping = server.mock("GET", "/v2/").with_status(200).create();

        // First request returns an image index
        let index = serde_json::json!({
            "mediaType": "application/vnd.oci.image.index.v1+json",
            "schemaVersion": 2,
            "manifests": [
                { "digest": "sha256:manifest1", "platform": { "os": "linux" } }
            ]
        });
        let _m_index = server
            .mock("GET", "/v2/myorg/multichart/manifests/3.0.0")
            .with_status(200)
            .with_header("content-type", "application/vnd.oci.image.index.v1+json")
            .with_body(index.to_string())
            .create();

        // Second request (recurse with digest) returns the actual manifest
        let manifest = serde_json::json!({
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "schemaVersion": 2,
            "layers": [{
                "mediaType": HELM_LAYER_MEDIA_TYPE,
                "digest": "sha256:indexblob",
                "size": 100
            }]
        });
        let _m_manifest = server
            .mock("GET", "/v2/myorg/multichart/manifests/sha256:manifest1")
            .with_status(200)
            .with_header("content-type", "application/vnd.oci.image.manifest.v1+json")
            .with_body(manifest.to_string())
            .create();

        let blob = make_chart_tgz("multichart");
        let _m_blob = server
            .mock("GET", "/v2/myorg/multichart/blobs/sha256:indexblob")
            .with_status(200)
            .with_body(blob)
            .create();

        let tmp = tempfile::tempdir().unwrap();
        let reference = format!("oci://{}/myorg/multichart", server.host_with_port());
        let result = resolve("test", &reference, "multichart", "3.0.0", tmp.path()).unwrap();
        assert_eq!(result["type"], "object");
    }
}
