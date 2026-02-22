use std::collections::HashMap;
use std::path::Path;

use husako_config::{HusakoConfig, SchemaSource};
use serde_json::Value;

use crate::HusakoError;

/// Resolve all schema sources from a `husako.toml` config.
///
/// Returns a merged `HashMap<String, Value>` where keys are discovery paths
/// (e.g., `"apis/cert-manager.io/v1"`) and values are OpenAPI spec JSON.
/// Later sources override for the same key.
pub fn resolve_all(
    config: &HusakoConfig,
    project_root: &Path,
    cache_dir: &Path,
) -> Result<HashMap<String, Value>, HusakoError> {
    let mut merged = HashMap::new();

    for (name, source) in &config.resources {
        let specs = match source {
            SchemaSource::File { path } => resolve_file(path, project_root)?,
            SchemaSource::Cluster { cluster } => {
                resolve_cluster(config, cluster.as_deref(), cache_dir)?
            }
            SchemaSource::Release { version } => resolve_release(version, cache_dir)?,
            SchemaSource::Git { repo, tag, path } => resolve_git(repo, tag, path, cache_dir)?,
        };

        if specs.is_empty() {
            eprintln!("warning: schema source '{name}' produced no specs");
        }

        merged.extend(specs);
    }

    Ok(merged)
}

/// Resolve a file-based schema source.
///
/// If `path` points to a single file, parse it as CRD YAML.
/// If `path` points to a directory, read all `.yaml`/`.yml` files and convert.
fn resolve_file(path: &str, project_root: &Path) -> Result<HashMap<String, Value>, HusakoError> {
    let resolved = project_root.join(path);

    if !resolved.exists() {
        return Err(HusakoError::GenerateIo(format!(
            "schema source path not found: {}",
            resolved.display()
        )));
    }

    let yaml = if resolved.is_dir() {
        read_crd_directory(&resolved)?
    } else {
        std::fs::read_to_string(&resolved)
            .map_err(|e| HusakoError::GenerateIo(format!("read {}: {e}", resolved.display())))?
    };

    let openapi = husako_openapi::crd::crd_yaml_to_openapi(&yaml)?;

    // Build discovery keys from the schemas' GVK
    crd_openapi_to_specs(&openapi)
}

/// Read all `.yaml`/`.yml` files in a directory and concatenate them.
fn read_crd_directory(dir: &Path) -> Result<String, HusakoError> {
    let mut parts = Vec::new();
    let entries = std::fs::read_dir(dir)
        .map_err(|e| HusakoError::GenerateIo(format!("read dir {}: {e}", dir.display())))?;

    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
        })
        .collect();
    paths.sort();

    for path in paths {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| HusakoError::GenerateIo(format!("read {}: {e}", path.display())))?;
        parts.push(content);
    }

    if parts.is_empty() {
        return Err(HusakoError::GenerateIo(format!(
            "no .yaml/.yml files found in {}",
            dir.display()
        )));
    }

    Ok(parts.join("\n---\n"))
}

/// Convert CRD-parsed OpenAPI JSON to discovery-keyed specs.
///
/// Groups schemas by their GVK group/version to produce keys like `"apis/cert-manager.io/v1"`.
fn crd_openapi_to_specs(openapi: &Value) -> Result<HashMap<String, Value>, HusakoError> {
    let schemas = openapi
        .pointer("/components/schemas")
        .and_then(Value::as_object)
        .ok_or_else(|| HusakoError::GenerateIo("invalid CRD OpenAPI output".to_string()))?;

    // Group schemas by their discovery key (derived from GVK)
    let mut grouped: HashMap<String, serde_json::Map<String, Value>> = HashMap::new();

    for (name, schema) in schemas {
        let gvk = schema.get("x-kubernetes-group-version-kind");
        let discovery_key = if let Some(gvk_arr) = gvk
            && let Some(gvk_obj) = gvk_arr.as_array().and_then(|a| a.first())
        {
            let group = gvk_obj["group"].as_str().unwrap_or("");
            let version = gvk_obj["version"].as_str().unwrap_or("");
            if group.is_empty() {
                format!("api/{version}")
            } else {
                format!("apis/{group}/{version}")
            }
        } else {
            // Non-GVK schemas: try to derive key from the schema name
            derive_discovery_key(name)
        };

        grouped
            .entry(discovery_key)
            .or_default()
            .insert(name.clone(), schema.clone());
    }

    let mut result = HashMap::new();
    for (key, schemas_map) in grouped {
        result.insert(
            key,
            serde_json::json!({
                "components": {
                    "schemas": schemas_map
                }
            }),
        );
    }

    Ok(result)
}

