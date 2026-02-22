use dialoguer::{Input, Select};
use husako_config::{ChartSource, SchemaSource};
use husako_core::AddTarget;

/// Interactively prompt the user to build an AddTarget.
pub fn prompt_add() -> Result<AddTarget, String> {
    let kind = Select::new()
        .with_prompt("Dependency type")
        .items(["Resource", "Chart"])
        .default(0)
        .interact()
        .map_err(|e| e.to_string())?;

    match kind {
        0 => prompt_add_resource(),
        1 => prompt_add_chart(),
        _ => unreachable!(),
    }
}

fn prompt_add_resource() -> Result<AddTarget, String> {
    let source_type = Select::new()
        .with_prompt("Source type")
        .items(["release", "cluster", "git", "file"])
        .default(0)
        .interact()
        .map_err(|e| e.to_string())?;

    let name: String = Input::new()
        .with_prompt("Name")
        .interact_text()
        .map_err(|e| e.to_string())?;

    let source = match source_type {
        0 => {
            let version: String = Input::new()
                .with_prompt("Kubernetes version (e.g. 1.35)")
                .interact_text()
                .map_err(|e| e.to_string())?;
            SchemaSource::Release { version }
        }
        1 => {
            let cluster: String = Input::new()
                .with_prompt("Cluster name (leave empty for default)")
                .allow_empty(true)
                .interact_text()
                .map_err(|e| e.to_string())?;
            SchemaSource::Cluster {
                cluster: if cluster.is_empty() {
                    None
                } else {
                    Some(cluster)
                },
            }
        }
        2 => {
            let repo: String = Input::new()
                .with_prompt("Git repository URL")
                .interact_text()
                .map_err(|e| e.to_string())?;
            let tag: String = Input::new()
                .with_prompt("Git tag")
                .interact_text()
                .map_err(|e| e.to_string())?;
            let path: String = Input::new()
                .with_prompt("Path to CRDs in repository")
                .interact_text()
                .map_err(|e| e.to_string())?;
            SchemaSource::Git { repo, tag, path }
        }
        3 => {
            let path: String = Input::new()
                .with_prompt("Path to CRD YAML file or directory")
                .interact_text()
                .map_err(|e| e.to_string())?;
            SchemaSource::File { path }
        }
        _ => unreachable!(),
    };

    Ok(AddTarget::Resource { name, source })
}

fn prompt_add_chart() -> Result<AddTarget, String> {
    let source_type = Select::new()
        .with_prompt("Source type")
        .items(["registry", "artifacthub", "git", "file"])
        .default(0)
        .interact()
        .map_err(|e| e.to_string())?;

    let name: String = Input::new()
        .with_prompt("Name")
        .interact_text()
        .map_err(|e| e.to_string())?;

    let source = match source_type {
        0 => {
            let repo: String = Input::new()
                .with_prompt("Repository URL")
                .interact_text()
                .map_err(|e| e.to_string())?;
            let chart: String = Input::new()
                .with_prompt("Chart name in repository")
                .interact_text()
                .map_err(|e| e.to_string())?;
            let version: String = Input::new()
                .with_prompt("Version")
                .interact_text()
                .map_err(|e| e.to_string())?;
            ChartSource::Registry {
                repo,
                chart,
                version,
            }
        }
        1 => {
            let package: String = Input::new()
                .with_prompt("Package (e.g. bitnami/postgresql)")
                .interact_text()
                .map_err(|e| e.to_string())?;
            let version: String = Input::new()
                .with_prompt("Version")
                .interact_text()
                .map_err(|e| e.to_string())?;
            ChartSource::ArtifactHub { package, version }
        }
        2 => {
            let repo: String = Input::new()
                .with_prompt("Git repository URL")
                .interact_text()
                .map_err(|e| e.to_string())?;
            let tag: String = Input::new()
                .with_prompt("Git tag")
                .interact_text()
                .map_err(|e| e.to_string())?;
            let path: String = Input::new()
                .with_prompt("Path to chart in repository")
                .interact_text()
                .map_err(|e| e.to_string())?;
            ChartSource::Git { repo, tag, path }
        }
        3 => {
            let path: String = Input::new()
                .with_prompt("Path to values.schema.json")
                .interact_text()
                .map_err(|e| e.to_string())?;
            ChartSource::File { path }
        }
        _ => unreachable!(),
    };

    Ok(AddTarget::Chart { name, source })
}

/// Interactively prompt which dependency to remove.
pub fn prompt_remove(deps: &[(String, &'static str, &'static str)]) -> Result<String, String> {
    if deps.is_empty() {
        return Err("no dependencies configured".to_string());
    }

    let items: Vec<String> = deps
        .iter()
        .map(|(name, kind, source)| format!("{name} ({kind}, {source})"))
        .collect();

    let selection = Select::new()
        .with_prompt("Which dependency to remove?")
        .items(&items)
        .default(0)
        .interact()
        .map_err(|e| e.to_string())?;

    Ok(deps[selection].0.clone())
}

/// Interactively prompt what to clean.
pub fn prompt_clean() -> Result<(bool, bool), String> {
    let selection = Select::new()
        .with_prompt("What do you want to clean?")
        .items(["Cache", "Types", "Both"])
        .default(2)
        .interact()
        .map_err(|e| e.to_string())?;

    Ok(match selection {
        0 => (true, false),
        1 => (false, true),
        2 => (true, true),
        _ => unreachable!(),
    })
}
