use std::collections::HashMap;
use std::path::{Path, PathBuf};

use husako_config::{HusakoConfig, PluginManifest, PluginSource};

use crate::progress::ProgressReporter;
use crate::HusakoError;

/// Installed plugin data collected during `install_plugins`.
#[derive(Debug)]
pub struct InstalledPlugin {
    pub name: String,
    pub manifest: PluginManifest,
    pub dir: PathBuf,
}

/// Install all plugins declared in `[plugins]` to `.husako/plugins/<name>/`.
///
/// For `source = "git"`, shallow-clones the repository.
/// For `source = "path"`, copies the directory contents.
///
/// Returns a list of installed plugins with their parsed manifests.
pub fn install_plugins(
    config: &HusakoConfig,
    project_root: &Path,
    progress: &dyn ProgressReporter,
) -> Result<Vec<InstalledPlugin>, HusakoError> {
    if config.plugins.is_empty() {
        return Ok(Vec::new());
    }

    let plugins_dir = project_root.join(".husako/plugins");

    let mut installed = Vec::new();

    for (name, source) in &config.plugins {
        let plugin_dir = plugins_dir.join(name);
        let task = progress.start_task(&format!("Installing plugin {name}..."));

        match install_plugin(name, source, project_root, &plugin_dir) {
            Ok(()) => {
                match husako_config::load_plugin_manifest(&plugin_dir) {
                    Ok(manifest) => {
                        task.finish_ok(&format!("{name}: installed (v{})", manifest.plugin.version));
                        installed.push(InstalledPlugin {
                            name: name.clone(),
                            manifest,
                            dir: plugin_dir,
                        });
                    }
                    Err(e) => {
                        task.finish_err(&format!("{name}: invalid manifest: {e}"));
                        return Err(HusakoError::Config(e));
                    }
                }
            }
            Err(e) => {
                task.finish_err(&format!("{name}: {e}"));
                return Err(e);
            }
        }
    }

    Ok(installed)
}

fn install_plugin(
    name: &str,
    source: &PluginSource,
    project_root: &Path,
    target_dir: &Path,
) -> Result<(), HusakoError> {
    // Clean existing install
    if target_dir.exists() {
        std::fs::remove_dir_all(target_dir).map_err(|e| {
            HusakoError::GenerateIo(format!("remove {}: {e}", target_dir.display()))
        })?;
    }

    match source {
        PluginSource::Git { url } => install_git(name, url, target_dir),
        PluginSource::Path { path } => {
            let source_dir = project_root.join(path);
            install_path(name, &source_dir, target_dir)
        }
    }
}

fn install_git(name: &str, url: &str, target_dir: &Path) -> Result<(), HusakoError> {
    std::fs::create_dir_all(target_dir).map_err(|e| {
        HusakoError::GenerateIo(format!("create dir {}: {e}", target_dir.display()))
    })?;

    let output = std::process::Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--single-branch",
            url,
            &target_dir.to_string_lossy(),
        ])
        .output()
        .map_err(|e| {
            HusakoError::GenerateIo(format!("plugin '{name}': git clone failed: {e}"))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Clean up partial clone
        let _ = std::fs::remove_dir_all(target_dir);
        return Err(HusakoError::GenerateIo(format!(
            "plugin '{name}': git clone failed: {stderr}"
        )));
    }

    // Remove .git directory to save space
    let git_dir = target_dir.join(".git");
    if git_dir.exists() {
        let _ = std::fs::remove_dir_all(&git_dir);
    }

    Ok(())
}

