use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

pub const CONFIG_FILENAME: &str = "husako.toml";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse husako.toml: {0}")]
    Parse(String),
    #[error("config validation error: {0}")]
    Validation(String),
}

/// Full `husako.toml` configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct HusakoConfig {
    /// Entry file aliases: `dev = "env/dev.ts"`.
    #[serde(default)]
    pub entries: HashMap<String, String>,

    /// Single cluster connection (shorthand for the common case).
    #[serde(default)]
    pub cluster: Option<ClusterConfig>,

    /// Named cluster connections for multi-cluster setups.
    #[serde(default)]
    pub clusters: HashMap<String, ClusterConfig>,

    /// Schema dependencies.
    #[serde(default)]
    pub schemas: HashMap<String, SchemaSource>,
}

/// Cluster connection configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ClusterConfig {
    pub server: String,
}

/// A schema dependency entry. Every entry must specify `source` explicitly.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "source")]
pub enum SchemaSource {
    /// Fetch OpenAPI v3 specs from kubernetes/kubernetes GitHub releases.
    /// `kubernetes = { source = "release", version = "1.35" }`
    #[serde(rename = "release")]
    Release { version: String },

    /// Fetch all specs from a live K8s API server.
    /// `cluster-crds = { source = "cluster" }` — uses `[cluster]`
    /// `dev-crds = { source = "cluster", cluster = "dev" }` — uses `[clusters.dev]`
    #[serde(rename = "cluster")]
    Cluster {
        #[serde(default)]
        cluster: Option<String>,
    },

    /// Clone a git repo at a tag and extract CRD YAML manifests.
    /// `cert-manager = { source = "git", repo = "...", tag = "v1.17.2", path = "deploy/crds" }`
    #[serde(rename = "git")]
    Git {
        repo: String,
        tag: String,
        path: String,
    },

    /// Read CRD YAML from a local file or directory.
    /// `my-crd = { source = "file", path = "./crds/my-crd.yaml" }`
    #[serde(rename = "file")]
    File { path: String },
}

/// Load `husako.toml` from the given directory.
///
/// Returns `Ok(None)` if the file does not exist.
/// Returns `Err` if the file exists but cannot be read or parsed.
pub fn load(project_root: &Path) -> Result<Option<HusakoConfig>, ConfigError> {
    let path = project_root.join(CONFIG_FILENAME);
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path).map_err(|e| ConfigError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let config: HusakoConfig =
        toml::from_str(&content).map_err(|e| ConfigError::Parse(e.to_string()))?;
    validate(&config)?;
    Ok(Some(config))
}

