use crate::HusakoError;

const GITHUB_API_BASE: &str = "https://api.github.com";
const ARTIFACTHUB_BASE: &str = "https://artifacthub.io";

/// Return true if `version` matches a partial semver prefix.
///
/// Leading `v` is optional on both sides. Examples:
/// - `"16"` matches `"16.4.0"` and `"16.0.0"` but not `"17.0.0"`
/// - `"16.4"` matches `"16.4.0"` and `"16.4.1"` but not `"16.5.0"`
/// - `"16.4.0"` matches only `"16.4.0"` (exact)
pub fn version_matches_prefix(version: &str, prefix: &str) -> bool {
    let v = version.strip_prefix('v').unwrap_or(version);
    let p = prefix.strip_prefix('v').unwrap_or(prefix);
    v == p || v.starts_with(&format!("{p}."))
}

// TODO: use urlencoding instead
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    out
}

// --- ArtifactHub search types ---

#[derive(Debug, serde::Deserialize)]
pub struct ArtifactHubPackage {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub repository: ArtifactHubRepo,
}

#[derive(Debug, serde::Deserialize)]
pub struct ArtifactHubRepo {
    pub name: String,
}

pub struct ArtifactHubSearchResult {
    pub packages: Vec<ArtifactHubPackage>,
    pub has_more: bool,
}

pub const ARTIFACTHUB_PAGE_SIZE: usize = 20;

/// Search ArtifactHub for Helm charts matching the query.
pub async fn search_artifacthub(
    query: &str,
    offset: usize,
) -> Result<ArtifactHubSearchResult, HusakoError> {
    search_artifacthub_from(query, offset, ARTIFACTHUB_BASE).await
}

async fn search_artifacthub_from(
    query: &str,
    offset: usize,
    base_url: &str,
) -> Result<ArtifactHubSearchResult, HusakoError> {
    let client = reqwest::Client::builder()
        .user_agent("husako")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| HusakoError::GenerateIo(format!("HTTP client: {e}")))?;

    let encoded_query = percent_encode(query);
    let limit = ARTIFACTHUB_PAGE_SIZE + 1;
    let url = format!(
        "{base_url}/api/v1/packages/search?ts_query_web={encoded_query}&kind=0&limit={limit}&offset={offset}"
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| HusakoError::GenerateIo(format!("ArtifactHub search: {e}")))?;

    let mut packages: Vec<ArtifactHubPackage> = resp
        .json::<serde_json::Value>()
        .await
        .map_err(|e| HusakoError::GenerateIo(format!("parse ArtifactHub search: {e}")))?
        .get("packages")
        .cloned()
        .unwrap_or(serde_json::Value::Array(vec![]))
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();

    let has_more = packages.len() > ARTIFACTHUB_PAGE_SIZE;
    packages.truncate(ARTIFACTHUB_PAGE_SIZE);

    Ok(ArtifactHubSearchResult { packages, has_more })
}

/// Discover the N most recent stable Kubernetes release versions (major.minor).
pub async fn discover_recent_releases(
    limit: usize,
    offset: usize,
) -> Result<Vec<String>, HusakoError> {
    discover_recent_releases_from(limit, offset, GITHUB_API_BASE).await
}

async fn discover_recent_releases_from(
    limit: usize,
    offset: usize,
    base_url: &str,
) -> Result<Vec<String>, HusakoError> {
    let client = reqwest::Client::builder()
        .user_agent("husako")
        .build()
        .map_err(|e| HusakoError::GenerateIo(format!("HTTP client: {e}")))?;

    let resp = client
        .get(format!(
            "{base_url}/repos/kubernetes/kubernetes/tags?per_page=100"
        ))
        .send()
        .await
        .map_err(|e| HusakoError::GenerateIo(format!("GitHub API: {e}")))?;

    let tags: Vec<serde_json::Value> = resp
        .json()
        .await
        .map_err(|e| HusakoError::GenerateIo(format!("parse tags: {e}")))?;

    let mut versions: Vec<semver::Version> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for tag in &tags {
        let Some(name) = tag["name"].as_str() else {
            continue;
        };
        let stripped = name.strip_prefix('v').unwrap_or(name);
        if stripped.contains('-') {
            continue;
        }
        if let Ok(v) = semver::Version::parse(stripped) {
            let key = format!("{}.{}", v.major, v.minor);
            if seen.insert(key) {
                versions.push(v);
            }
        }
    }

    versions.sort_by(|a, b| b.cmp(a));

    Ok(versions
        .iter()
        .skip(offset)
        .take(limit)
        .map(|v| format!("{}.{}", v.major, v.minor))
        .collect())
}

