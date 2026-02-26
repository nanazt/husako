use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use husako_config::{
    ChartLockEntry, ChartSource, HusakoConfig, HusakoLock, PluginLockEntry, PluginManifest,
    PluginSource, ResourceLockEntry, SchemaSource,
};

/// Returns an RFC 3339 UTC timestamp string for lock entries.
pub fn utc_now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

// ---------------------------------------------------------------------------
// File / directory hashing (djb2)
// ---------------------------------------------------------------------------

fn djb2_bytes(data: &[u8]) -> u64 {
    let mut hash: u64 = 5381;
    for &b in data {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }
    hash
}

/// Hash a single file using djb2.
pub fn hash_file(path: &Path) -> Result<String, std::io::Error> {
    let content = std::fs::read(path)?;
    Ok(format!("{:016x}", djb2_bytes(&content)))
}

/// Hash a directory by sorting all contained files lexicographically and
/// hashing their relative path concatenated with their content.
pub fn hash_dir(dir: &Path) -> Result<String, std::io::Error> {
    let mut files = collect_all_files(dir)?;
    files.sort();
    let mut hash: u64 = 5381;
    for file in &files {
        let rel = file.strip_prefix(dir).unwrap_or(file);
        for b in rel.to_string_lossy().bytes() {
            hash = hash.wrapping_mul(33).wrapping_add(b as u64);
        }
        for b in std::fs::read(file)? {
            hash = hash.wrapping_mul(33).wrapping_add(b as u64);
        }
    }
    Ok(format!("{hash:016x}"))
}

fn collect_all_files(dir: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut result = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            result.extend(collect_all_files(&path)?);
        } else {
            result.push(path);
        }
    }
    Ok(result)
}

