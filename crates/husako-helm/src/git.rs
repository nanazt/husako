use std::path::Path;

use crate::HelmError;

/// Resolve a Helm chart from a git repository.
///
/// Flow:
/// 1. Check cache
/// 2. Shallow-clone the repo at the specified tag
/// 3. Read `values.schema.json` from the specified path within the repo
/// 4. Cache and return
pub async fn resolve(
    name: &str,
    repo: &str,
    tag: &str,
    path: &str,
    cache_dir: &Path,
    on_progress: Option<&crate::ProgressCb>,
) -> Result<serde_json::Value, HelmError> {
    // Check cache
    let cache_key = crate::cache_hash(&format!("{repo}/{path}"));
    let cache_path = cache_dir.join(format!("helm/git/{cache_key}/{tag}.json"));
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

    // Clone repo at specific tag
    let temp_dir = tempfile::tempdir()
        .map_err(|e| HelmError::Io(format!("chart '{name}': create temp dir: {e}")))?;

    let mut child = tokio::process::Command::new("git")
        .args(["clone", "--depth", "1", "--branch", tag, "--progress", repo])
        .arg(temp_dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| HelmError::Io(format!("chart '{name}': git clone failed: {e}")))?;

    // Stream stderr for real-time progress; collect for error reporting.
    // git --progress uses \r to update in-place, so we split on both \r and \n.
    let mut stderr_text = String::new();
    if let Some(stderr) = child.stderr.take() {
        use tokio::io::AsyncBufReadExt;
        let mut lines = tokio::io::BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            for part in line.split('\r') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                stderr_text.push_str(part);
                stderr_text.push('\n');
                if let (Some(cb), Some((pct, bytes))) = (on_progress, parse_git_progress(part)) {
                    cb(bytes, None, Some(pct));
                }
            }
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| HelmError::Io(format!("chart '{name}': git clone wait: {e}")))?;

    if !status.success() {
        return Err(HelmError::Io(format!(
            "chart '{name}': git clone {repo} at tag {tag} failed: {stderr_text}"
        )));
    }

    // Read values.schema.json from the specified path
    let schema_path = temp_dir.path().join(path);
    if !schema_path.exists() {
        return Err(HelmError::NotFound(format!(
            "chart '{name}': path '{path}' not found in repository {repo} at tag {tag}"
        )));
    }

    let content = std::fs::read_to_string(&schema_path).map_err(|e| {
        HelmError::Io(format!(
            "chart '{name}': read {}: {e}",
            schema_path.display()
        ))
    })?;

    let schema: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        HelmError::InvalidSchema(format!("chart '{name}': parse values.schema.json: {e}"))
    })?;

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

/// Parse a git progress line like "Receiving objects:  45% (24430/54286), 53.29 MiB | ..."
/// Returns `(pct, bytes_received)`.
fn parse_git_progress(line: &str) -> Option<(u8, u64)> {
    let rest = line.strip_prefix("Receiving objects:")?.trim();
    let pct_end = rest.find('%')?;
    let pct: u8 = rest[..pct_end].trim().parse().ok()?;

    // Try to extract MiB value: ", 53.29 MiB | ..."
    let bytes = if let Some(mib_pos) = rest.find(" MiB") {
        let before = rest[..mib_pos].trim_end();
        let num_start = before
            .rfind(|c: char| !c.is_ascii_digit() && c != '.')
            .map(|i| i + 1)
            .unwrap_or(0);
        before[num_start..]
            .parse::<f64>()
            .ok()
            .map(|mib| (mib * 1_048_576.0) as u64)
            .unwrap_or(0)
    } else {
        0
    };

    Some((pct, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cache_hit_returns_cached_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path();
        let cache_key = crate::cache_hash(
            "https://github.com/example/chart/charts/my-chart/values.schema.json",
        );
        let cache_sub = cache_dir.join(format!("helm/git/{cache_key}"));
        std::fs::create_dir_all(&cache_sub).unwrap();
        std::fs::write(
            cache_sub.join("v1.0.0.json"),
            r#"{"type":"object","properties":{"replicas":{"type":"integer"}}}"#,
        )
        .unwrap();

        let result = resolve(
            "test",
            "https://github.com/example/chart",
            "v1.0.0",
            "charts/my-chart/values.schema.json",
            cache_dir,
            None,
        )
        .await
        .unwrap();
        assert_eq!(result["type"], "object");
        assert!(result["properties"]["replicas"].is_object());
    }

    #[tokio::test]
    async fn cache_invalid_json_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path();
        let cache_key = crate::cache_hash(
            "https://github.com/example/chart/charts/my-chart/values.schema.json",
        );
        let cache_sub = cache_dir.join(format!("helm/git/{cache_key}"));
        std::fs::create_dir_all(&cache_sub).unwrap();
        std::fs::write(cache_sub.join("v1.0.0.json"), "not json").unwrap();

        let err = resolve(
            "test",
            "https://github.com/example/chart",
            "v1.0.0",
            "charts/my-chart/values.schema.json",
            cache_dir,
            None,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("parse cached schema"));
    }

    #[test]
    fn parse_git_progress_with_bytes() {
        let (pct, bytes) =
            parse_git_progress("Receiving objects:  45% (24430/54286), 53.29 MiB | 10.00 MiB/s")
                .unwrap();
        assert_eq!(pct, 45);
        assert_eq!(bytes, (53.29 * 1_048_576.0) as u64);
    }

    #[test]
    fn parse_git_progress_no_bytes() {
        let (pct, bytes) = parse_git_progress("Receiving objects: 100% (54286/54286)").unwrap();
        assert_eq!(pct, 100);
        assert_eq!(bytes, 0);
    }

    #[test]
    fn parse_git_progress_non_matching_line() {
        assert!(parse_git_progress("remote: Counting objects: 100% (54286/54286)").is_none());
        assert!(parse_git_progress("Resolving deltas:  25% (100/400)").is_none());
        assert!(parse_git_progress("").is_none());
    }
}