/// Discover available versions for a chart from a Helm registry.
pub async fn discover_registry_versions(
    repo: &str,
    chart: &str,
    limit: usize,
    offset: usize,
) -> Result<Vec<String>, HusakoError> {
    let url = format!("{}/index.yaml", repo.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .user_agent("husako")
        .build()
        .map_err(|e| HusakoError::GenerateIo(format!("HTTP client: {e}")))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| HusakoError::GenerateIo(format!("fetch registry index: {e}")))?;

    let text = resp
        .text()
        .await
        .map_err(|e| HusakoError::GenerateIo(format!("read registry index: {e}")))?;

    let index: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text)
        .map_err(|e| HusakoError::GenerateIo(format!("parse registry index: {e}")))?;

    let entries = index
        .get("entries")
        .and_then(|e| e.get(chart))
        .and_then(|e| e.as_sequence())
        .ok_or_else(|| {
            HusakoError::GenerateIo(format!("chart '{chart}' not found in registry index"))
        })?;

    let mut versions: Vec<semver::Version> = Vec::new();
    for entry in entries {
        let Some(version_str) = entry.get("version").and_then(|v| v.as_str()) else {
            continue;
        };
        if let Ok(v) = semver::Version::parse(version_str)
            && v.pre.is_empty()
        {
            versions.push(v);
        }
    }

    versions.sort_by(|a, b| b.cmp(a));

    Ok(versions
        .iter()
        .skip(offset)
        .take(limit)
        .map(|v| v.to_string())
        .collect())
}

/// Discover the latest stable Kubernetes release version from GitHub API.
pub async fn discover_latest_release() -> Result<String, HusakoError> {
    discover_latest_release_from(GITHUB_API_BASE).await
}

async fn discover_latest_release_from(base_url: &str) -> Result<String, HusakoError> {
    let client = reqwest::Client::builder()
        .user_agent("husako")
        .build()
        .map_err(|e| HusakoError::GenerateIo(format!("HTTP client: {e}")))?;

    let resp = client
        .get(format!(
            "{base_url}/repos/kubernetes/kubernetes/tags?per_page=100"
        ))
        .send()
        .await
        .map_err(|e| HusakoError::GenerateIo(format!("GitHub API: {e}")))?;

    let tags: Vec<serde_json::Value> = resp
        .json()
        .await
        .map_err(|e| HusakoError::GenerateIo(format!("parse tags: {e}")))?;

    let mut best: Option<semver::Version> = None;

    for tag in &tags {
        let Some(name) = tag["name"].as_str() else {
            continue;
        };
        let stripped = name.strip_prefix('v').unwrap_or(name);

        // Skip pre-release tags (alpha, beta, rc)
        if stripped.contains('-') {
            continue;
        }

        if let Ok(v) = semver::Version::parse(stripped)
            && best.as_ref().is_none_or(|b| v > *b)
        {
            best = Some(v);
        }
    }

    best.map(|v| format!("{}.{}", v.major, v.minor))
        .ok_or_else(|| HusakoError::GenerateIo("no stable release tags found".to_string()))
}

