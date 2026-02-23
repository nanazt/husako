pub mod edit;

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

    /// Resource schema dependencies (renamed from `schemas`).
    #[serde(default, alias = "schemas")]
    pub resources: HashMap<String, SchemaSource>,

    /// Chart values schema sources.
    #[serde(default)]
    pub charts: HashMap<String, ChartSource>,

    /// Plugin dependencies.
    #[serde(default)]
    pub plugins: HashMap<String, PluginSource>,
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

/// A chart values schema source. Specifies where to find `values.schema.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "source")]
pub enum ChartSource {
    /// Fetch from an HTTP Helm chart repository.
    /// `ingress-nginx = { source = "registry", repo = "https://...", chart = "ingress-nginx", version = "4.12.0" }`
    #[serde(rename = "registry")]
    Registry {
        repo: String,
        chart: String,
        version: String,
    },

    /// Fetch from ArtifactHub API.
    /// `postgresql = { source = "artifacthub", package = "bitnami/postgresql", version = "16.4.0" }`
    #[serde(rename = "artifacthub")]
    ArtifactHub { package: String, version: String },

    /// Read a local `values.schema.json` file.
    /// `my-chart = { source = "file", path = "./schemas/my-chart-values.schema.json" }`
    #[serde(rename = "file")]
    File { path: String },

    /// Clone a git repo at a tag and extract `values.schema.json`.
    /// `my-chart = { source = "git", repo = "https://...", tag = "v1.0.0", path = "charts/my-chart" }`
    #[serde(rename = "git")]
    Git {
        repo: String,
        tag: String,
        path: String,
    },
}

/// A plugin dependency entry in `husako.toml`.
/// `flux = { source = "git", url = "https://github.com/nanazt/husako-plugin-flux" }`
/// `my-plugin = { source = "path", path = "./plugins/my-plugin" }`
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "source")]
pub enum PluginSource {
    /// Clone from a git repository.
    #[serde(rename = "git")]
    Git { url: String },

    /// Use a local directory.
    #[serde(rename = "path")]
    Path { path: String },
}

/// Plugin manifest (`plugin.toml`).
#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginMeta,

    /// Resource dependency presets.
    #[serde(default)]
    pub resources: HashMap<String, SchemaSource>,

    /// Chart dependency presets.
    #[serde(default)]
    pub charts: HashMap<String, ChartSource>,

    /// Module import mappings: specifier → relative file path.
    #[serde(default)]
    pub modules: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginMeta {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
}

pub const PLUGIN_MANIFEST: &str = "plugin.toml";

