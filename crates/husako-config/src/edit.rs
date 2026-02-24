use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, Item, Table, value};

use crate::{CONFIG_FILENAME, ChartSource, ConfigError, PluginSource, SchemaSource};

/// Load the husako.toml as a format-preserving TOML document.
pub fn load_document(project_root: &Path) -> Result<(DocumentMut, PathBuf), ConfigError> {
    let path = project_root.join(CONFIG_FILENAME);
    if !path.exists() {
        return Err(ConfigError::Io {
            path: path.display().to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "husako.toml not found"),
        });
    }
    let content = std::fs::read_to_string(&path).map_err(|e| ConfigError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let doc: DocumentMut = content
        .parse()
        .map_err(|e: toml_edit::TomlError| ConfigError::Parse(e.to_string()))?;
    Ok((doc, path))
}

/// Save the TOML document back to disk.
pub fn save_document(doc: &DocumentMut, path: &Path) -> Result<(), ConfigError> {
    std::fs::write(path, doc.to_string()).map_err(|e| ConfigError::Io {
        path: path.display().to_string(),
        source: e,
    })
}

/// Add a resource entry to the [resources] section.
pub fn add_resource(doc: &mut DocumentMut, name: &str, source: &SchemaSource) {
    ensure_table(doc, "resources");

    let inline = source_to_inline_table(source);
    doc["resources"][name] = Item::Value(toml_edit::Value::InlineTable(inline));
}

/// Add a chart entry to the [charts] section.
pub fn add_chart(doc: &mut DocumentMut, name: &str, source: &ChartSource) {
    ensure_table(doc, "charts");

    let inline = chart_source_to_inline_table(source);
    doc["charts"][name] = Item::Value(toml_edit::Value::InlineTable(inline));
}

/// Update the version of a resource entry. Returns true if found and updated.
pub fn update_resource_version(doc: &mut DocumentMut, name: &str, new_version: &str) -> bool {
    if let Some(table) = doc.get_mut("resources").and_then(|t| t.as_table_like_mut())
        && let Some(entry) = table.get_mut(name)
    {
        return update_version_in_item(entry, new_version);
    }
    false
}

/// Update the version of a chart entry. Returns true if found and updated.
pub fn update_chart_version(doc: &mut DocumentMut, name: &str, new_version: &str) -> bool {
    if let Some(table) = doc.get_mut("charts").and_then(|t| t.as_table_like_mut())
        && let Some(entry) = table.get_mut(name)
    {
        return update_version_in_item(entry, new_version);
    }
    false
}

/// Remove a resource entry. Returns true if found and removed.
pub fn remove_resource(doc: &mut DocumentMut, name: &str) -> bool {
    if let Some(table) = doc.get_mut("resources").and_then(|t| t.as_table_like_mut()) {
        return table.remove(name).is_some();
    }
    false
}

/// Remove a chart entry. Returns true if found and removed.
pub fn remove_chart(doc: &mut DocumentMut, name: &str) -> bool {
    if let Some(table) = doc.get_mut("charts").and_then(|t| t.as_table_like_mut()) {
        return table.remove(name).is_some();
    }
    false
}

/// Add a plugin entry to the [plugins] section.
pub fn add_plugin(doc: &mut DocumentMut, name: &str, source: &PluginSource) {
    ensure_table(doc, "plugins");

    let inline = plugin_source_to_inline_table(source);
    doc["plugins"][name] = Item::Value(toml_edit::Value::InlineTable(inline));
}

/// Remove a plugin entry. Returns true if found and removed.
pub fn remove_plugin(doc: &mut DocumentMut, name: &str) -> bool {
    if let Some(table) = doc.get_mut("plugins").and_then(|t| t.as_table_like_mut()) {
        return table.remove(name).is_some();
    }
    false
}

fn ensure_table(doc: &mut DocumentMut, key: &str) {
    if !doc.contains_key(key) {
        doc[key] = Item::Table(Table::new());
    }
}

fn update_version_in_item(item: &mut Item, new_version: &str) -> bool {
    // Handle inline table: { source = "release", version = "1.35" }
    if let Some(inline) = item.as_inline_table_mut() {
        if inline.contains_key("version") {
            inline.insert("version", new_version.into());
            return true;
        }
        if inline.contains_key("tag") {
            inline.insert("tag", new_version.into());
            return true;
        }
    }
    // Handle standard table
    if let Some(table) = item.as_table_like_mut() {
        if table.contains_key("version") {
            table.insert("version", value(new_version));
            return true;
        }
        if table.contains_key("tag") {
            table.insert("tag", value(new_version));
            return true;
        }
    }
    false
}

fn source_to_inline_table(source: &SchemaSource) -> toml_edit::InlineTable {
    let mut t = toml_edit::InlineTable::new();
    match source {
        SchemaSource::Release { version } => {
            t.insert("source", "release".into());
            t.insert("version", version.as_str().into());
        }
        SchemaSource::Cluster { cluster } => {
            t.insert("source", "cluster".into());
            if let Some(c) = cluster {
                t.insert("cluster", c.as_str().into());
            }
        }
        SchemaSource::Git { repo, tag, path } => {
            t.insert("source", "git".into());
            t.insert("repo", repo.as_str().into());
            t.insert("tag", tag.as_str().into());
            t.insert("path", path.as_str().into());
        }
        SchemaSource::File { path } => {
            t.insert("source", "file".into());
            t.insert("path", path.as_str().into());
        }
    }
    t
}