/// Discover the latest version from a Helm chart registry's index.yaml.
///
/// When `prefix` is `Some("16")`, returns the latest version whose numeric part starts with
/// `"16."` (e.g. `"16.4.0"`). Leading `v` is optional. When `None`, returns the overall latest.
pub async fn discover_latest_registry(
    repo: &str,
    chart: &str,
    prefix: Option<&str>,
) -> Result<String, HusakoError> {
    let url = format!("{}/index.yaml", repo.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .user_agent("husako")
        .build()
        .map_err(|e| HusakoError::GenerateIo(format!("HTTP client: {e}")))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| HusakoError::GenerateIo(format!("fetch registry index: {e}")))?;

    let text = resp
        .text()
        .await
        .map_err(|e| HusakoError::GenerateIo(format!("read registry index: {e}")))?;

    let index: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text)
        .map_err(|e| HusakoError::GenerateIo(format!("parse registry index: {e}")))?;

    let entries = index
        .get("entries")
        .and_then(|e| e.get(chart))
        .and_then(|e| e.as_sequence())
        .ok_or_else(|| {
            HusakoError::GenerateIo(format!("chart '{chart}' not found in registry index"))
        })?;

    let mut best: Option<(semver::Version, String)> = None;

    for entry in entries {
        let Some(version_str) = entry.get("version").and_then(|v| v.as_str()) else {
            continue;
        };
        if let Some(pfx) = prefix
            && !version_matches_prefix(version_str, pfx)
        {
            continue;
        }
        if let Ok(v) = semver::Version::parse(version_str.strip_prefix('v').unwrap_or(version_str))
            && v.pre.is_empty()
            && best.as_ref().is_none_or(|(b, _)| v > *b)
        {
            best = Some((v, version_str.to_string()));
        }
    }

    best.map(|(_, s)| s).ok_or_else(|| {
        if let Some(pfx) = prefix {
            HusakoError::GenerateIo(format!(
                "no versions matching '{pfx}' found for chart '{chart}'"
            ))
        } else {
            HusakoError::GenerateIo(format!("no versions found for chart '{chart}'"))
        }
    })
}

/// Discover the latest version from ArtifactHub API.
///
/// When `prefix` is `Some("16")`, returns the latest version matching the prefix.
/// When `None`, returns the overall latest (from the `version` field directly).
pub async fn discover_latest_artifacthub(
    package: &str,
    prefix: Option<&str>,
) -> Result<String, HusakoError> {
    discover_latest_artifacthub_from(package, prefix, ARTIFACTHUB_BASE).await
}

async fn discover_latest_artifacthub_from(
    package: &str,
    prefix: Option<&str>,
    base_url: &str,
) -> Result<String, HusakoError> {
    let url = format!(
        "{base_url}/api/v1/packages/helm/{}",
        package.trim_start_matches('/')
    );
    let client = reqwest::Client::builder()
        .user_agent("husako")
        .build()
        .map_err(|e| HusakoError::GenerateIo(format!("HTTP client: {e}")))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| HusakoError::GenerateIo(format!("ArtifactHub API: {e}")))?;

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| HusakoError::GenerateIo(format!("parse ArtifactHub response: {e}")))?;

    if let Some(pfx) = prefix {
        // Filter available_versions by prefix, return highest matching
        let all_versions = parse_artifacthub_versions(&data, usize::MAX, 0);
        all_versions
            .into_iter()
            .find(|v| version_matches_prefix(v, pfx))
            .ok_or_else(|| {
                HusakoError::GenerateIo(format!("no versions matching '{pfx}' for '{package}'"))
            })
    } else {
        data["version"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| {
                HusakoError::GenerateIo(format!(
                    "no version field in ArtifactHub response for '{package}'"
                ))
            })
    }
}

/// Discover available versions for a package from ArtifactHub API.
/// Returns up to `limit` stable versions, sorted newest first.
pub async fn discover_artifacthub_versions(
    package: &str,
    limit: usize,
    offset: usize,
) -> Result<Vec<String>, HusakoError> {
    discover_artifacthub_versions_from(package, limit, offset, ARTIFACTHUB_BASE).await
}

async fn discover_artifacthub_versions_from(
    package: &str,
    limit: usize,
    offset: usize,
    base_url: &str,
) -> Result<Vec<String>, HusakoError> {
    let url = format!(
        "{base_url}/api/v1/packages/helm/{}",
        package.trim_start_matches('/')
    );
    let client = reqwest::Client::builder()
        .user_agent("husako")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| HusakoError::GenerateIo(format!("HTTP client: {e}")))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| HusakoError::GenerateIo(format!("ArtifactHub API: {e}")))?;

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| HusakoError::GenerateIo(format!("parse ArtifactHub response: {e}")))?;

    let versions = parse_artifacthub_versions(&data, limit, offset);
    Ok(versions)
}