fn hash_path_source(path: &str, project_root: &Path) -> String {
    let full = project_root.join(path);
    if full.is_dir() {
        hash_dir(&full).unwrap_or_default()
    } else {
        hash_file(&full).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Skip decisions
// ---------------------------------------------------------------------------

/// Returns `true` if the entire k8s resource type generation block can be
/// skipped. All resources are treated as one unit: if any resource fails its
/// skip check, all k8s types are regenerated.
pub fn should_skip_k8s(
    config: Option<&HusakoConfig>,
    old_lock: Option<&HusakoLock>,
    husako_version: &str,
    types_dir: &Path,
    project_root: &Path,
) -> bool {
    let Some(lock) = old_lock else {
        return false;
    };
    let Some(config) = config else {
        return false;
    };

    // Binary version changed — always regenerate
    if lock.husako_version != husako_version {
        return false;
    }

    // k8s types directory must exist and be non-empty
    let k8s_dir = types_dir.join("k8s");
    if !k8s_dir.is_dir() {
        return false;
    }
    if std::fs::read_dir(&k8s_dir)
        .map(|mut d| d.next().is_none())
        .unwrap_or(true)
    {
        return false;
    }

    // Resource names in config must match lock exactly (no additions or removals)
    let config_names: BTreeSet<&String> = config.resources.keys().collect();
    let lock_names: BTreeSet<&String> = lock.resources.keys().collect();
    if config_names != lock_names {
        return false;
    }

    // Every resource must pass its identity check
    for (name, source) in &config.resources {
        if !resource_identity_matches(name, source, lock, project_root) {
            return false;
        }
    }

    true
}

fn resource_identity_matches(
    name: &str,
    source: &SchemaSource,
    lock: &HusakoLock,
    project_root: &Path,
) -> bool {
    match source {
        SchemaSource::Release { version } => {
            matches!(
                lock.resources.get(name),
                Some(ResourceLockEntry::Release { version: lv, .. }) if lv == version
            )
        }

        SchemaSource::Git { repo, tag, path } => {
            matches!(
                lock.resources.get(name),
                Some(ResourceLockEntry::Git { repo: lr, tag: lt, path: lp, .. })
                    if lr == repo && lt == tag && lp == path
            )
        }

        SchemaSource::File { path } => match lock.resources.get(name) {
            Some(ResourceLockEntry::File {
                path: lp,
                content_hash: lh,
                ..
            }) => {
                if lp != path {
                    return false;
                }
                let current_hash = hash_path_source(path, project_root);
                current_hash == *lh && !current_hash.is_empty()
            }
            _ => false,
        },
    }
}

/// Returns `true` if a single Helm chart's type generation can be skipped.
pub fn should_skip_chart(
    name: &str,
    source: &ChartSource,
    old_lock: Option<&HusakoLock>,
    husako_version: &str,
    types_dir: &Path,
    project_root: &Path,
) -> bool {
    let Some(lock) = old_lock else {
        return false;
    };

    // Binary version changed
    if lock.husako_version != husako_version {
        return false;
    }

    // Generated type file must exist
    if !types_dir.join(format!("helm/{name}.d.ts")).exists() {
        return false;
    }

    chart_identity_matches(name, source, lock, project_root)
}

fn chart_identity_matches(
    name: &str,
    source: &ChartSource,
    lock: &HusakoLock,
    project_root: &Path,
) -> bool {
    match source {
        ChartSource::Registry {
            repo,
            chart,
            version,
        } => {
            matches!(
                lock.charts.get(name),
                Some(ChartLockEntry::Registry { repo: lr, chart: lc, version: lv, .. })
                    if lr == repo && lc == chart && lv == version
            )
        }

        ChartSource::ArtifactHub { package, version } => {
            matches!(
                lock.charts.get(name),
                Some(ChartLockEntry::ArtifactHub { package: lp, version: lv, .. })
                    if lp == package && lv == version
            )
        }

        ChartSource::File { path } => match lock.charts.get(name) {
            Some(ChartLockEntry::File {
                path: lp,
                content_hash: lh,
                ..
            }) => {
                if lp != path {
                    return false;
                }
                let current = hash_path_source(path, project_root);
                current == *lh && !current.is_empty()
            }
            _ => false,
        },

        ChartSource::Git { repo, tag, path } => {
            matches!(
                lock.charts.get(name),
                Some(ChartLockEntry::Git { repo: lr, tag: lt, path: lp, .. })
                    if lr == repo && lt == tag && lp == path
            )
        }

        ChartSource::Oci { reference, version } => {
            matches!(
                lock.charts.get(name),
                Some(ChartLockEntry::Oci { reference: lr, version: lv, .. })
                    if lr == reference && lv == version
            )
        }
    }
}

/// Returns `true` if a plugin can be reused from the existing installation
/// without re-cloning or re-copying.
pub fn should_skip_plugin(
    name: &str,
    source: &PluginSource,
    old_lock: Option<&HusakoLock>,
    plugins_dir: &Path,
    project_root: &Path,
) -> bool {
    let Some(lock) = old_lock else {
        return false;
    };
    let Some(lock_entry) = lock.plugins.get(name) else {
        return false;
    };

    // Installed directory must exist
    let plugin_dir = plugins_dir.join(name);
    if !plugin_dir.is_dir() {
        return false;
    }

    match (source, lock_entry) {
        (
            PluginSource::Git {
                url,
                path: src_path,
            },
            PluginLockEntry::Git {
                url: lu,
                path: lp,
                plugin_version: lv,
                ..
            },
        ) => {
            if url != lu || src_path != lp {
                return false;
            }
            // Plugin version in installed manifest must match lock
            match husako_config::load_plugin_manifest(&plugin_dir) {
                Ok(m) => m.plugin.version == *lv,
                Err(_) => false,
            }
        }

        (
            PluginSource::Path { path },
            PluginLockEntry::Path {
                path: lp,
                content_hash: lh,
                plugin_version: lv,
                ..
            },
        ) => {
            if path != lp {
                return false;
            }
            // Check directory content hash
            let current_hash = hash_dir(&project_root.join(path)).unwrap_or_default();
            if current_hash != *lh || current_hash.is_empty() {
                return false;
            }
            // Check installed plugin version matches lock
            match husako_config::load_plugin_manifest(&plugin_dir) {
                Ok(m) => m.plugin.version == *lv,
                Err(_) => false,
            }
        }

        // Source type changed (e.g. was git, now path)
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Entry builders — called after successful generation to populate new_lock
// ---------------------------------------------------------------------------

/// Build lock entries for all k8s resource sources in the config.
pub fn build_resource_entries(
    config: &HusakoConfig,
    project_root: &Path,
) -> BTreeMap<String, ResourceLockEntry> {
    let now = utc_now();
    let mut entries = BTreeMap::new();
    for (name, source) in &config.resources {
        let entry = match source {
            SchemaSource::Release { version } => ResourceLockEntry::Release {
                version: version.clone(),
                generated_at: now.clone(),
            },
            SchemaSource::Git { repo, tag, path } => ResourceLockEntry::Git {
                repo: repo.clone(),
                tag: tag.clone(),
                path: path.clone(),
                generated_at: now.clone(),
            },
            SchemaSource::File { path } => ResourceLockEntry::File {
                path: path.clone(),
                content_hash: hash_path_source(path, project_root),
                generated_at: now.clone(),
            },
        };
        entries.insert(name.clone(), entry);
    }
    entries
}

/// Build a lock entry for a single Helm chart source.
pub fn build_chart_entry(source: &ChartSource, project_root: &Path) -> ChartLockEntry {
    let now = utc_now();
    match source {
        ChartSource::Registry {
            repo,
            chart,
            version,
        } => ChartLockEntry::Registry {
            repo: repo.clone(),
            chart: chart.clone(),
            version: version.clone(),
            generated_at: now,
        },
        ChartSource::ArtifactHub { package, version } => ChartLockEntry::ArtifactHub {
            package: package.clone(),
            version: version.clone(),
            generated_at: now,
        },
        ChartSource::File { path } => ChartLockEntry::File {
            path: path.clone(),
            content_hash: hash_path_source(path, project_root),
            generated_at: now,
        },
        ChartSource::Git { repo, tag, path } => ChartLockEntry::Git {
            repo: repo.clone(),
            tag: tag.clone(),
            path: path.clone(),
            generated_at: now,
        },
        ChartSource::Oci { reference, version } => ChartLockEntry::Oci {
            reference: reference.clone(),
            version: version.clone(),
            generated_at: now,
        },
    }
}

/// Build a lock entry for a plugin after successful installation.
pub fn build_plugin_entry(
    source: &PluginSource,
    plugin_dir: &Path,
    manifest: &PluginManifest,
    project_root: &Path,
) -> PluginLockEntry {
    let now = utc_now();
    let version = manifest.plugin.version.clone();
    match source {
        PluginSource::Git { url, path } => PluginLockEntry::Git {
            url: url.clone(),
            path: path.clone(),
            plugin_version: version,
            generated_at: now,
        },
        PluginSource::Path { path } => {
            let source_dir = project_root.join(path);
            let hash = hash_dir(&source_dir).unwrap_or_default();
            // If source dir hash failed, try hashing the installed dir as fallback
            let content_hash = if hash.is_empty() {
                hash_dir(plugin_dir).unwrap_or_default()
            } else {
                hash
            };
            PluginLockEntry::Path {
                path: path.clone(),
                content_hash,
                plugin_version: version,
                generated_at: now,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn make_lock(
        resources: BTreeMap<String, ResourceLockEntry>,
        charts: BTreeMap<String, ChartLockEntry>,
        plugins: BTreeMap<String, PluginLockEntry>,
    ) -> HusakoLock {
        HusakoLock {
            format_version: 1,
            husako_version: "0.3.0".to_string(),
            resources,
            charts,
            plugins,
        }
    }

    fn lock_with_resource(name: &str, entry: ResourceLockEntry) -> HusakoLock {
        let mut r = BTreeMap::new();
        r.insert(name.to_string(), entry);
        make_lock(r, BTreeMap::new(), BTreeMap::new())
    }

    fn config_with_resource(name: &str, source: SchemaSource) -> HusakoConfig {
        let mut resources = std::collections::HashMap::new();
        resources.insert(name.to_string(), source);
        HusakoConfig {
            resources,
            ..Default::default()
        }
    }

    // Helper: create a types dir with a non-empty k8s/ subdirectory
    fn make_k8s_types_dir(root: &Path) {
        let k8s_dir = root.join("k8s");
        std::fs::create_dir_all(&k8s_dir).unwrap();
        std::fs::write(k8s_dir.join("core.d.ts"), "export {};").unwrap();
    }

    // -----------------------------------------------------------------------
    // Resource skip tests
    // -----------------------------------------------------------------------

    #[test]
    fn skip_release_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_k8s_types_dir(root);
        let lock = lock_with_resource(
            "kubernetes",
            ResourceLockEntry::Release {
                version: "1.35".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let config = config_with_resource(
            "kubernetes",
            SchemaSource::Release {
                version: "1.35".to_string(),
            },
        );
        assert!(should_skip_k8s(
            Some(&config),
            Some(&lock),
            "0.3.0",
            root,
            root
        ));
    }

    #[test]
    fn skip_release_version_changed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_k8s_types_dir(root);
        let lock = lock_with_resource(
            "kubernetes",
            ResourceLockEntry::Release {
                version: "1.34".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let config = config_with_resource(
            "kubernetes",
            SchemaSource::Release {
                version: "1.35".to_string(),
            },
        );
        assert!(!should_skip_k8s(
            Some(&config),
            Some(&lock),
            "0.3.0",
            root,
            root
        ));
    }

    #[test]
    fn skip_git_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_k8s_types_dir(root);
        let lock = lock_with_resource(
            "cert-manager",
            ResourceLockEntry::Git {
                repo: "https://github.com/cert-manager/cert-manager".to_string(),
                tag: "v1.17.2".to_string(),
                path: "deploy/crds".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let config = config_with_resource(
            "cert-manager",
            SchemaSource::Git {
                repo: "https://github.com/cert-manager/cert-manager".to_string(),
                tag: "v1.17.2".to_string(),
                path: "deploy/crds".to_string(),
            },
        );
        assert!(should_skip_k8s(
            Some(&config),
            Some(&lock),
            "0.3.0",
            root,
            root
        ));
    }

    #[test]
    fn skip_git_tag_changed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_k8s_types_dir(root);
        let lock = lock_with_resource(
            "cert-manager",
            ResourceLockEntry::Git {
                repo: "https://github.com/cert-manager/cert-manager".to_string(),
                tag: "v1.17.1".to_string(),
                path: "deploy/crds".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let config = config_with_resource(
            "cert-manager",
            SchemaSource::Git {
                repo: "https://github.com/cert-manager/cert-manager".to_string(),
                tag: "v1.17.2".to_string(),
                path: "deploy/crds".to_string(),
            },
        );
        assert!(!should_skip_k8s(
            Some(&config),
            Some(&lock),
            "0.3.0",
            root,
            root
        ));
    }

    #[test]
    fn skip_file_content_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_k8s_types_dir(root);
        // Write a real file so hash_file works
        std::fs::write(root.join("my-crd.yaml"), b"kind: CustomResourceDefinition").unwrap();
        let real_hash = hash_file(&root.join("my-crd.yaml")).unwrap();

        let lock = lock_with_resource(
            "my-crd",
            ResourceLockEntry::File {
                path: "my-crd.yaml".to_string(),
                content_hash: real_hash.clone(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let config = config_with_resource(
            "my-crd",
            SchemaSource::File {
                path: "my-crd.yaml".to_string(),
            },
        );
        assert!(should_skip_k8s(
            Some(&config),
            Some(&lock),
            "0.3.0",
            root,
            root
        ));
    }

    #[test]
    fn skip_file_content_changed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_k8s_types_dir(root);
        std::fs::write(root.join("my-crd.yaml"), b"new content").unwrap();

        let lock = lock_with_resource(
            "my-crd",
            ResourceLockEntry::File {
                path: "my-crd.yaml".to_string(),
                content_hash: "0000000000000000".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let config = config_with_resource(
            "my-crd",
            SchemaSource::File {
                path: "my-crd.yaml".to_string(),
            },
        );
        assert!(!should_skip_k8s(
            Some(&config),
            Some(&lock),
            "0.3.0",
            root,
            root
        ));
    }

    #[test]
    fn k8s_no_skip_entry_added() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_k8s_types_dir(root);
        // Lock has "kubernetes", config has "kubernetes" + "new-dep"
        let lock = lock_with_resource(
            "kubernetes",
            ResourceLockEntry::Release {
                version: "1.35".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let mut resources = std::collections::HashMap::new();
        resources.insert(
            "kubernetes".to_string(),
            SchemaSource::Release {
                version: "1.35".to_string(),
            },
        );
        resources.insert(
            "new-dep".to_string(),
            SchemaSource::Release {
                version: "1.35".to_string(),
            },
        );
        let config = HusakoConfig {
            resources,
            ..Default::default()
        };
        assert!(!should_skip_k8s(
            Some(&config),
            Some(&lock),
            "0.3.0",
            root,
            root
        ));
    }

    #[test]
    fn k8s_no_skip_types_dir_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        // No k8s types dir created
        let lock = lock_with_resource(
            "kubernetes",
            ResourceLockEntry::Release {
                version: "1.35".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let config = config_with_resource(
            "kubernetes",
            SchemaSource::Release {
                version: "1.35".to_string(),
            },
        );
        assert!(!should_skip_k8s(
            Some(&config),
            Some(&lock),
            "0.3.0",
            root,
            root
        ));
    }

    #[test]
    fn k8s_no_skip_husako_version_changed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_k8s_types_dir(root);
        let lock = lock_with_resource(
            "kubernetes",
            ResourceLockEntry::Release {
                version: "1.35".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let config = config_with_resource(
            "kubernetes",
            SchemaSource::Release {
                version: "1.35".to_string(),
            },
        );
        // Lock has "0.2.0" but current version is "0.3.0"
        assert!(!should_skip_k8s(
            Some(&config),
            Some(&lock),
            "0.3.1", // different from lock's "0.3.0"
            root,
            root
        ));
    }

    // -----------------------------------------------------------------------
    // Chart skip tests
    // -----------------------------------------------------------------------

    fn make_chart_lock(name: &str, entry: ChartLockEntry) -> HusakoLock {
        let mut charts = BTreeMap::new();
        charts.insert(name.to_string(), entry);
        make_lock(BTreeMap::new(), charts, BTreeMap::new())
    }

    fn make_helm_types_dir(root: &Path, chart_name: &str) {
        let dir = root.join("helm");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{chart_name}.d.ts")), "export {};").unwrap();
    }

    #[test]
    fn skip_chart_registry_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_helm_types_dir(root, "ingress-nginx");
        let lock = make_chart_lock(
            "ingress-nginx",
            ChartLockEntry::Registry {
                repo: "https://kubernetes.github.io/ingress-nginx".to_string(),
                chart: "ingress-nginx".to_string(),
                version: "4.12.0".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let source = ChartSource::Registry {
            repo: "https://kubernetes.github.io/ingress-nginx".to_string(),
            chart: "ingress-nginx".to_string(),
            version: "4.12.0".to_string(),
        };
        assert!(should_skip_chart(
            "ingress-nginx",
            &source,
            Some(&lock),
            "0.3.0",
            root,
            root
        ));
    }

    #[test]
    fn skip_chart_registry_version_changed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_helm_types_dir(root, "ingress-nginx");
        let lock = make_chart_lock(
            "ingress-nginx",
            ChartLockEntry::Registry {
                repo: "https://kubernetes.github.io/ingress-nginx".to_string(),
                chart: "ingress-nginx".to_string(),
                version: "4.11.0".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let source = ChartSource::Registry {
            repo: "https://kubernetes.github.io/ingress-nginx".to_string(),
            chart: "ingress-nginx".to_string(),
            version: "4.12.0".to_string(),
        };
        assert!(!should_skip_chart(
            "ingress-nginx",
            &source,
            Some(&lock),
            "0.3.0",
            root,
            root
        ));
    }

    #[test]
    fn skip_chart_artifacthub_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_helm_types_dir(root, "postgresql");
        let lock = make_chart_lock(
            "postgresql",
            ChartLockEntry::ArtifactHub {
                package: "bitnami/postgresql".to_string(),
                version: "16.4.0".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let source = ChartSource::ArtifactHub {
            package: "bitnami/postgresql".to_string(),
            version: "16.4.0".to_string(),
        };
        assert!(should_skip_chart(
            "postgresql",
            &source,
            Some(&lock),
            "0.3.0",
            root,
            root
        ));
    }

    #[test]
    fn skip_chart_oci_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_helm_types_dir(root, "pg");
        let lock = make_chart_lock(
            "pg",
            ChartLockEntry::Oci {
                reference: "oci://ghcr.io/org/postgresql".to_string(),
                version: "1.2.3".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let source = ChartSource::Oci {
            reference: "oci://ghcr.io/org/postgresql".to_string(),
            version: "1.2.3".to_string(),
        };
        assert!(should_skip_chart(
            "pg",
            &source,
            Some(&lock),
            "0.3.0",
            root,
            root
        ));
    }

    #[test]
    fn no_skip_chart_types_file_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        // No helm types dir created
        let lock = make_chart_lock(
            "ingress-nginx",
            ChartLockEntry::Registry {
                repo: "https://kubernetes.github.io/ingress-nginx".to_string(),
                chart: "ingress-nginx".to_string(),
                version: "4.12.0".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let source = ChartSource::Registry {
            repo: "https://kubernetes.github.io/ingress-nginx".to_string(),
            chart: "ingress-nginx".to_string(),
            version: "4.12.0".to_string(),
        };
        assert!(!should_skip_chart(
            "ingress-nginx",
            &source,
            Some(&lock),
            "0.3.0",
            root,
            root
        ));
    }

    #[test]
    fn no_skip_chart_husako_version_changed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_helm_types_dir(root, "postgresql");
        let lock = make_chart_lock(
            "postgresql",
            ChartLockEntry::ArtifactHub {
                package: "bitnami/postgresql".to_string(),
                version: "16.4.0".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let source = ChartSource::ArtifactHub {
            package: "bitnami/postgresql".to_string(),
            version: "16.4.0".to_string(),
        };
        assert!(!should_skip_chart(
            "postgresql",
            &source,
            Some(&lock),
            "0.4.0", // different from lock's "0.3.0"
            root,
            root
        ));
    }

    // -----------------------------------------------------------------------
    // Plugin skip tests
    // -----------------------------------------------------------------------

    fn make_plugin_lock(name: &str, entry: PluginLockEntry) -> HusakoLock {
        let mut plugins = BTreeMap::new();
        plugins.insert(name.to_string(), entry);
        make_lock(BTreeMap::new(), BTreeMap::new(), plugins)
    }

    fn write_plugin_manifest(dir: &Path, version: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("plugin.toml"),
            format!("[plugin]\nname = \"test\"\nversion = \"{version}\"\n"),
        )
        .unwrap();
    }

    #[test]
    fn skip_plugin_git_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let plugins_dir = tmp.path().join("plugins");
        let plugin_dir = plugins_dir.join("fluxcd");
        write_plugin_manifest(&plugin_dir, "0.2.0");

        let lock = make_plugin_lock(
            "fluxcd",
            PluginLockEntry::Git {
                url: "https://github.com/nanazt/husako".to_string(),
                path: Some("plugins/fluxcd".to_string()),
                plugin_version: "0.2.0".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let source = PluginSource::Git {
            url: "https://github.com/nanazt/husako".to_string(),
            path: Some("plugins/fluxcd".to_string()),
        };
        assert!(should_skip_plugin(
            "fluxcd",
            &source,
            Some(&lock),
            &plugins_dir,
            tmp.path()
        ));
    }

    #[test]
    fn skip_plugin_git_url_changed() {
        let tmp = tempfile::tempdir().unwrap();
        let plugins_dir = tmp.path().join("plugins");
        let plugin_dir = plugins_dir.join("fluxcd");
        write_plugin_manifest(&plugin_dir, "0.2.0");

        let lock = make_plugin_lock(
            "fluxcd",
            PluginLockEntry::Git {
                url: "https://github.com/old/repo".to_string(),
                path: None,
                plugin_version: "0.2.0".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let source = PluginSource::Git {
            url: "https://github.com/new/repo".to_string(),
            path: None,
        };
        assert!(!should_skip_plugin(
            "fluxcd",
            &source,
            Some(&lock),
            &plugins_dir,
            tmp.path()
        ));
    }

    #[test]
    fn skip_plugin_plugin_version_changed() {
        let tmp = tempfile::tempdir().unwrap();
        let plugins_dir = tmp.path().join("plugins");
        let plugin_dir = plugins_dir.join("fluxcd");
        // Installed manifest says 0.3.0, lock says 0.2.0 → don't skip
        write_plugin_manifest(&plugin_dir, "0.3.0");

        let lock = make_plugin_lock(
            "fluxcd",
            PluginLockEntry::Git {
                url: "https://github.com/nanazt/husako".to_string(),
                path: None,
                plugin_version: "0.2.0".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let source = PluginSource::Git {
            url: "https://github.com/nanazt/husako".to_string(),
            path: None,
        };
        assert!(!should_skip_plugin(
            "fluxcd",
            &source,
            Some(&lock),
            &plugins_dir,
            tmp.path()
        ));
    }

    #[test]
    fn no_skip_plugin_installed_dir_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let plugins_dir = tmp.path().join("plugins");
        // Don't create plugin_dir

        let lock = make_plugin_lock(
            "fluxcd",
            PluginLockEntry::Git {
                url: "https://github.com/nanazt/husako".to_string(),
                path: None,
                plugin_version: "0.2.0".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        let source = PluginSource::Git {
            url: "https://github.com/nanazt/husako".to_string(),
            path: None,
        };
        assert!(!should_skip_plugin(
            "fluxcd",
            &source,
            Some(&lock),
            &plugins_dir,
            tmp.path()
        ));
    }

    // -----------------------------------------------------------------------
    // Hashing tests
    // -----------------------------------------------------------------------

    #[test]
    fn hash_dir_detects_content_change() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        let h1 = hash_dir(tmp.path()).unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"world").unwrap();
        let h2 = hash_dir(tmp.path()).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_dir_detects_rename() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        let h1 = hash_dir(tmp.path()).unwrap();
        std::fs::rename(tmp.path().join("a.txt"), tmp.path().join("b.txt")).unwrap();
        let h2 = hash_dir(tmp.path()).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_dir_detects_new_file() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        let h1 = hash_dir(tmp.path()).unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"world").unwrap();
        let h2 = hash_dir(tmp.path()).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_dir_is_deterministic() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"world").unwrap();
        let h1 = hash_dir(tmp.path()).unwrap();
        let h2 = hash_dir(tmp.path()).unwrap();
        assert_eq!(h1, h2);
    }
}
