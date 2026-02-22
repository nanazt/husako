use crate::OpenApiError;

/// Credentials resolved from a kubeconfig file.
#[derive(Debug, Clone)]
pub struct ClusterCredentials {
    pub server: String,
    pub bearer_token: String,
}

/// Resolve credentials for a Kubernetes API server from kubeconfig files.
///
/// Searches `~/.kube/` by default. Returns the bearer token for the first
/// cluster matching `server_url`.
pub fn resolve_credentials(server_url: &str) -> Result<ClusterCredentials, OpenApiError> {
    let kube_dir = dirs_kube();
    resolve_credentials_from_dir(&kube_dir, server_url)
}

/// Resolve credentials from a specific directory of kubeconfig files.
pub fn resolve_credentials_from_dir(
    kube_dir: &std::path::Path,
    server_url: &str,
) -> Result<ClusterCredentials, OpenApiError> {
    let entries = std::fs::read_dir(kube_dir).map_err(|e| {
        OpenApiError::Kubeconfig(format!("cannot read {}: {e}", kube_dir.display()))
    })?;

    let normalized_target = normalize_url(server_url);

    for entry in entries {
        let entry = entry.map_err(|e| OpenApiError::Kubeconfig(format!("read entry: {e}")))?;
        let path = entry.path();

        // Only process regular files (no subdirectory traversal)
        if !path.is_file() {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue, // silently skip unreadable files
        };

        let config: KubeConfig = match serde_yaml_ng::from_str(&content) {
            Ok(c) => c,
            Err(_) => continue, // silently skip non-kubeconfig files
        };

        if let Some(creds) = find_credentials(&config, &normalized_target) {
            return Ok(creds);
        }
    }

    Err(OpenApiError::Kubeconfig(format!(
        "no kubeconfig found for server '{server_url}' in {}",
        kube_dir.display()
    )))
}

fn find_credentials(config: &KubeConfig, normalized_target: &str) -> Option<ClusterCredentials> {
    // Find the cluster entry matching the target server URL
    let cluster_entry = config.clusters.iter().find(|c| {
        let normalized = normalize_url(&c.cluster.server);
        normalized == normalized_target
    })?;

    let cluster_name = &cluster_entry.name;

    // Find a context referencing this cluster
    let context_entry = config
        .contexts
        .iter()
        .find(|ctx| &ctx.context.cluster == cluster_name)?;

    let user_name = &context_entry.context.user;

    // Find the user entry
    let user_entry = config.users.iter().find(|u| &u.name == user_name)?;

    // Extract bearer token
    let token = user_entry.user.token.as_ref()?;

    Some(ClusterCredentials {
        server: cluster_entry.cluster.server.clone(),
        bearer_token: token.clone(),
    })
}

