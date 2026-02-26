use std::path::Path;

/// Result of URL/package auto-detection for `husako add`.
#[derive(Debug, PartialEq)]
pub enum UrlDetected {
    ArtifactHub {
        package: String,
    },
    Oci {
        reference: String,
    },
    Git {
        repo: String,
        sub_path: Option<String>,
        branch: Option<String>,
    },
    HelmRegistry {
        repo: String,
    },
    LocalPath {
        path: String,
    },
}

/// Auto-detect the source type from a URL or package identifier.
///
/// Detection order:
/// 1. `oci://` → OCI
/// 2. `org/chart` (exactly one `/`, no protocol) → ArtifactHub
/// 3. `https?://github.com/`, `gitlab.com/`, `bitbucket.org/` → Git
/// 4. Other `https?://` → HelmRegistry
/// 5. `./`, `../`, `/` prefix → LocalPath
/// 6. Otherwise → None
pub fn detect_url(input: &str) -> Option<UrlDetected> {
    // 1. OCI
    if input.starts_with("oci://") {
        return Some(UrlDetected::Oci {
            reference: input.to_string(),
        });
    }

    // 2. ArtifactHub: exactly one '/', no protocol, safe chars only
    if is_artifacthub_package(input) {
        return Some(UrlDetected::ArtifactHub {
            package: input.to_string(),
        });
    }

    // 3. Git hosts
    if let Some(git) = try_git_url(input) {
        return Some(git);
    }

    // 4. Other https/http → HelmRegistry
    if input.starts_with("https://") || input.starts_with("http://") {
        return Some(UrlDetected::HelmRegistry {
            repo: input.to_string(),
        });
    }

    // 5. Local path
    if input.starts_with("./") || input.starts_with("../") || input.starts_with('/') {
        return Some(UrlDetected::LocalPath {
            path: input.to_string(),
        });
    }

    None
}

fn is_artifacthub_package(input: &str) -> bool {
    if input.contains("://") {
        return false;
    }
    let slash_count = input.chars().filter(|&c| c == '/').count();
    if slash_count != 1 {
        return false;
    }
    let (left, right) = input.split_once('/').unwrap();
    let valid_part = |s: &str| {
        !s.is_empty()
            && s.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' || c == '.'
            })
    };
    valid_part(left) && valid_part(right)
}

const GIT_HOSTS: &[&str] = &["github.com", "gitlab.com", "bitbucket.org"];

fn try_git_url(input: &str) -> Option<UrlDetected> {
    let rest = input
        .strip_prefix("https://")
        .or_else(|| input.strip_prefix("http://"))?;

    let host_end = rest.find('/').unwrap_or(rest.len());
    let host = &rest[..host_end];
    if !GIT_HOSTS.contains(&host) {
        return None;
    }

    let path_part = &rest[host_end..]; // starts with '/'
    let segments: Vec<&str> = path_part.trim_start_matches('/').split('/').collect();

    if segments.len() < 2 || segments[0].is_empty() || segments[1].is_empty() {
        return None;
    }

    let org = segments[0];
    let repo_name = segments[1].trim_end_matches(".git");
    let repo_base = format!("https://{host}/{org}/{repo_name}");

    if segments.len() == 2 {
        return Some(UrlDetected::Git {
            repo: repo_base,
            sub_path: None,
            branch: None,
        });
    }

    let rest_segments = &segments[2..];

    // /tree/<branch>/... pattern
    if rest_segments.len() >= 2 && rest_segments[0] == "tree" {
        let branch = rest_segments[1].to_string();
        let sub_path = if rest_segments.len() > 2 {
            Some(rest_segments[2..].join("/"))
        } else {
            None
        };
        return Some(UrlDetected::Git {
            repo: repo_base,
            sub_path,
            branch: Some(branch),
        });
    }

    // Embedded path (no tree/)
    let sub_path = rest_segments.join("/");
    Some(UrlDetected::Git {
        repo: repo_base,
        sub_path: Some(sub_path),
        branch: None,
    })
}

/// Source kind detected from content inspection.
#[derive(Debug, PartialEq)]
pub enum SourceKind {
    Resource,
    Chart,
}