fn chart_source_to_inline_table(source: &ChartSource) -> toml_edit::InlineTable {
    let mut t = toml_edit::InlineTable::new();
    match source {
        ChartSource::Registry {
            repo,
            chart,
            version,
        } => {
            t.insert("source", "registry".into());
            t.insert("repo", repo.as_str().into());
            t.insert("chart", chart.as_str().into());
            t.insert("version", version.as_str().into());
        }
        ChartSource::ArtifactHub { package, version } => {
            t.insert("source", "artifacthub".into());
            t.insert("package", package.as_str().into());
            t.insert("version", version.as_str().into());
        }
        ChartSource::File { path } => {
            t.insert("source", "file".into());
            t.insert("path", path.as_str().into());
        }
        ChartSource::Git { repo, tag, path } => {
            t.insert("source", "git".into());
            t.insert("repo", repo.as_str().into());
            t.insert("tag", tag.as_str().into());
            t.insert("path", path.as_str().into());
        }
        ChartSource::Oci { reference, version } => {
            t.insert("source", "oci".into());
            t.insert("reference", reference.as_str().into());
            t.insert("version", version.as_str().into());
        }
    }
    t
}

fn plugin_source_to_inline_table(source: &PluginSource) -> toml_edit::InlineTable {
    let mut t = toml_edit::InlineTable::new();
    match source {
        PluginSource::Git { url, path } => {
            t.insert("source", "git".into());
            t.insert("url", url.as_str().into());
            if let Some(p) = path {
                t.insert("path", p.as_str().into());
            }
        }
        PluginSource::Path { path } => {
            t.insert("source", "path".into());
            t.insert("path", path.as_str().into());
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_toml(content: &str) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(CONFIG_FILENAME);
        std::fs::write(&path, content).unwrap();
        (tmp, path)
    }

    #[test]
    fn add_resource_release() {
        let (_tmp, path) = create_test_toml("[entries]\ndev = \"env/dev.ts\"\n");
        let mut doc: DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();

        add_resource(
            &mut doc,
            "kubernetes",
            &SchemaSource::Release {
                version: "1.35".to_string(),
            },
        );

        let output = doc.to_string();
        assert!(output.contains("[resources]"));
        assert!(output.contains("kubernetes"));
        assert!(output.contains("release"));
        assert!(output.contains("1.35"));
    }

    #[test]
    fn add_chart_registry() {
        let (_tmp, path) = create_test_toml("");
        let mut doc: DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();

        add_chart(
            &mut doc,
            "ingress-nginx",
            &ChartSource::Registry {
                repo: "https://kubernetes.github.io/ingress-nginx".to_string(),
                chart: "ingress-nginx".to_string(),
                version: "4.12.0".to_string(),
            },
        );

        let output = doc.to_string();
        assert!(output.contains("[charts]"));
        assert!(output.contains("ingress-nginx"));
        assert!(output.contains("4.12.0"));
    }

    #[test]
    fn add_chart_oci() {
        let (_tmp, path) = create_test_toml("");
        let mut doc: DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();

        add_chart(
            &mut doc,
            "postgresql",
            &ChartSource::Oci {
                reference: "oci://ghcr.io/org/postgresql".to_string(),
                version: "1.2.3".to_string(),
            },
        );

        let output = doc.to_string();
        assert!(output.contains("[charts]"));
        assert!(output.contains("postgresql"));
        assert!(output.contains("oci"));
        assert!(output.contains("ghcr.io/org/postgresql"));
        assert!(output.contains("1.2.3"));
    }

    #[test]
    fn remove_resource_existing() {
        let (_tmp, path) = create_test_toml(
            "[resources]\nkubernetes = { source = \"release\", version = \"1.35\" }\n",
        );
        let mut doc: DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();

        assert!(remove_resource(&mut doc, "kubernetes"));
        let output = doc.to_string();
        assert!(!output.contains("kubernetes"));
    }

    #[test]
    fn remove_resource_missing() {
        let (_tmp, path) = create_test_toml("[resources]\n");
        let mut doc: DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();

        assert!(!remove_resource(&mut doc, "nonexistent"));
    }

    #[test]
    fn remove_chart_existing() {
        let (_tmp, path) = create_test_toml(
            "[charts]\nmy-chart = { source = \"file\", path = \"./values.schema.json\" }\n",
        );
        let mut doc: DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();

        assert!(remove_chart(&mut doc, "my-chart"));
        let output = doc.to_string();
        assert!(!output.contains("my-chart"));
    }

    #[test]
    fn update_resource_version() {
        let (_tmp, path) = create_test_toml(
            "[resources]\nkubernetes = { source = \"release\", version = \"1.35\" }\n",
        );
        let mut doc: DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();

        assert!(super::update_resource_version(
            &mut doc,
            "kubernetes",
            "1.36"
        ));
        let output = doc.to_string();
        assert!(output.contains("1.36"));
        assert!(!output.contains("1.35"));
    }

    #[test]
    fn update_chart_version() {
        let (_tmp, path) = create_test_toml(
            "[charts]\ningress-nginx = { source = \"registry\", repo = \"https://example.com\", chart = \"ingress-nginx\", version = \"4.12.0\" }\n",
        );
        let mut doc: DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();

        assert!(super::update_chart_version(
            &mut doc,
            "ingress-nginx",
            "4.12.1"
        ));
        let output = doc.to_string();
        assert!(output.contains("4.12.1"));
        assert!(!output.contains("4.12.0"));
    }

    #[test]
    fn update_git_tag() {
        let (_tmp, path) = create_test_toml(
            "[resources]\ncert-manager = { source = \"git\", repo = \"https://github.com/cert-manager/cert-manager\", tag = \"v1.17.2\", path = \"deploy/crds\" }\n",
        );
        let mut doc: DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();

        assert!(super::update_resource_version(
            &mut doc,
            "cert-manager",
            "v1.18.0"
        ));
        let output = doc.to_string();
        assert!(output.contains("v1.18.0"));
        assert!(!output.contains("v1.17.2"));
    }

    #[test]
    fn preserve_comments() {
        let content = "# Project config\n\n[entries]\n# Entry aliases\ndev = \"env/dev.ts\"\n\n[resources]\n# K8s resources\nkubernetes = { source = \"release\", version = \"1.35\" }\n";
        let (_tmp, path) = create_test_toml(content);
        let mut doc: DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();

        super::update_resource_version(&mut doc, "kubernetes", "1.36");

        let output = doc.to_string();
        assert!(output.contains("# Project config"));
        assert!(output.contains("# Entry aliases"));
        assert!(output.contains("# K8s resources"));
        assert!(output.contains("dev = \"env/dev.ts\""));
    }

    #[test]
    fn round_trip() {
        let content = "[entries]\ndev = \"env/dev.ts\"\n\n[resources]\nkubernetes = { source = \"release\", version = \"1.35\" }\n";
        let (_tmp, path) = create_test_toml(content);
        let doc: DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();

        // Round-trip without changes should preserve content
        let output = doc.to_string();
        assert_eq!(output, content);
    }

    #[test]
    fn load_document_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let err = load_document(tmp.path()).unwrap_err();
        assert!(matches!(err, ConfigError::Io { .. }));
    }

    #[test]
    fn load_and_save_round_trip() {
        let content = "[entries]\ndev = \"env/dev.ts\"\n";
        let (tmp, _path) = create_test_toml(content);
        let (doc, path) = load_document(tmp.path()).unwrap();

        save_document(&doc, &path).unwrap();
        let reloaded = std::fs::read_to_string(&path).unwrap();
        assert_eq!(reloaded, content);
    }

    #[test]
    fn add_plugin_git() {
        let (_tmp, path) = create_test_toml("");
        let mut doc: DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();

        add_plugin(
            &mut doc,
            "flux",
            &PluginSource::Git {
                url: "https://github.com/nanazt/husako-plugin-flux".to_string(),
                path: None,
            },
        );

        let output = doc.to_string();
        assert!(output.contains("[plugins]"));
        assert!(output.contains("flux"));
        assert!(output.contains("git"));
        assert!(output.contains("husako-plugin-flux"));
        // No path field when None
        assert!(!output.contains("\"path\""));
    }

    #[test]
    fn add_plugin_git_with_path() {
        let (_tmp, path) = create_test_toml("");
        let mut doc: DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();

        add_plugin(
            &mut doc,
            "flux",
            &PluginSource::Git {
                url: "https://github.com/nanazt/husako".to_string(),
                path: Some("plugins/flux".to_string()),
            },
        );

        let output = doc.to_string();
        assert!(output.contains("[plugins]"));
        assert!(output.contains("flux"));
        assert!(output.contains("git"));
        assert!(output.contains("plugins/flux"));
    }

    #[test]
    fn add_plugin_path() {
        let (_tmp, path) = create_test_toml("");
        let mut doc: DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();

        add_plugin(
            &mut doc,
            "my-plugin",
            &PluginSource::Path {
                path: "./plugins/my-plugin".to_string(),
            },
        );

        let output = doc.to_string();
        assert!(output.contains("[plugins]"));
        assert!(output.contains("my-plugin"));
        assert!(output.contains("path"));
    }

    #[test]
    fn remove_plugin_existing() {
        let (_tmp, path) = create_test_toml(
            "[plugins]\nflux = { source = \"git\", url = \"https://github.com/nanazt/husako-plugin-flux\" }\n",
        );
        let mut doc: DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();

        assert!(remove_plugin(&mut doc, "flux"));
        let output = doc.to_string();
        assert!(!output.contains("flux"));
    }

    #[test]
    fn remove_plugin_missing() {
        let (_tmp, path) = create_test_toml("[plugins]\n");
        let mut doc: DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();

        assert!(!remove_plugin(&mut doc, "nonexistent"));
    }
}
