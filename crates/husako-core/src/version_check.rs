use crate::HusakoError;

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
pub fn search_artifacthub(
    query: &str,
    offset: usize,
) -> Result<ArtifactHubSearchResult, HusakoError> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("husako")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| HusakoError::GenerateIo(format!("HTTP client: {e}")))?;

    let encoded_query = percent_encode(query);
    let limit = ARTIFACTHUB_PAGE_SIZE + 1;
    let url = format!(
        "https://artifacthub.io/api/v1/packages/search?ts_query_web={encoded_query}&kind=0&limit={limit}&offset={offset}"
    );

    let resp = client
        .get(&url)
        .send()
        .map_err(|e| HusakoError::GenerateIo(format!("ArtifactHub search: {e}")))?;

    let mut packages: Vec<ArtifactHubPackage> = resp
        .json::<serde_json::Value>()
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
pub fn discover_recent_releases(limit: usize) -> Result<Vec<String>, HusakoError> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("husako")
        .build()
        .map_err(|e| HusakoError::GenerateIo(format!("HTTP client: {e}")))?;

    let resp = client
        .get("https://api.github.com/repos/kubernetes/kubernetes/tags?per_page=100")
        .send()
        .map_err(|e| HusakoError::GenerateIo(format!("GitHub API: {e}")))?;

    let tags: Vec<serde_json::Value> = resp
        .json()
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
    versions.truncate(limit);

    Ok(versions
        .iter()
        .map(|v| format!("{}.{}", v.major, v.minor))
        .collect())
}

/// Discover available versions for a chart from a Helm registry.
pub fn discover_registry_versions(
    repo: &str,
    chart: &str,
    limit: usize,
) -> Result<Vec<String>, HusakoError> {
    let url = format!("{}/index.yaml", repo.trim_end_matches('/'));
    let client = reqwest::blocking::Client::builder()
        .user_agent("husako")
        .build()
        .map_err(|e| HusakoError::GenerateIo(format!("HTTP client: {e}")))?;

    let resp = client
        .get(&url)
        .send()
        .map_err(|e| HusakoError::GenerateIo(format!("fetch registry index: {e}")))?;

    let text = resp
        .text()
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
    versions.truncate(limit);

    Ok(versions.iter().map(|v| v.to_string()).collect())
}

/// Discover the latest stable Kubernetes release version from GitHub API.
pub fn discover_latest_release() -> Result<String, HusakoError> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("husako")
        .build()
        .map_err(|e| HusakoError::GenerateIo(format!("HTTP client: {e}")))?;

    let resp = client
        .get("https://api.github.com/repos/kubernetes/kubernetes/tags?per_page=100")
        .send()
        .map_err(|e| HusakoError::GenerateIo(format!("GitHub API: {e}")))?;

    let tags: Vec<serde_json::Value> = resp
        .json()
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
pub fn discover_latest_registry(repo: &str, chart: &str) -> Result<String, HusakoError> {
    let url = format!("{}/index.yaml", repo.trim_end_matches('/'));
    let client = reqwest::blocking::Client::builder()
        .user_agent("husako")
        .build()
        .map_err(|e| HusakoError::GenerateIo(format!("HTTP client: {e}")))?;

    let resp = client
        .get(&url)
        .send()
        .map_err(|e| HusakoError::GenerateIo(format!("fetch registry index: {e}")))?;

    let text = resp
        .text()
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

    let mut best: Option<semver::Version> = None;

    for entry in entries {
        let Some(version_str) = entry.get("version").and_then(|v| v.as_str()) else {
            continue;
        };
        if let Ok(v) = semver::Version::parse(version_str)
            && v.pre.is_empty()
            && best.as_ref().is_none_or(|b| v > *b)
        {
            best = Some(v);
        }
    }

    best.map(|v| v.to_string())
        .ok_or_else(|| HusakoError::GenerateIo(format!("no versions found for chart '{chart}'")))
}

/// Discover the latest version from ArtifactHub API.
pub fn discover_latest_artifacthub(package: &str) -> Result<String, HusakoError> {
    let url = format!(
        "https://artifacthub.io/api/v1/packages/helm/{}",
        package.trim_start_matches('/')
    );
    let client = reqwest::blocking::Client::builder()
        .user_agent("husako")
        .build()
        .map_err(|e| HusakoError::GenerateIo(format!("HTTP client: {e}")))?;

    let resp = client
        .get(&url)
        .send()
        .map_err(|e| HusakoError::GenerateIo(format!("ArtifactHub API: {e}")))?;

    let data: serde_json::Value = resp
        .json()
        .map_err(|e| HusakoError::GenerateIo(format!("parse ArtifactHub response: {e}")))?;

    data["version"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| {
            HusakoError::GenerateIo(format!(
                "no version field in ArtifactHub response for '{package}'"
            ))
        })
}

/// Discover available versions for a package from ArtifactHub API.
/// Returns up to `limit` stable versions, sorted newest first.
pub fn discover_artifacthub_versions(
    package: &str,
    limit: usize,
) -> Result<Vec<String>, HusakoError> {
    let url = format!(
        "https://artifacthub.io/api/v1/packages/helm/{}",
        package.trim_start_matches('/')
    );
    let client = reqwest::blocking::Client::builder()
        .user_agent("husako")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| HusakoError::GenerateIo(format!("HTTP client: {e}")))?;

    let resp = client
        .get(&url)
        .send()
        .map_err(|e| HusakoError::GenerateIo(format!("ArtifactHub API: {e}")))?;

    let data: serde_json::Value = resp
        .json()
        .map_err(|e| HusakoError::GenerateIo(format!("parse ArtifactHub response: {e}")))?;

    let versions: Vec<String> = data["available_versions"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter(|entry| !entry["prerelease"].as_bool().unwrap_or(false))
        .filter_map(|entry| entry["version"].as_str().map(|s| s.to_string()))
        .take(limit)
        .collect();

    Ok(versions)
}

/// Discover the latest tag from a git repository using `git ls-remote --tags`.
pub fn discover_latest_git_tag(repo: &str) -> Result<Option<String>, HusakoError> {
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
pub fn discover_git_tags(repo: &str, limit: usize) -> Result<Vec<String>, HusakoError> {
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
    entries.truncate(limit);

    Ok(entries.into_iter().map(|(_, tag)| tag).collect())
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
    fn artifacthub_versions_filtering() {
        // Simulate the JSON structure returned by ArtifactHub API
        let data = serde_json::json!({
            "version": "3.0.0",
            "available_versions": [
                {"version": "3.0.0", "prerelease": false, "ts": 1700000000},
                {"version": "3.0.0-rc.1", "prerelease": true, "ts": 1699900000},
                {"version": "2.5.0", "prerelease": false, "ts": 1699000000},
                {"version": "2.5.0-beta.1", "prerelease": true, "ts": 1698000000},
                {"version": "2.4.0", "prerelease": false, "ts": 1697000000},
                {"version": "2.3.0", "prerelease": false, "ts": 1696000000},
            ]
        });

        let versions: Vec<String> = data["available_versions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| !entry["prerelease"].as_bool().unwrap_or(false))
            .filter_map(|entry| entry["version"].as_str().map(|s| s.to_string()))
            .take(3)
            .collect();

        assert_eq!(versions, vec!["3.0.0", "2.5.0", "2.4.0"]);
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
    fn versions_match_v_prefix() {
        assert!(versions_match("1.35", "1.35"));
    }
}