/// Normalize a server URL by stripping trailing slashes.
fn normalize_url(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

fn dirs_kube() -> std::path::PathBuf {
    dirs_home().join(".kube")
}

fn dirs_home() -> std::path::PathBuf {
    std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/root"))
}

// Minimal kubeconfig structs — just enough for credential extraction.

#[derive(Debug, serde::Deserialize)]
struct KubeConfig {
    #[serde(default)]
    clusters: Vec<NamedCluster>,
    #[serde(default)]
    contexts: Vec<NamedContext>,
    #[serde(default)]
    users: Vec<NamedUser>,
}

#[derive(Debug, serde::Deserialize)]
struct NamedCluster {
    name: String,
    cluster: ClusterInfo,
}

#[derive(Debug, serde::Deserialize)]
struct ClusterInfo {
    server: String,
}

#[derive(Debug, serde::Deserialize)]
struct NamedContext {
    #[allow(dead_code)]
    name: String,
    context: ContextInfo,
}

#[derive(Debug, serde::Deserialize)]
struct ContextInfo {
    cluster: String,
    user: String,
}

#[derive(Debug, serde::Deserialize)]
struct NamedUser {
    name: String,
    user: UserInfo,
}

#[derive(Debug, serde::Deserialize)]
struct UserInfo {
    #[serde(default)]
    token: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_kubeconfig(dir: &std::path::Path, filename: &str, content: &str) {
        std::fs::write(dir.join(filename), content).unwrap();
    }

    const STANDARD_KUBECONFIG: &str = r#"
apiVersion: v1
kind: Config
clusters:
  - name: my-cluster
    cluster:
      server: https://10.0.0.1:6443
contexts:
  - name: my-context
    context:
      cluster: my-cluster
      user: my-user
users:
  - name: my-user
    user:
      token: my-bearer-token-123
"#;

    #[test]
    fn resolve_standard_bearer_token() {
        let tmp = tempfile::tempdir().unwrap();
        write_kubeconfig(tmp.path(), "config", STANDARD_KUBECONFIG);

        let creds = resolve_credentials_from_dir(tmp.path(), "https://10.0.0.1:6443").unwrap();
        assert_eq!(creds.bearer_token, "my-bearer-token-123");
        assert_eq!(creds.server, "https://10.0.0.1:6443");
    }

    #[test]
    fn url_normalization_trailing_slash() {
        let tmp = tempfile::tempdir().unwrap();
        write_kubeconfig(tmp.path(), "config", STANDARD_KUBECONFIG);

        // Query with trailing slash should still match
        let creds = resolve_credentials_from_dir(tmp.path(), "https://10.0.0.1:6443/").unwrap();
        assert_eq!(creds.bearer_token, "my-bearer-token-123");
    }

    #[test]
    fn no_match_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        write_kubeconfig(tmp.path(), "config", STANDARD_KUBECONFIG);

        let err =
            resolve_credentials_from_dir(tmp.path(), "https://other-server:6443").unwrap_err();
        assert!(err.to_string().contains("no kubeconfig found"));
    }

    #[test]
    fn skip_non_yaml_files() {
        let tmp = tempfile::tempdir().unwrap();
        write_kubeconfig(tmp.path(), "config", STANDARD_KUBECONFIG);
        std::fs::write(tmp.path().join("binary.dat"), [0xFF, 0xFE, 0x00]).unwrap();
        std::fs::write(tmp.path().join("readme.txt"), "not yaml").unwrap();

        let creds = resolve_credentials_from_dir(tmp.path(), "https://10.0.0.1:6443").unwrap();
        assert_eq!(creds.bearer_token, "my-bearer-token-123");
    }

    #[test]
    fn multiple_configs_first_match_wins() {
        let tmp = tempfile::tempdir().unwrap();

        write_kubeconfig(
            tmp.path(),
            "config-a",
            r#"
apiVersion: v1
kind: Config
clusters:
  - name: cluster-a
    cluster:
      server: https://a:6443
contexts:
  - name: ctx-a
    context:
      cluster: cluster-a
      user: user-a
users:
  - name: user-a
    user:
      token: token-a
"#,
        );

        write_kubeconfig(
            tmp.path(),
            "config-b",
            r#"
apiVersion: v1
kind: Config
clusters:
  - name: cluster-b
    cluster:
      server: https://b:6443
contexts:
  - name: ctx-b
    context:
      cluster: cluster-b
      user: user-b
users:
  - name: user-b
    user:
      token: token-b
"#,
        );

        let creds = resolve_credentials_from_dir(tmp.path(), "https://b:6443").unwrap();
        assert_eq!(creds.bearer_token, "token-b");
    }

    #[test]
    fn no_token_user_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        write_kubeconfig(
            tmp.path(),
            "config",
            r#"
apiVersion: v1
kind: Config
clusters:
  - name: cl
    cluster:
      server: https://10.0.0.1:6443
contexts:
  - name: ctx
    context:
      cluster: cl
      user: usr
users:
  - name: usr
    user: {}
"#,
        );

        let err = resolve_credentials_from_dir(tmp.path(), "https://10.0.0.1:6443").unwrap_err();
        assert!(err.to_string().contains("no kubeconfig found"));
    }
}