/// Derive a discovery key from a schema name.
/// `io.cert-manager.v1.CertificateSpec` → `apis/cert-manager.io/v1`
fn derive_discovery_key(name: &str) -> String {
    // Schema names follow pattern: io.<reversed-group>.<version>.<Type>
    // We need to reverse-engineer the group from the name
    let parts: Vec<&str> = name.split('.').collect();
    if parts.len() < 4 {
        return format!("apis/unknown/{name}");
    }

    // Find the version segment (starts with 'v' followed by a digit)
    let version_idx = parts.iter().position(|p| {
        p.starts_with('v') && p.len() > 1 && p[1..].starts_with(|c: char| c.is_ascii_digit())
    });

    if let Some(vi) = version_idx {
        let version = parts[vi];
        // Reverse the prefix parts (skip the first since it's from the reversed domain)
        let prefix_parts = &parts[..vi];
        let group_parts: Vec<&str> = prefix_parts.iter().rev().copied().collect();
        let group = group_parts.join(".");

        if group.is_empty() || group == "k8s.io" || group == "io" {
            format!("api/{version}")
        } else {
            format!("apis/{group}/{version}")
        }
    } else {
        format!("apis/unknown/{name}")
    }
}

/// Resolve a cluster-based schema source.
fn resolve_cluster(
    config: &HusakoConfig,
    cluster_name: Option<&str>,
    cache_dir: &Path,
) -> Result<HashMap<String, Value>, HusakoError> {
    let server =
        if let Some(name) = cluster_name {
            config
                .clusters
                .get(name)
                .map(|c| &c.server)
                .ok_or_else(|| {
                    HusakoError::GenerateIo(format!("cluster '{name}' not found in config"))
                })?
        } else {
            config.cluster.as_ref().map(|c| &c.server).ok_or_else(|| {
                HusakoError::GenerateIo("no [cluster] section in config".to_string())
            })?
        };

    let creds = husako_openapi::kubeconfig::resolve_credentials(server)?;

    let client = husako_openapi::OpenApiClient::new(husako_openapi::FetchOptions {
        source: husako_openapi::OpenApiSource::Url {
            base_url: creds.server,
            bearer_token: Some(creds.bearer_token),
        },
        cache_dir: cache_dir.to_path_buf(),
        offline: false,
    })?;

    let specs = client.fetch_all_specs()?;
    Ok(specs)
}

/// Resolve a GitHub release schema source.
fn resolve_release(version: &str, cache_dir: &Path) -> Result<HashMap<String, Value>, HusakoError> {
    let specs = husako_openapi::release::fetch_release_specs(version, cache_dir)?;
    Ok(specs)
}

/// Resolve a git-based schema source.
fn resolve_git(
    repo: &str,
    tag: &str,
    path: &str,
    cache_dir: &Path,
) -> Result<HashMap<String, Value>, HusakoError> {
    let repo_hash = simple_hash(repo);
    let git_cache = cache_dir.join(format!("git/{repo_hash}/{tag}"));

    // Check cache
    if git_cache.exists() {
        return load_git_cache(&git_cache);
    }

    // Clone repo at specific tag
    let temp_dir = tempfile::tempdir()
        .map_err(|e| HusakoError::GenerateIo(format!("create temp dir: {e}")))?;

    let status = std::process::Command::new("git")
        .args(["clone", "--depth", "1", "--branch", tag, repo])
        .arg(temp_dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .status()
        .map_err(|e| HusakoError::GenerateIo(format!("git clone failed: {e}")))?;

    if !status.success() {
        return Err(HusakoError::GenerateIo(format!(
            "git clone {repo} at tag {tag} failed (exit {})",
            status.code().unwrap_or(-1)
        )));
    }

    // Read CRD YAML files
    let crd_dir = temp_dir.path().join(path);
    if !crd_dir.exists() {
        return Err(HusakoError::GenerateIo(format!(
            "path '{path}' not found in repository"
        )));
    }

    let yaml = read_crd_directory(&crd_dir)?;
    let openapi = husako_openapi::crd::crd_yaml_to_openapi(&yaml)?;
    let specs = crd_openapi_to_specs(&openapi)?;

    // Cache the converted specs
    std::fs::create_dir_all(&git_cache)
        .map_err(|e| HusakoError::GenerateIo(format!("create cache dir: {e}")))?;
    for (key, spec) in &specs {
        let filename = key.replace('/', "__") + ".json";
        let _ = std::fs::write(
            git_cache.join(&filename),
            serde_json::to_string(spec).unwrap_or_default(),
        );
    }

    Ok(specs)
}

fn load_git_cache(cache_dir: &Path) -> Result<HashMap<String, Value>, HusakoError> {
    let mut specs = HashMap::new();
    let entries = std::fs::read_dir(cache_dir)
        .map_err(|e| HusakoError::GenerateIo(format!("read cache dir: {e}")))?;

    for entry in entries {
        let entry = entry.map_err(|e| HusakoError::GenerateIo(format!("read entry: {e}")))?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let filename = path.file_stem().unwrap().to_string_lossy();
            let key = filename.replace("__", "/");
            let data = std::fs::read_to_string(&path)
                .map_err(|e| HusakoError::GenerateIo(format!("read {}: {e}", path.display())))?;
            let spec: Value = serde_json::from_str(&data)
                .map_err(|e| HusakoError::GenerateIo(format!("parse {}: {e}", path.display())))?;
            specs.insert(key, spec);
        }
    }

    Ok(specs)
}