/// Detect source kind from a cloned git directory by reading file content.
pub fn detect_git_kind(dir: &Path, sub_path: &str) -> Result<SourceKind, String> {
    let target = if sub_path == "." || sub_path.is_empty() {
        dir.to_path_buf()
    } else {
        dir.join(sub_path)
    };

    // Chart.yaml → Chart
    if target.join("Chart.yaml").exists() || target.join("chart.yaml").exists() {
        return Ok(SourceKind::Chart);
    }

    // JSON file with "$schema" key → Helm values schema (Chart)
    if target.is_file() && target.extension().and_then(|e| e.to_str()) == Some("json") {
        let content = std::fs::read_to_string(&target)
            .map_err(|e| format!("could not read {}: {e}", target.display()))?;
        if content.contains("\"$schema\"") {
            return Ok(SourceKind::Chart);
        }
    }

    // CRD YAML → Resource
    if has_crd_content(&target)? {
        return Ok(SourceKind::Resource);
    }

    Err(format!(
        "could not detect resource or chart at '{}'; check the path or use --path",
        sub_path
    ))
}

fn has_crd_content(target: &Path) -> Result<bool, String> {
    if target.is_file() {
        let content = std::fs::read_to_string(target)
            .map_err(|e| format!("could not read {}: {e}", target.display()))?;
        return Ok(content_is_crd(&content));
    }

    if target.is_dir() {
        for entry in std::fs::read_dir(target)
            .map_err(|e| format!("could not read directory {}: {e}", target.display()))?
        {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if (ext == "yaml" || ext == "yml") && path.is_file() {
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                if content_is_crd(&content) {
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}

fn content_is_crd(content: &str) -> bool {
    content.contains("kind: CustomResourceDefinition")
}

/// Detect source kind from a local file or directory by reading content.
pub fn detect_local_kind(path: &str) -> Result<SourceKind, String> {
    let p = Path::new(path);

    if p.is_file() {
        let content =
            std::fs::read_to_string(p).map_err(|e| format!("could not read {path}: {e}"))?;

        if content_is_crd(&content) {
            return Ok(SourceKind::Resource);
        }

        // JSON Schema heuristic: "$schema" key → chart values schema
        if content.contains("\"$schema\"") {
            return Ok(SourceKind::Chart);
        }
    } else if p.is_dir() {
        if p.join("Chart.yaml").exists() || p.join("chart.yaml").exists() {
            return Ok(SourceKind::Chart);
        }
        if has_crd_content(p)? {
            return Ok(SourceKind::Resource);
        }
    }

    Err(format!(
        "could not determine if '{path}' is a CRD or a Helm values schema"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- detect_url ---

    #[test]
    fn detect_artifacthub() {
        assert_eq!(
            detect_url("bitnami/postgresql"),
            Some(UrlDetected::ArtifactHub {
                package: "bitnami/postgresql".to_string()
            })
        );
    }

    #[test]
    fn detect_oci() {
        assert_eq!(
            detect_url("oci://ghcr.io/bitnami/postgresql"),
            Some(UrlDetected::Oci {
                reference: "oci://ghcr.io/bitnami/postgresql".to_string()
            })
        );
    }

    #[test]
    fn detect_git_simple() {
        assert_eq!(
            detect_url("https://github.com/cert-manager/cert-manager"),
            Some(UrlDetected::Git {
                repo: "https://github.com/cert-manager/cert-manager".to_string(),
                sub_path: None,
                branch: None,
            })
        );
    }

    #[test]
    fn detect_git_with_path() {
        assert_eq!(
            detect_url("https://github.com/cert-manager/cert-manager/deploy/crds"),
            Some(UrlDetected::Git {
                repo: "https://github.com/cert-manager/cert-manager".to_string(),
                sub_path: Some("deploy/crds".to_string()),
                branch: None,
            })
        );
    }

    #[test]
    fn detect_git_tree_branch_path() {
        assert_eq!(
            detect_url("https://github.com/cert-manager/cert-manager/tree/master/deploy/crds"),
            Some(UrlDetected::Git {
                repo: "https://github.com/cert-manager/cert-manager".to_string(),
                sub_path: Some("deploy/crds".to_string()),
                branch: Some("master".to_string()),
            })
        );
    }

    #[test]
    fn detect_git_tree_branch_no_path() {
        assert_eq!(
            detect_url("https://github.com/cert-manager/cert-manager/tree/main"),
            Some(UrlDetected::Git {
                repo: "https://github.com/cert-manager/cert-manager".to_string(),
                sub_path: None,
                branch: Some("main".to_string()),
            })
        );
    }

    #[test]
    fn detect_helm_registry() {
        assert_eq!(
            detect_url("https://charts.jetstack.io"),
            Some(UrlDetected::HelmRegistry {
                repo: "https://charts.jetstack.io".to_string()
            })
        );
    }

    #[test]
    fn detect_local_relative() {
        assert_eq!(
            detect_url("./crds/my-crd.yaml"),
            Some(UrlDetected::LocalPath {
                path: "./crds/my-crd.yaml".to_string()
            })
        );
    }

    #[test]
    fn detect_local_absolute() {
        assert_eq!(
            detect_url("/abs/path/crd.yaml"),
            Some(UrlDetected::LocalPath {
                path: "/abs/path/crd.yaml".to_string()
            })
        );
    }

    #[test]
    fn detect_none_for_bare_word() {
        assert_eq!(detect_url("foo"), None);
    }

    #[test]
    fn detect_none_for_version_string() {
        assert_eq!(detect_url("1.35"), None);
        assert_eq!(detect_url("v1.35"), None);
    }

    #[test]
    fn detect_none_for_single_segment() {
        assert_eq!(detect_url("postgresql"), None);
    }

    #[test]
    fn artifacthub_rejects_uppercase() {
        assert_eq!(detect_url("Bitnami/postgresql"), None);
    }

    #[test]
    fn artifacthub_rejects_multiple_slashes() {
        assert_eq!(detect_url("bitnami/postgresql/extra"), None);
    }

    #[test]
    fn detect_gitlab() {
        assert_eq!(
            detect_url("https://gitlab.com/org/repo"),
            Some(UrlDetected::Git {
                repo: "https://gitlab.com/org/repo".to_string(),
                sub_path: None,
                branch: None,
            })
        );
    }

    // --- detect_git_kind ---

    #[test]
    fn detect_git_kind_chart_yaml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Chart.yaml"), "name: my-chart\n").unwrap();
        assert_eq!(detect_git_kind(dir.path(), ".").unwrap(), SourceKind::Chart);
    }

    #[test]
    fn detect_git_kind_crd_yaml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("crd.yaml"),
            "kind: CustomResourceDefinition\napiVersion: apiextensions.k8s.io/v1\n",
        )
        .unwrap();
        assert_eq!(
            detect_git_kind(dir.path(), ".").unwrap(),
            SourceKind::Resource
        );
    }

    #[test]
    fn detect_git_kind_empty_dir_returns_err() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_git_kind(dir.path(), ".").is_err());
    }

    #[test]
    fn detect_git_kind_json_schema_file() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("charts/prometheus");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            sub.join("values.schema.json"),
            r#"{"$schema":"http://json-schema.org/draft-07/schema#","properties":{}}"#,
        )
        .unwrap();
        assert_eq!(
            detect_git_kind(dir.path(), "charts/prometheus/values.schema.json").unwrap(),
            SourceKind::Chart
        );
    }

    #[test]
    fn detect_git_kind_with_sub_path() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("deploy/crds");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("crd.yaml"), "kind: CustomResourceDefinition\n").unwrap();
        assert_eq!(
            detect_git_kind(dir.path(), "deploy/crds").unwrap(),
            SourceKind::Resource
        );
    }

    // --- detect_local_kind ---

    #[test]
    fn detect_local_crd_file() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("crd.yaml");
        std::fs::write(&f, "kind: CustomResourceDefinition\n").unwrap();
        assert_eq!(
            detect_local_kind(f.to_str().unwrap()).unwrap(),
            SourceKind::Resource
        );
    }

    #[test]
    fn detect_local_json_schema_file() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("values.schema.json");
        std::fs::write(
            &f,
            r#"{"$schema":"http://json-schema.org/draft-07/schema#","properties":{}}"#,
        )
        .unwrap();
        assert_eq!(
            detect_local_kind(f.to_str().unwrap()).unwrap(),
            SourceKind::Chart
        );
    }

    #[test]
    fn detect_local_unknown_file_returns_err() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("random.yaml");
        std::fs::write(&f, "some: yaml\nwithout: crd\n").unwrap();
        assert!(detect_local_kind(f.to_str().unwrap()).is_err());
    }

    #[test]
    fn detect_local_chart_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Chart.yaml"), "name: my-chart\n").unwrap();
        assert_eq!(
            detect_local_kind(dir.path().to_str().unwrap()).unwrap(),
            SourceKind::Chart
        );
    }
}