/// Load a plugin manifest from a directory.
pub fn load_plugin_manifest(plugin_dir: &Path) -> Result<PluginManifest, ConfigError> {
    let path = plugin_dir.join(PLUGIN_MANIFEST);
    if !path.exists() {
        return Err(ConfigError::Validation(format!(
            "plugin manifest not found: {}",
            path.display()
        )));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| ConfigError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let manifest: PluginManifest =
        toml::from_str(&content).map_err(|e| ConfigError::Parse(e.to_string()))?;
    Ok(manifest)
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

    // Detect deprecated [schemas] section
    if content.contains("[schemas]") && !content.contains("[resources]") {
        eprintln!("warning: [schemas] is deprecated in husako.toml, use [resources] instead");
    }

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
    for (name, source) in &config.resources {
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

    // Chart file paths must be relative
    for (name, source) in &config.charts {
        if let ChartSource::File { path } = source
            && Path::new(path).is_absolute()
        {
            return Err(ConfigError::Validation(format!(
                "chart '{name}' has absolute path '{path}'; use a relative path"
            )));
        }
    }

    // Plugin path sources must be relative
    for (name, source) in &config.plugins {
        if let PluginSource::Path { path } = source
            && Path::new(path).is_absolute()
        {
            return Err(ConfigError::Validation(format!(
                "plugin '{name}' has absolute path '{path}'; use a relative path"
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
        assert_eq!(config.resources.len(), 4);
        assert!(config.cluster.is_some());
        assert_eq!(config.cluster.unwrap().server, "https://10.0.0.1:6443");

        assert!(matches!(
            config.resources["kubernetes"],
            SchemaSource::Release { ref version } if version == "1.35"
        ));
        assert!(matches!(
            config.resources["cluster-crds"],
            SchemaSource::Cluster { cluster: None }
        ));
        assert!(matches!(
            config.resources["cert-manager"],
            SchemaSource::Git { .. }
        ));
        assert!(matches!(
            config.resources["my-crd"],
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
        assert!(config.resources.is_empty());
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
        assert!(config.resources.is_empty());
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
            resources: HashMap::from([(
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
            resources: HashMap::from([(
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
            resources: HashMap::from([(
                "crds".to_string(),
                SchemaSource::Cluster { cluster: None },
            )]),
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
            resources: HashMap::from([(
                "crds".to_string(),
                SchemaSource::Cluster { cluster: None },
            )]),
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

    #[test]
    fn parse_resources_section() {
        let toml = r#"
[resources]
kubernetes = { source = "release", version = "1.35" }
"#;
        let config: HusakoConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.resources.len(), 1);
        assert!(matches!(
            config.resources["kubernetes"],
            SchemaSource::Release { ref version } if version == "1.35"
        ));
    }

    #[test]
    fn schemas_alias_still_works() {
        let toml = r#"
[schemas]
kubernetes = { source = "release", version = "1.35" }
"#;
        let config: HusakoConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.resources.len(), 1);
    }

    #[test]
    fn parse_charts_section() {
        let toml = r#"
[charts]
ingress-nginx = { source = "registry", repo = "https://kubernetes.github.io/ingress-nginx", chart = "ingress-nginx", version = "4.12.0" }
postgresql = { source = "artifacthub", package = "bitnami/postgresql", version = "16.4.0" }
my-chart = { source = "file", path = "./schemas/my-chart.schema.json" }
my-other = { source = "git", repo = "https://github.com/example/repo", tag = "v1.0.0", path = "charts/my-chart" }
"#;
        let config: HusakoConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.charts.len(), 4);
        assert!(matches!(
            config.charts["ingress-nginx"],
            ChartSource::Registry { .. }
        ));
        assert!(matches!(
            config.charts["postgresql"],
            ChartSource::ArtifactHub { .. }
        ));
        assert!(matches!(
            config.charts["my-chart"],
            ChartSource::File { .. }
        ));
        assert!(matches!(config.charts["my-other"], ChartSource::Git { .. }));
    }

    #[test]
    fn reject_absolute_chart_file_path() {
        let config = HusakoConfig {
            charts: HashMap::from([(
                "my-chart".to_string(),
                ChartSource::File {
                    path: "/absolute/schema.json".to_string(),
                },
            )]),
            ..Default::default()
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("absolute path"));
    }

    #[test]
    fn parse_plugins_section() {
        let toml = r#"
[plugins]
flux = { source = "git", url = "https://github.com/nanazt/husako-plugin-flux" }
my-plugin = { source = "path", path = "./plugins/my-plugin" }
"#;
        let config: HusakoConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.plugins.len(), 2);
        assert!(matches!(
            config.plugins["flux"],
            PluginSource::Git { ref url } if url == "https://github.com/nanazt/husako-plugin-flux"
        ));
        assert!(matches!(
            config.plugins["my-plugin"],
            PluginSource::Path { ref path } if path == "./plugins/my-plugin"
        ));
    }

    #[test]
    fn reject_absolute_plugin_path() {
        let config = HusakoConfig {
            plugins: HashMap::from([(
                "test".to_string(),
                PluginSource::Path {
                    path: "/absolute/path".to_string(),
                },
            )]),
            ..Default::default()
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("absolute path"));
    }

    #[test]
    fn parse_plugin_manifest() {
        let toml = r#"
[plugin]
name = "flux"
version = "0.1.0"
description = "Flux CD integration for husako"

[resources]
flux-source = { source = "git", repo = "https://github.com/fluxcd/source-controller", tag = "v1.5.0", path = "config/crd/bases" }

[modules]
"flux" = "modules/index.js"
"flux/helm" = "modules/helm.js"
"#;
        let manifest: PluginManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.plugin.name, "flux");
        assert_eq!(manifest.plugin.version, "0.1.0");
        assert_eq!(manifest.plugin.description.as_deref(), Some("Flux CD integration for husako"));
        assert_eq!(manifest.resources.len(), 1);
        assert!(matches!(manifest.resources["flux-source"], SchemaSource::Git { .. }));
        assert_eq!(manifest.modules.len(), 2);
        assert_eq!(manifest.modules["flux"], "modules/index.js");
        assert_eq!(manifest.modules["flux/helm"], "modules/helm.js");
    }

    #[test]
    fn parse_plugin_manifest_minimal() {
        let toml = r#"
[plugin]
name = "test"
version = "0.1.0"
"#;
        let manifest: PluginManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.plugin.name, "test");
        assert!(manifest.resources.is_empty());
        assert!(manifest.charts.is_empty());
        assert!(manifest.modules.is_empty());
    }

    #[test]
    fn load_plugin_manifest_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let err = load_plugin_manifest(tmp.path()).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn load_plugin_manifest_valid() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("plugin.toml"),
            r#"
[plugin]
name = "test"
version = "0.1.0"

[modules]
"test" = "modules/index.js"
"#,
        )
        .unwrap();
        let manifest = load_plugin_manifest(tmp.path()).unwrap();
        assert_eq!(manifest.plugin.name, "test");
        assert_eq!(manifest.modules["test"], "modules/index.js");
    }

    #[test]
    fn parse_mixed_resources_and_charts() {
        let toml = r#"
[resources]
kubernetes = { source = "release", version = "1.35" }

[charts]
my-chart = { source = "file", path = "./values.schema.json" }
"#;
        let config: HusakoConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.resources.len(), 1);
        assert_eq!(config.charts.len(), 1);
    }
}