fn validate(config: &HusakoConfig) -> Result<(), ConfigError> {
    // Entry paths must be relative
    for (alias, path) in &config.entries {
        if Path::new(path).is_absolute() {
            return Err(ConfigError::Validation(format!(
                "entry '{alias}' has absolute path '{path}'; use a relative path"
            )));
        }
    }

    // Cannot have both [cluster] and [clusters]
    if config.cluster.is_some() && !config.clusters.is_empty() {
        return Err(ConfigError::Validation(
            "cannot use both [cluster] and [clusters]; use one or the other".to_string(),
        ));
    }

    // Schema cluster references must resolve
    for (name, source) in &config.schemas {
        if let SchemaSource::Cluster {
            cluster: Some(cluster_name),
        } = source
            && !config.clusters.contains_key(cluster_name)
        {
            return Err(ConfigError::Validation(format!(
                "schema '{name}' references unknown cluster '{cluster_name}'; \
                 define it in [clusters.{cluster_name}]"
            )));
        }

        if let SchemaSource::Cluster { cluster: None } = source {
            if config.cluster.is_none() && config.clusters.is_empty() {
                return Err(ConfigError::Validation(format!(
                    "schema '{name}' uses source = \"cluster\" but no [cluster] section is defined"
                )));
            }
            if config.cluster.is_none() && !config.clusters.is_empty() {
                return Err(ConfigError::Validation(format!(
                    "schema '{name}' uses source = \"cluster\" without a cluster name; \
                     specify which cluster to use, e.g. cluster = \"dev\""
                )));
            }
        }

        // File paths must be relative
        if let SchemaSource::File { path } = source
            && Path::new(path).is_absolute()
        {
            return Err(ConfigError::Validation(format!(
                "schema '{name}' has absolute path '{path}'; use a relative path"
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_config() {
        let toml = r#"
[entries]
dev = "env/dev.ts"
staging = "env/staging.ts"

[cluster]
server = "https://10.0.0.1:6443"

[schemas]
kubernetes = { source = "release", version = "1.35" }
cluster-crds = { source = "cluster" }
cert-manager = { source = "git", repo = "https://github.com/cert-manager/cert-manager", tag = "v1.17.2", path = "deploy/crds" }
my-crd = { source = "file", path = "./crds/my-crd.yaml" }
"#;
        let config: HusakoConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.entries.len(), 2);
        assert_eq!(config.entries["dev"], "env/dev.ts");
        assert_eq!(config.schemas.len(), 4);
        assert!(config.cluster.is_some());
        assert_eq!(config.cluster.unwrap().server, "https://10.0.0.1:6443");

        assert!(matches!(
            config.schemas["kubernetes"],
            SchemaSource::Release { ref version } if version == "1.35"
        ));
        assert!(matches!(
            config.schemas["cluster-crds"],
            SchemaSource::Cluster { cluster: None }
        ));
        assert!(matches!(
            config.schemas["cert-manager"],
            SchemaSource::Git { .. }
        ));
        assert!(matches!(
            config.schemas["my-crd"],
            SchemaSource::File { .. }
        ));
    }

    #[test]
    fn parse_multi_cluster_config() {
        let toml = r#"
[clusters.dev]
server = "https://dev:6443"

[clusters.prod]
server = "https://prod:6443"

[schemas]
dev-crds = { source = "cluster", cluster = "dev" }
prod-crds = { source = "cluster", cluster = "prod" }
"#;
        let config: HusakoConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.clusters.len(), 2);
        assert!(config.cluster.is_none());
        validate(&config).unwrap();
    }

    #[test]
    fn parse_empty_config() {
        let config: HusakoConfig = toml::from_str("").unwrap();
        assert!(config.entries.is_empty());
        assert!(config.schemas.is_empty());
        assert!(config.cluster.is_none());
        assert!(config.clusters.is_empty());
    }

    #[test]
    fn parse_entries_only() {
        let toml = r#"
[entries]
dev = "env/dev.ts"
"#;
        let config: HusakoConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.entries.len(), 1);
        assert!(config.schemas.is_empty());
    }

    #[test]
    fn invalid_toml_returns_error() {
        let result: Result<HusakoConfig, _> = toml::from_str("invalid [[ toml");
        assert!(result.is_err());
    }

    #[test]
    fn reject_absolute_entry_path() {
        let config = HusakoConfig {
            entries: HashMap::from([("dev".to_string(), "/absolute/path.ts".to_string())]),
            ..Default::default()
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("absolute path"));
    }

    #[test]
    fn reject_absolute_file_source_path() {
        let config = HusakoConfig {
            schemas: HashMap::from([(
                "my-crd".to_string(),
                SchemaSource::File {
                    path: "/absolute/crd.yaml".to_string(),
                },
            )]),
            ..Default::default()
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("absolute path"));
    }

    #[test]
    fn reject_both_cluster_and_clusters() {
        let config = HusakoConfig {
            cluster: Some(ClusterConfig {
                server: "https://a:6443".to_string(),
            }),
            clusters: HashMap::from([(
                "dev".to_string(),
                ClusterConfig {
                    server: "https://b:6443".to_string(),
                },
            )]),
            ..Default::default()
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("cannot use both"));
    }

    #[test]
    fn reject_unknown_cluster_reference() {
        let config = HusakoConfig {
            clusters: HashMap::from([(
                "dev".to_string(),
                ClusterConfig {
                    server: "https://dev:6443".to_string(),
                },
            )]),
            schemas: HashMap::from([(
                "crds".to_string(),
                SchemaSource::Cluster {
                    cluster: Some("staging".to_string()),
                },
            )]),
            ..Default::default()
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("unknown cluster 'staging'"));
    }

    #[test]
    fn reject_cluster_source_without_cluster_section() {
        let config = HusakoConfig {
            schemas: HashMap::from([("crds".to_string(), SchemaSource::Cluster { cluster: None })]),
            ..Default::default()
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("no [cluster] section"));
    }

    #[test]
    fn reject_unnamed_cluster_with_named_clusters() {
        let config = HusakoConfig {
            clusters: HashMap::from([(
                "dev".to_string(),
                ClusterConfig {
                    server: "https://dev:6443".to_string(),
                },
            )]),
            schemas: HashMap::from([("crds".to_string(), SchemaSource::Cluster { cluster: None })]),
            ..Default::default()
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("specify which cluster"));
    }

    #[test]
    fn load_missing_file_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        let result = load(tmp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn load_valid_file() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("husako.toml"),
            r#"
[entries]
dev = "env/dev.ts"

[schemas]
kubernetes = { source = "release", version = "1.35" }
"#,
        )
        .unwrap();
        let config = load(tmp.path()).unwrap().unwrap();
        assert_eq!(config.entries["dev"], "env/dev.ts");
    }

    #[test]
    fn load_invalid_file_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("husako.toml"), "invalid [[ toml").unwrap();
        let err = load(tmp.path()).unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn parse_unknown_source_returns_error() {
        let toml = r#"
[schemas]
foo = { source = "unknown", bar = "baz" }
"#;
        let result: Result<HusakoConfig, _> = toml::from_str(toml);
        assert!(result.is_err());
    }
}