/// Parse and sort ArtifactHub `available_versions` by semver descending.
/// Filters out pre-release entries and non-semver strings.
fn parse_artifacthub_versions(
    data: &serde_json::Value,
    limit: usize,
    offset: usize,
) -> Vec<String> {
    let mut parsed: Vec<semver::Version> = data["available_versions"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter(|entry| !entry["prerelease"].as_bool().unwrap_or(false))
        .filter_map(|entry| entry["version"].as_str())
        .filter_map(|v| semver::Version::parse(v).ok())
        .filter(|v| v.pre.is_empty())
        .collect();

    parsed.sort_by(|a, b| b.cmp(a));

    parsed
        .iter()
        .skip(offset)
        .take(limit)
        .map(|v| v.to_string())
        .collect()
}

/// Discover the latest tag from a git repository using `git ls-remote --tags`.
///
/// When `prefix` is `Some("v1")`, returns the latest tag whose version matches the prefix.
/// When `None`, returns the overall latest stable tag.
///
/// Intentionally sync: called from async contexts without needing block_in_place,
/// and also from the update/outdated logic.
pub fn discover_latest_git_tag(
    repo: &str,
    prefix: Option<&str>,
) -> Result<Option<String>, HusakoError> {
    let output = std::process::Command::new("git")
        .args(["ls-remote", "--tags", "--sort=-v:refname", repo])
        .output()
        .map_err(|e| HusakoError::GenerateIo(format!("git ls-remote: {e}")))?;

    if !output.status.success() {
        return Err(HusakoError::GenerateIo(format!(
            "git ls-remote failed for '{repo}'"
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut best: Option<(semver::Version, String)> = None;

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 {
            continue;
        }
        let refname = parts[1];
        let tag = refname
            .strip_prefix("refs/tags/")
            .unwrap_or(refname)
            .trim_end_matches("^{}");

        if let Some(pfx) = prefix
            && !version_matches_prefix(tag, pfx)
        {
            continue;
        }

        let stripped = tag.strip_prefix('v').unwrap_or(tag);
        if let Ok(v) = semver::Version::parse(stripped)
            && v.pre.is_empty()
            && best.as_ref().is_none_or(|(b, _)| v > *b)
        {
            best = Some((v, tag.to_string()));
        }
    }

    Ok(best.map(|(_, tag)| tag))
}

/// Discover recent stable tags from a git repository.
/// Returns up to `limit` stable semver tags, sorted newest first.
/// Intentionally sync — see `discover_latest_git_tag` for rationale.
pub fn discover_git_tags(
    repo: &str,
    limit: usize,
    offset: usize,
) -> Result<Vec<String>, HusakoError> {
    let output = std::process::Command::new("git")
        .args(["ls-remote", "--tags", "--sort=-v:refname", repo])
        .output()
        .map_err(|e| HusakoError::GenerateIo(format!("git ls-remote: {e}")))?;

    if !output.status.success() {
        return Err(HusakoError::GenerateIo(format!(
            "git ls-remote failed for '{repo}'"
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut seen = std::collections::HashSet::new();
    let mut entries: Vec<(semver::Version, String)> = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 {
            continue;
        }
        let refname = parts[1];
        let tag = refname
            .strip_prefix("refs/tags/")
            .unwrap_or(refname)
            .trim_end_matches("^{}");

        let stripped = tag.strip_prefix('v').unwrap_or(tag);
        if let Ok(v) = semver::Version::parse(stripped)
            && v.pre.is_empty()
            && seen.insert(tag.to_string())
        {
            entries.push((v, tag.to_string()));
        }
    }

    entries.sort_by(|a, b| b.0.cmp(&a.0));

    Ok(entries
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|(_, tag)| tag)
        .collect())
}

/// Fetch up to `limit` available stable OCI tags for `reference`, starting at `offset`.
pub async fn discover_oci_tags(
    reference: &str,
    limit: usize,
    offset: usize,
) -> Result<Vec<String>, HusakoError> {
    husako_helm::oci::list_tags(reference, limit, offset)
        .await
        .map_err(|e| HusakoError::GenerateIo(e.to_string()))
}

/// Return the latest stable OCI tag for `reference`, or None if no tags found.
pub async fn discover_latest_oci(reference: &str) -> Result<Option<String>, HusakoError> {
    let tags = discover_oci_tags(reference, 1, 0).await?;
    Ok(tags.into_iter().next())
}

/// Compare two version strings for equivalence.
///
/// Handles cases like "1.35" matching "1.35" from latest discovery,
/// and "v1.17.2" matching "v1.17.2".
pub fn versions_match(current: &str, latest: &str) -> bool {
    if current == latest {
        return true;
    }

    // Try semver comparison: parse both (stripping 'v' prefix)
    let c = current.strip_prefix('v').unwrap_or(current);
    let l = latest.strip_prefix('v').unwrap_or(latest);

    // If current is just major.minor, check if latest starts with it
    if !c.contains('.') || c.matches('.').count() == 1 {
        return l.starts_with(c) || l == c;
    }

    c == l
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifacthub_package_deserialize() {
        let json = serde_json::json!({
            "name": "postgresql",
            "version": "16.4.0",
            "description": "PostgreSQL object-relational database",
            "repository": { "name": "bitnami" }
        });
        let pkg: ArtifactHubPackage = serde_json::from_value(json).unwrap();
        assert_eq!(pkg.name, "postgresql");
        assert_eq!(pkg.version, "16.4.0");
        assert_eq!(
            pkg.description.as_deref(),
            Some("PostgreSQL object-relational database")
        );
        assert_eq!(pkg.repository.name, "bitnami");
    }

    #[test]
    fn artifacthub_package_missing_description() {
        let json = serde_json::json!({
            "name": "test",
            "version": "1.0.0",
            "repository": { "name": "org" }
        });
        let pkg: ArtifactHubPackage = serde_json::from_value(json).unwrap();
        assert!(pkg.description.is_none());
    }

    #[test]
    fn artifacthub_has_more_detection() {
        // Simulate 21 results → has_more = true
        let packages: Vec<ArtifactHubPackage> = (0..21)
            .map(|i| ArtifactHubPackage {
                name: format!("pkg-{i}"),
                version: "1.0.0".to_string(),
                description: None,
                repository: ArtifactHubRepo {
                    name: "org".to_string(),
                },
            })
            .collect();
        let has_more = packages.len() > ARTIFACTHUB_PAGE_SIZE;
        assert!(has_more);

        // Simulate 15 results → has_more = false
        let packages: Vec<ArtifactHubPackage> = (0..15)
            .map(|i| ArtifactHubPackage {
                name: format!("pkg-{i}"),
                version: "1.0.0".to_string(),
                description: None,
                repository: ArtifactHubRepo {
                    name: "org".to_string(),
                },
            })
            .collect();
        let has_more = packages.len() > ARTIFACTHUB_PAGE_SIZE;
        assert!(!has_more);
    }

    #[test]
    fn artifacthub_display_formatting() {
        let pkg = ArtifactHubPackage {
            name: "postgresql".to_string(),
            version: "16.4.0".to_string(),
            description: Some("A very long description that should be truncated when displayed in the selection prompt for the user".to_string()),
            repository: ArtifactHubRepo {
                name: "bitnami".to_string(),
            },
        };
        let package_id = format!("{}/{}", pkg.repository.name, pkg.name);
        assert_eq!(package_id, "bitnami/postgresql");

        // Truncate description at 50 chars
        let desc = pkg.description.as_deref().unwrap_or("");
        let truncated = if desc.len() > 50 {
            format!("{}...", &desc[..50])
        } else {
            desc.to_string()
        };
        assert!(truncated.ends_with("..."));
        assert!(truncated.len() <= 53);
    }

    #[test]
    fn git_tags_multiple() {
        // Simulate the parsing logic from discover_git_tags
        let stdout = "\
abc123\trefs/tags/v2.0.0\n\
def456\trefs/tags/v2.0.0^{}\n\
ghi789\trefs/tags/v1.9.0\n\
jkl012\trefs/tags/v1.9.0^{}\n\
mno345\trefs/tags/v1.8.0-rc.1\n\
pqr678\trefs/tags/v1.8.0-rc.1^{}\n\
stu901\trefs/tags/v1.7.0\n\
vwx234\trefs/tags/v1.7.0^{}\n";

        let mut seen = std::collections::HashSet::new();
        let mut entries: Vec<(semver::Version, String)> = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 2 {
                continue;
            }
            let refname = parts[1];
            let tag = refname
                .strip_prefix("refs/tags/")
                .unwrap_or(refname)
                .trim_end_matches("^{}");

            let stripped = tag.strip_prefix('v').unwrap_or(tag);
            if let Ok(v) = semver::Version::parse(stripped)
                && v.pre.is_empty()
                && seen.insert(tag.to_string())
            {
                entries.push((v, tag.to_string()));
            }
        }

        entries.sort_by(|a, b| b.0.cmp(&a.0));
        entries.truncate(2);

        let tags: Vec<String> = entries.into_iter().map(|(_, tag)| tag).collect();
        assert_eq!(tags, vec!["v2.0.0", "v1.9.0"]);
    }

    #[test]
    fn artifacthub_versions_filters_prerelease() {
        let data = serde_json::json!({
            "available_versions": [
                {"version": "3.0.0", "prerelease": false},
                {"version": "3.0.0-rc.1", "prerelease": true},
                {"version": "2.5.0", "prerelease": false},
                {"version": "2.5.0-beta.1", "prerelease": true},
                {"version": "2.4.0", "prerelease": false},
            ]
        });
        let versions = parse_artifacthub_versions(&data, 10, 0);
        assert_eq!(versions, vec!["3.0.0", "2.5.0", "2.4.0"]);
    }

    #[test]
    fn artifacthub_versions_sorted_descending() {
        // API may return versions in arbitrary order
        let data = serde_json::json!({
            "available_versions": [
                {"version": "1.0.0", "prerelease": false},
                {"version": "3.0.0", "prerelease": false},
                {"version": "0.0.0", "prerelease": false},
                {"version": "2.1.0", "prerelease": false},
                {"version": "2.0.0", "prerelease": false},
            ]
        });
        let versions = parse_artifacthub_versions(&data, 10, 0);
        assert_eq!(versions, vec!["3.0.0", "2.1.0", "2.0.0", "1.0.0", "0.0.0"]);
    }

    #[test]
    fn artifacthub_versions_offset_and_limit() {
        let data = serde_json::json!({
            "available_versions": [
                {"version": "5.0.0", "prerelease": false},
                {"version": "4.0.0", "prerelease": false},
                {"version": "3.0.0", "prerelease": false},
                {"version": "2.0.0", "prerelease": false},
                {"version": "1.0.0", "prerelease": false},
            ]
        });
        // Skip first 2, take 2
        let versions = parse_artifacthub_versions(&data, 2, 2);
        assert_eq!(versions, vec!["3.0.0", "2.0.0"]);
    }

    #[test]
    fn artifacthub_versions_skips_invalid_semver() {
        let data = serde_json::json!({
            "available_versions": [
                {"version": "2.0.0", "prerelease": false},
                {"version": "not-a-version", "prerelease": false},
                {"version": "1.0.0", "prerelease": false},
            ]
        });
        let versions = parse_artifacthub_versions(&data, 10, 0);
        assert_eq!(versions, vec!["2.0.0", "1.0.0"]);
    }

    #[test]
    fn version_matches_prefix_major() {
        assert!(version_matches_prefix("16.4.0", "16"));
        assert!(version_matches_prefix("16.0.0", "16"));
        assert!(!version_matches_prefix("17.0.0", "16"));
        assert!(!version_matches_prefix("6.0.0", "16"));
    }

    #[test]
    fn version_matches_prefix_minor() {
        assert!(version_matches_prefix("16.4.0", "16.4"));
        assert!(version_matches_prefix("16.4.1", "16.4"));
        assert!(!version_matches_prefix("16.5.0", "16.4"));
    }

    #[test]
    fn version_matches_prefix_full() {
        assert!(version_matches_prefix("16.4.0", "16.4.0"));
        assert!(!version_matches_prefix("16.4.1", "16.4.0"));
    }

    #[test]
    fn version_matches_prefix_v_optional() {
        assert!(version_matches_prefix("v16.4.0", "16"));
        assert!(version_matches_prefix("16.4.0", "v16"));
        assert!(version_matches_prefix("v16.4.0", "v16.4"));
    }

    #[test]
    fn versions_match_exact() {
        assert!(versions_match("1.35", "1.35"));
        assert!(versions_match("v1.17.2", "v1.17.2"));
    }

    #[test]
    fn versions_match_prefix() {
        assert!(versions_match("1.35", "1.35.0"));
        assert!(versions_match("1.35", "1.35.1"));
    }

    #[test]
    fn versions_no_match() {
        assert!(!versions_match("1.35", "1.36"));
        assert!(!versions_match("v1.17.2", "v1.18.0"));
    }

    #[test]
    fn versions_match_v_prefix_stripped() {
        // v-prefix is stripped before comparison
        assert!(versions_match("v1.35.0", "1.35.0"));
        assert!(versions_match("1.35.0", "v1.35.0"));
        assert!(versions_match("v1.17.2", "v1.17.2"));
        assert!(!versions_match("v1.35.0", "v1.36.0"));
    }

    // ── percent_encode ────────────────────────────────────────────────────────

    #[test]
    fn percent_encode_safe_chars_unchanged() {
        assert_eq!(percent_encode("abc"), "abc");
        assert_eq!(percent_encode("ABC"), "ABC");
        assert_eq!(percent_encode("0123456789"), "0123456789");
        assert_eq!(percent_encode("-_.~"), "-_.~");
    }

    #[test]
    fn percent_encode_special_chars() {
        assert_eq!(percent_encode(" "), "%20");
        assert_eq!(percent_encode("/"), "%2F");
        assert_eq!(percent_encode(":"), "%3A");
        assert_eq!(percent_encode("a b/c:d"), "a%20b%2Fc%3Ad");
    }

    // ── search_artifacthub_from (mockito) ─────────────────────────────────────

    #[tokio::test]
    async fn search_artifacthub_returns_packages() {
        let mut server = mockito::Server::new_async().await;
        let resp = serde_json::json!({
            "packages": [
                {"name": "postgresql", "version": "16.4.0", "repository": {"name": "bitnami"}},
                {"name": "redis", "version": "20.0.0", "repository": {"name": "bitnami"}}
            ]
        });
        let _m = server
            .mock("GET", "/api/v1/packages/search")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(resp.to_string())
            .create_async()
            .await;

        let result = search_artifacthub_from("postgres", 0, &server.url())
            .await
            .unwrap();
        assert_eq!(result.packages.len(), 2);
        assert_eq!(result.packages[0].name, "postgresql");
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn search_artifacthub_empty_result() {
        let mut server = mockito::Server::new_async().await;
        let resp = serde_json::json!({"packages": []});
        let _m = server
            .mock("GET", "/api/v1/packages/search")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(resp.to_string())
            .create_async()
            .await;

        let result = search_artifacthub_from("nonexistent", 0, &server.url())
            .await
            .unwrap();
        assert!(result.packages.is_empty());
        assert!(!result.has_more);
    }

    // ── discover_recent_releases_from (mockito) ───────────────────────────────

    #[tokio::test]
    async fn discover_recent_releases_dedupes_minor() {
        let mut server = mockito::Server::new_async().await;
        let tags = serde_json::json!([
            {"name": "v1.35.1"},
            {"name": "v1.35.0"},
            {"name": "v1.34.0"}
        ]);
        let _m = server
            .mock("GET", "/repos/kubernetes/kubernetes/tags")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(tags.to_string())
            .create_async()
            .await;

        let versions = discover_recent_releases_from(10, 0, &server.url())
            .await
            .unwrap();
        // v1.35.1 and v1.35.0 share the same minor → only one "1.35" entry
        assert_eq!(versions, vec!["1.35", "1.34"]);
    }

    // ── discover_latest_release_from (mockito) ────────────────────────────────

    #[tokio::test]
    async fn discover_latest_release_returns_highest() {
        let mut server = mockito::Server::new_async().await;
        let tags = serde_json::json!([
            {"name": "v1.35.0-alpha.1"},
            {"name": "v1.34.0"},
            {"name": "v1.33.0"}
        ]);
        let _m = server
            .mock("GET", "/repos/kubernetes/kubernetes/tags")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(tags.to_string())
            .create_async()
            .await;

        let version = discover_latest_release_from(&server.url()).await.unwrap();
        assert_eq!(version, "1.34");
    }

    // ── discover_latest_artifacthub_from (mockito) ────────────────────────────

    #[tokio::test]
    async fn discover_latest_artifacthub_extracts_version() {
        let mut server = mockito::Server::new_async().await;
        let resp = serde_json::json!({
            "name": "postgresql",
            "version": "16.4.0",
            "available_versions": [{"version": "16.4.0", "prerelease": false}]
        });
        let _m = server
            .mock("GET", "/api/v1/packages/helm/bitnami/postgresql")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(resp.to_string())
            .create_async()
            .await;

        let version = discover_latest_artifacthub_from("bitnami/postgresql", None, &server.url())
            .await
            .unwrap();
        assert_eq!(version, "16.4.0");
    }

    #[tokio::test]
    async fn discover_latest_artifacthub_missing_version_field() {
        let mut server = mockito::Server::new_async().await;
        let resp = serde_json::json!({"name": "postgresql"});
        let _m = server
            .mock("GET", "/api/v1/packages/helm/bitnami/postgresql")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(resp.to_string())
            .create_async()
            .await;

        let err = discover_latest_artifacthub_from("bitnami/postgresql", None, &server.url())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no version field"));
    }

    // ── discover_registry_versions (mockito) ──────────────────────────────────

    fn make_index_yaml(chart: &str, versions: &[&str]) -> String {
        let entries: String = versions
            .iter()
            .map(|v| format!("    - version: \"{v}\"\n"))
            .collect();
        format!("apiVersion: v1\nentries:\n  {chart}:\n{entries}")
    }

    #[tokio::test]
    async fn discover_registry_versions_filters_prerelease_and_sorts() {
        let mut server = mockito::Server::new_async().await;
        let index = make_index_yaml("my-chart", &["1.0.0-beta.1", "2.0.0", "1.0.0", "3.0.0"]);
        let _m = server
            .mock("GET", "/index.yaml")
            .with_status(200)
            .with_body(index)
            .create_async()
            .await;

        let versions = discover_registry_versions(&server.url(), "my-chart", 10, 0)
            .await
            .unwrap();
        assert_eq!(versions, vec!["3.0.0", "2.0.0", "1.0.0"]);
    }

    #[tokio::test]
    async fn discover_registry_versions_applies_offset_and_limit() {
        let mut server = mockito::Server::new_async().await;
        let index = make_index_yaml("my-chart", &["5.0.0", "4.0.0", "3.0.0", "2.0.0", "1.0.0"]);
        let _m = server
            .mock("GET", "/index.yaml")
            .with_status(200)
            .with_body(index)
            .create_async()
            .await;

        // Skip the first (5.0.0), take next 2 (4.0.0, 3.0.0)
        let versions = discover_registry_versions(&server.url(), "my-chart", 2, 1)
            .await
            .unwrap();
        assert_eq!(versions, vec!["4.0.0", "3.0.0"]);
    }

    #[tokio::test]
    async fn discover_registry_versions_chart_not_found() {
        let mut server = mockito::Server::new_async().await;
        let index = make_index_yaml("other-chart", &["1.0.0"]);
        let _m = server
            .mock("GET", "/index.yaml")
            .with_status(200)
            .with_body(index)
            .create_async()
            .await;

        let err = discover_registry_versions(&server.url(), "my-chart", 10, 0)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    // ── discover_latest_registry (mockito) ────────────────────────────────────

    #[tokio::test]
    async fn discover_latest_registry_returns_highest_stable() {
        let mut server = mockito::Server::new_async().await;
        let index = make_index_yaml("my-chart", &["1.0.0-rc.1", "2.0.0", "1.5.0"]);
        let _m = server
            .mock("GET", "/index.yaml")
            .with_status(200)
            .with_body(index)
            .create_async()
            .await;

        let version = discover_latest_registry(&server.url(), "my-chart", None)
            .await
            .unwrap();
        assert_eq!(version, "2.0.0");
    }

    #[tokio::test]
    async fn discover_latest_registry_all_prerelease_returns_error() {
        let mut server = mockito::Server::new_async().await;
        let index = make_index_yaml("my-chart", &["1.0.0-alpha.1", "2.0.0-beta.1"]);
        let _m = server
            .mock("GET", "/index.yaml")
            .with_status(200)
            .with_body(index)
            .create_async()
            .await;

        let err = discover_latest_registry(&server.url(), "my-chart", None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no versions found"));
    }
}