fn install_path(name: &str, source_dir: &Path, target_dir: &Path) -> Result<(), HusakoError> {
    if !source_dir.is_dir() {
        return Err(HusakoError::GenerateIo(format!(
            "plugin '{name}': source directory not found: {}",
            source_dir.display()
        )));
    }

    copy_dir_recursive(source_dir, target_dir).map_err(|e| {
        HusakoError::GenerateIo(format!(
            "plugin '{name}': copy failed: {e}"
        ))
    })
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if entry.metadata()?.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Merge plugin resource presets into the config's resources map.
/// Plugin resources are added with a namespaced key `<plugin>/<resource>` to avoid collisions.
pub fn merge_plugin_presets(
    config: &mut HusakoConfig,
    plugins: &[InstalledPlugin],
) {
    for plugin in plugins {
        for (res_name, res_source) in &plugin.manifest.resources {
            let key = format!("{}:{}", plugin.name, res_name);
            config.resources.entry(key).or_insert_with(|| res_source.clone());
        }
        for (chart_name, chart_source) in &plugin.manifest.charts {
            let key = format!("{}:{}", plugin.name, chart_name);
            config.charts.entry(key).or_insert_with(|| chart_source.clone());
        }
    }
}

/// Build plugin module path mappings for tsconfig.json.
///
/// Returns a map of import specifier → `.d.ts` path (relative to project root).
pub fn plugin_tsconfig_paths(
    plugins: &[InstalledPlugin],
) -> HashMap<String, String> {
    let mut paths = HashMap::new();
    for plugin in plugins {
        for (specifier, rel_path) in &plugin.manifest.modules {
            // Convert .js path to .d.ts path for TypeScript
            let dts_path = rel_path.replace(".js", ".d.ts");
            let ts_path = format!(".husako/plugins/{}/{}", plugin.name, dts_path);
            paths.insert(specifier.clone(), ts_path);
        }
    }
    paths
}

/// Remove a plugin from `.husako/plugins/`.
pub fn remove_plugin(
    project_root: &Path,
    name: &str,
) -> Result<bool, HusakoError> {
    let plugin_dir = project_root.join(".husako/plugins").join(name);
    if plugin_dir.exists() {
        std::fs::remove_dir_all(&plugin_dir).map_err(|e| {
            HusakoError::GenerateIo(format!("remove {}: {e}", plugin_dir.display()))
        })?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// List installed plugins from `.husako/plugins/`.
pub fn list_plugins(
    project_root: &Path,
) -> Vec<InstalledPlugin> {
    let plugins_dir = project_root.join(".husako/plugins");
    if !plugins_dir.is_dir() {
        return Vec::new();
    }

    let Ok(entries) = std::fs::read_dir(&plugins_dir) else {
        return Vec::new();
    };

    let mut plugins = Vec::new();
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if let Ok(manifest) = husako_config::load_plugin_manifest(&dir) {
            plugins.push(InstalledPlugin {
                name,
                manifest,
                dir,
            });
        }
    }
    plugins.sort_by(|a, b| a.name.cmp(&b.name));
    plugins
}

#[cfg(test)]
mod tests {
    use super::*;
    use husako_config::{ChartSource, SchemaSource};

    #[test]
    fn install_path_source() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path();

        // Create a local plugin
        let plugin_src = project_root.join("my-plugin");
        std::fs::create_dir_all(plugin_src.join("modules")).unwrap();
        std::fs::write(
            plugin_src.join("plugin.toml"),
            r#"
[plugin]
name = "test"
version = "0.1.0"

[modules]
"test" = "modules/index.js"
"#,
        )
        .unwrap();
        std::fs::write(
            plugin_src.join("modules/index.js"),
            "export function hello() { return 42; }",
        )
        .unwrap();

        let config = HusakoConfig {
            plugins: HashMap::from([(
                "test".to_string(),
                PluginSource::Path {
                    path: "my-plugin".to_string(),
                },
            )]),
            ..Default::default()
        };

        let progress = crate::progress::SilentProgress;
        let installed = install_plugins(&config, project_root, &progress).unwrap();

        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].name, "test");
        assert_eq!(installed[0].manifest.plugin.version, "0.1.0");

        // Verify files were copied
        let installed_dir = project_root.join(".husako/plugins/test");
        assert!(installed_dir.join("plugin.toml").exists());
        assert!(installed_dir.join("modules/index.js").exists());
    }

    #[test]
    fn install_path_source_missing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path();

        let config = HusakoConfig {
            plugins: HashMap::from([(
                "test".to_string(),
                PluginSource::Path {
                    path: "nonexistent".to_string(),
                },
            )]),
            ..Default::default()
        };

        let progress = crate::progress::SilentProgress;
        let err = install_plugins(&config, project_root, &progress).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn install_replaces_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path();

        // Create plugin source
        let plugin_src = project_root.join("my-plugin");
        std::fs::create_dir_all(&plugin_src).unwrap();
        std::fs::write(
            plugin_src.join("plugin.toml"),
            "[plugin]\nname = \"test\"\nversion = \"0.2.0\"\n",
        )
        .unwrap();

        // Pre-create old installation
        let old_dir = project_root.join(".husako/plugins/test");
        std::fs::create_dir_all(&old_dir).unwrap();
        std::fs::write(
            old_dir.join("plugin.toml"),
            "[plugin]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let config = HusakoConfig {
            plugins: HashMap::from([(
                "test".to_string(),
                PluginSource::Path {
                    path: "my-plugin".to_string(),
                },
            )]),
            ..Default::default()
        };

        let progress = crate::progress::SilentProgress;
        let installed = install_plugins(&config, project_root, &progress).unwrap();

        assert_eq!(installed[0].manifest.plugin.version, "0.2.0");
    }

    #[test]
    fn merge_plugin_presets_adds_resources() {
        let mut config = HusakoConfig {
            resources: HashMap::from([(
                "kubernetes".to_string(),
                SchemaSource::Release {
                    version: "1.35".to_string(),
                },
            )]),
            ..Default::default()
        };

        let plugins = vec![InstalledPlugin {
            name: "flux".to_string(),
            manifest: PluginManifest {
                plugin: husako_config::PluginMeta {
                    name: "flux".to_string(),
                    version: "0.1.0".to_string(),
                    description: None,
                },
                resources: HashMap::from([(
                    "flux-source".to_string(),
                    SchemaSource::Git {
                        repo: "https://github.com/fluxcd/source-controller".to_string(),
                        tag: "v1.5.0".to_string(),
                        path: "config/crd/bases".to_string(),
                    },
                )]),
                charts: HashMap::new(),
                modules: HashMap::new(),
            },
            dir: PathBuf::from("/tmp/plugins/flux"),
        }];

        merge_plugin_presets(&mut config, &plugins);

        // Original resource preserved
        assert!(config.resources.contains_key("kubernetes"));
        // Plugin resource added with namespaced key
        assert!(config.resources.contains_key("flux:flux-source"));
    }

    #[test]
    fn merge_plugin_presets_adds_charts() {
        let mut config = HusakoConfig::default();

        let plugins = vec![InstalledPlugin {
            name: "my".to_string(),
            manifest: PluginManifest {
                plugin: husako_config::PluginMeta {
                    name: "my".to_string(),
                    version: "0.1.0".to_string(),
                    description: None,
                },
                resources: HashMap::new(),
                charts: HashMap::from([(
                    "nginx".to_string(),
                    ChartSource::Registry {
                        repo: "https://charts.bitnami.com/bitnami".to_string(),
                        chart: "nginx".to_string(),
                        version: "16.0.0".to_string(),
                    },
                )]),
                modules: HashMap::new(),
            },
            dir: PathBuf::from("/tmp/plugins/my"),
        }];

        merge_plugin_presets(&mut config, &plugins);
        assert!(config.charts.contains_key("my:nginx"));
    }

    #[test]
    fn plugin_tsconfig_paths_builds_mappings() {
        let plugins = vec![InstalledPlugin {
            name: "flux".to_string(),
            manifest: PluginManifest {
                plugin: husako_config::PluginMeta {
                    name: "flux".to_string(),
                    version: "0.1.0".to_string(),
                    description: None,
                },
                resources: HashMap::new(),
                charts: HashMap::new(),
                modules: HashMap::from([
                    ("flux".to_string(), "modules/index.js".to_string()),
                    ("flux/helm".to_string(), "modules/helm.js".to_string()),
                ]),
            },
            dir: PathBuf::from("/tmp/plugins/flux"),
        }];

        let paths = plugin_tsconfig_paths(&plugins);
        assert_eq!(
            paths["flux"],
            ".husako/plugins/flux/modules/index.d.ts"
        );
        assert_eq!(
            paths["flux/helm"],
            ".husako/plugins/flux/modules/helm.d.ts"
        );
    }

    #[test]
    fn remove_plugin_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path();

        let plugin_dir = project_root.join(".husako/plugins/test");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("plugin.toml"), "").unwrap();

        let removed = remove_plugin(project_root, "test").unwrap();
        assert!(removed);
        assert!(!plugin_dir.exists());
    }

    #[test]
    fn remove_plugin_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let removed = remove_plugin(tmp.path(), "nonexistent").unwrap();
        assert!(!removed);
    }

    #[test]
    fn list_plugins_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let plugins = list_plugins(tmp.path());
        assert!(plugins.is_empty());
    }

    #[test]
    fn list_plugins_finds_installed() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path();

        // Create two installed plugins
        for name in ["alpha", "beta"] {
            let plugin_dir = project_root.join(format!(".husako/plugins/{name}"));
            std::fs::create_dir_all(&plugin_dir).unwrap();
            std::fs::write(
                plugin_dir.join("plugin.toml"),
                format!("[plugin]\nname = \"{name}\"\nversion = \"0.1.0\"\n"),
            )
            .unwrap();
        }

        let plugins = list_plugins(project_root);
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].name, "alpha");
        assert_eq!(plugins[1].name, "beta");
    }

    #[test]
    fn empty_plugins_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config = HusakoConfig::default();
        let progress = crate::progress::SilentProgress;
        let installed = install_plugins(&config, tmp.path(), &progress).unwrap();
        assert!(installed.is_empty());
    }
}