/// Simple hash for cache directory naming.
fn simple_hash(s: &str) -> String {
    let mut hash: u64 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_file_single() {
        let tmp = tempfile::tempdir().unwrap();
        let crd_path = tmp.path().join("my-crd.yaml");
        std::fs::write(
            &crd_path,
            r#"
apiVersion: apiextensions.k8s.io/v1
kind: CustomResourceDefinition
metadata:
  name: widgets.example.com
spec:
  group: example.com
  names:
    kind: Widget
    plural: widgets
  scope: Namespaced
  versions:
    - name: v1
      served: true
      storage: true
      schema:
        openAPIV3Schema:
          type: object
          properties:
            spec:
              type: object
              properties:
                size:
                  type: integer
"#,
        )
        .unwrap();

        let result = resolve_file("my-crd.yaml", tmp.path()).unwrap();
        assert!(!result.is_empty());
        // Should produce a discovery key for apis/example.com/v1
        assert!(result.contains_key("apis/example.com/v1"));
    }

    #[test]
    fn resolve_file_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let crds_dir = tmp.path().join("crds");
        std::fs::create_dir_all(&crds_dir).unwrap();

        std::fs::write(
            crds_dir.join("widget.yaml"),
            r#"
apiVersion: apiextensions.k8s.io/v1
kind: CustomResourceDefinition
metadata:
  name: widgets.example.com
spec:
  group: example.com
  names:
    kind: Widget
    plural: widgets
  scope: Namespaced
  versions:
    - name: v1
      served: true
      storage: true
      schema:
        openAPIV3Schema:
          type: object
          properties:
            spec:
              type: object
              properties:
                size:
                  type: integer
"#,
        )
        .unwrap();

        std::fs::write(
            crds_dir.join("gadget.yml"),
            r#"
apiVersion: apiextensions.k8s.io/v1
kind: CustomResourceDefinition
metadata:
  name: gadgets.example.com
spec:
  group: example.com
  names:
    kind: Gadget
    plural: gadgets
  scope: Namespaced
  versions:
    - name: v1
      served: true
      storage: true
      schema:
        openAPIV3Schema:
          type: object
          properties:
            spec:
              type: object
              properties:
                name:
                  type: string
"#,
        )
        .unwrap();

        let result = resolve_file("crds", tmp.path()).unwrap();
        let spec = &result["apis/example.com/v1"];
        let schemas = spec["components"]["schemas"].as_object().unwrap();
        // Both CRDs should be in the same group-version spec
        assert!(schemas.contains_key("com.example.v1.Widget"));
        assert!(schemas.contains_key("com.example.v1.Gadget"));
    }

    #[test]
    fn resolve_file_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let err = resolve_file("nonexistent.yaml", tmp.path()).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn crd_dir_reading_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("empty");
        std::fs::create_dir_all(&dir).unwrap();
        let err = read_crd_directory(&dir).unwrap_err();
        assert!(err.to_string().contains("no .yaml/.yml files"));
    }

    #[test]
    fn derive_discovery_key_from_name() {
        assert_eq!(
            derive_discovery_key("io.cert-manager.v1.CertificateSpec"),
            "apis/cert-manager.io/v1"
        );
        assert_eq!(
            derive_discovery_key("io.cnpg.postgresql.v1.ClusterSpec"),
            "apis/postgresql.cnpg.io/v1"
        );
    }

    #[test]
    fn simple_hash_deterministic() {
        let h1 = simple_hash("https://github.com/cert-manager/cert-manager");
        let h2 = simple_hash("https://github.com/cert-manager/cert-manager");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16);
    }

    #[test]
    fn git_cache_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path().join("git/test/v1");
        std::fs::create_dir_all(&cache_dir).unwrap();

        let spec = serde_json::json!({"components": {"schemas": {"test": {}}}});
        std::fs::write(
            cache_dir.join("apis__example.com__v1.json"),
            serde_json::to_string(&spec).unwrap(),
        )
        .unwrap();

        let result = load_git_cache(&cache_dir).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("apis/example.com/v1"));
    }
}
