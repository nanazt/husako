use crate::HusakoError;

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
