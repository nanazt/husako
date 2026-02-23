use dialoguer::{Confirm, FuzzySelect, Input, Select};
use husako_config::{ChartSource, SchemaSource};
use husako_core::AddTarget;

/// Interactively prompt the user to build an AddTarget.
pub fn prompt_add() -> Result<AddTarget, String> {
    let theme = crate::theme::husako_theme();
    let kind = Select::with_theme(&theme)
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
    let theme = crate::theme::husako_theme();
    let source_type = Select::with_theme(&theme)
        .with_prompt("Source type")
        .items(["release", "cluster", "git", "file"])
        .default(0)
        .interact()
        .map_err(|e| e.to_string())?;

    let name: String = Input::with_theme(&theme)
        .with_prompt("Name")
        .validate_with(validate_name)
        .interact_text()
        .map_err(|e| e.to_string())?;

    let source = match source_type {
        0 => {
            let version = prompt_release_version()?;
            SchemaSource::Release { version }
        }
        1 => {
            let cluster_str =
                crate::text_input::run("Cluster name", "default", |_| Ok::<(), String>(()))?
                    .ok_or_else(|| "cancelled".to_string())?;
            SchemaSource::Cluster {
                cluster: if cluster_str == "default" {
                    None
                } else {
                    Some(cluster_str)
                },
            }
        }
        2 => {
            let repo: String = Input::with_theme(&theme)
                .with_prompt("Git repository URL")
                .validate_with(validate_url)
                .interact_text()
                .map_err(|e| e.to_string())?;
            let tag: String = Input::with_theme(&theme)
                .with_prompt("Git tag")
                .validate_with(validate_non_empty("tag"))
                .interact_text()
                .map_err(|e| e.to_string())?;
            let path: String = Input::with_theme(&theme)
                .with_prompt("Path to CRDs in repository")
                .validate_with(validate_non_empty("path"))
                .interact_text()
                .map_err(|e| e.to_string())?;
            SchemaSource::Git { repo, tag, path }
        }
        3 => {
            let path: String = Input::with_theme(&theme)
                .with_prompt("Path to CRD YAML file or directory")
                .validate_with(validate_non_empty("path"))
                .interact_text()
                .map_err(|e| e.to_string())?;
            SchemaSource::File { path }
        }
        _ => unreachable!(),
    };

    Ok(AddTarget::Resource { name, source })
}

fn prompt_add_chart() -> Result<AddTarget, String> {
    let theme = crate::theme::husako_theme();
    let source_type = Select::with_theme(&theme)
        .with_prompt("Source type")
        .items(["artifacthub", "registry", "git", "file"])
        .default(0)
        .interact()
        .map_err(|e| e.to_string())?;

    let source = match source_type {
        0 => prompt_artifacthub_chart()?,
        1 => prompt_registry_chart()?,
        2 => {
            let repo: String = Input::with_theme(&theme)
                .with_prompt("Git repository URL")
                .validate_with(validate_url)
                .interact_text()
                .map_err(|e| e.to_string())?;
            let default_name = repo
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or("chart")
                .trim_end_matches(".git");
            let nv =
                crate::name_version_select::run(default_name, is_valid_name, |limit, offset| {
                    husako_core::version_check::discover_git_tags(&repo, limit, offset)
                        .map_err(|e| e.to_string())
                })?
                .ok_or_else(|| "cancelled".to_string())?;
            let name = nv.name;
            let tag = nv.version;
            let path: String = Input::with_theme(&theme)
                .with_prompt("Path to chart in repository")
                .validate_with(validate_non_empty("path"))
                .interact_text()
                .map_err(|e| e.to_string())?;
            return Ok(AddTarget::Chart {
                name,
                source: ChartSource::Git { repo, tag, path },
            });
        }
        3 => {
            let name: String = Input::with_theme(&theme)
                .with_prompt("Name")
                .validate_with(validate_name)
                .interact_text()
                .map_err(|e| e.to_string())?;
            let path: String = Input::with_theme(&theme)
                .with_prompt("Path to values.schema.json")
                .validate_with(validate_non_empty("path"))
                .interact_text()
                .map_err(|e| e.to_string())?;
            return Ok(AddTarget::Chart {
                name,
                source: ChartSource::File { path },
            });
        }
        _ => unreachable!(),
    };

    Ok(source)
}

fn prompt_registry_chart() -> Result<AddTarget, String> {
    let theme = crate::theme::husako_theme();
    let repo: String = Input::with_theme(&theme)
        .with_prompt("Repository URL")
        .validate_with(validate_url)
        .interact_text()
        .map_err(|e| e.to_string())?;
    let chart: String = Input::with_theme(&theme)
        .with_prompt("Chart name in repository")
        .validate_with(validate_non_empty("chart name"))
        .interact_text()
        .map_err(|e| e.to_string())?;

    let result = crate::name_version_select::run(&chart, is_valid_name, |limit, offset| {
        husako_core::version_check::discover_registry_versions(&repo, &chart, limit, offset)
            .map_err(|e| e.to_string())
    })?
    .ok_or_else(|| "cancelled".to_string())?;

    Ok(AddTarget::Chart {
        name: result.name,
        source: ChartSource::Registry {
            repo,
            chart,
            version: result.version,
        },
    })
}

fn prompt_artifacthub_chart() -> Result<AddTarget, String> {
    let theme = crate::theme::husako_theme();
    let method = Select::with_theme(&theme)
        .with_prompt("How to find the package?")
        .items(["Search ArtifactHub", "Enter manually"])
        .default(0)
        .interact()
        .map_err(|e| e.to_string())?;

    if method == 1 {
        return prompt_artifacthub_manual();
    }

    // Search flow
    let query: String = Input::with_theme(&theme)
        .with_prompt("Search query")
        .validate_with(validate_non_empty("query"))
        .interact_text()
        .map_err(|e| e.to_string())?;

    eprintln!("{}", crate::style::dim("Searching ArtifactHub..."));
    let result = match husako_core::version_check::search_artifacthub(&query, 0) {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "{} search failed ({e}), falling back to manual entry",
                crate::style::warning_prefix()
            );
            return prompt_artifacthub_manual();
        }
    };

    if result.packages.is_empty() {
        eprintln!(
            "{} no packages found, falling back to manual entry",
            crate::style::warning_prefix()
        );
        return prompt_artifacthub_manual();
    }

    use std::cell::{Cell, RefCell};

    let packages = RefCell::new(result.packages);
    let mut display_items: Vec<String> = format_packages(&packages.borrow());
    let mut has_more = result.has_more;
    let next_offset = Cell::new(husako_core::version_check::ARTIFACTHUB_PAGE_SIZE);

    let selection = crate::search_select::run(
        "Select a package",
        &mut display_items,
        &mut has_more,
        || {
            let offset = next_offset.get();
            let result = husako_core::version_check::search_artifacthub(&query, offset)
                .map_err(|e| e.to_string())?;
            next_offset.set(offset + husako_core::version_check::ARTIFACTHUB_PAGE_SIZE);
            let new_items = format_packages(&result.packages);
            packages.borrow_mut().extend(result.packages);
            Ok((new_items, result.has_more))
        },
    )?;

    let Some(idx) = selection else {
        return Err("selection cancelled".to_string());
    };

    let pkgs = packages.into_inner();
    let pkg = &pkgs[idx];
    let package_id = format!("{}/{}", pkg.repository.name, pkg.name);

    let result = crate::name_version_select::run(&pkg.name, is_valid_name, |limit, offset| {
        husako_core::version_check::discover_artifacthub_versions(&package_id, limit, offset)
            .map_err(|e| e.to_string())
    })?
    .ok_or_else(|| "cancelled".to_string())?;

    Ok(AddTarget::Chart {
        name: result.name,
        source: ChartSource::ArtifactHub {
            package: package_id,
            version: result.version,
        },
    })
}

fn format_packages(packages: &[husako_core::version_check::ArtifactHubPackage]) -> Vec<String> {
    packages
        .iter()
        .map(|pkg| {
            let desc = pkg.description.as_deref().unwrap_or("");
            let truncated = if desc.len() > 50 {
                format!("{}...", &desc[..50])
            } else {
                desc.to_string()
            };
            format!(
                "{}/{} ({}) \u{2014} {}",
                pkg.repository.name, pkg.name, pkg.version, truncated
            )
        })
        .collect()
}

fn prompt_artifacthub_manual() -> Result<AddTarget, String> {
    let theme = crate::theme::husako_theme();
    let package: String = Input::with_theme(&theme)
        .with_prompt("Package (e.g. bitnami/postgresql)")
        .validate_with(validate_non_empty("package"))
        .interact_text()
        .map_err(|e| e.to_string())?;

    let default_name = package.rsplit('/').next().unwrap_or(&package);
    let result =
        crate::name_version_select::run(default_name, is_valid_name, |limit, offset| {
            husako_core::version_check::discover_artifacthub_versions(&package, limit, offset)
                .map_err(|e| e.to_string())
        })?
        .ok_or_else(|| "cancelled".to_string())?;

    Ok(AddTarget::Chart {
        name: result.name,
        source: ChartSource::ArtifactHub {
            package,
            version: result.version,
        },
    })
}

fn prompt_version_select(
    fetch_label: &str,
    prompt_label: &'static str,
    fetch_fn: impl FnOnce() -> Result<Vec<String>, String>,
) -> Result<String, String> {
    let theme = crate::theme::husako_theme();
    eprintln!("{}", crate::style::dim(fetch_label));

    match fetch_fn() {
        Ok(versions) if !versions.is_empty() => {
            let mut items: Vec<String> = versions
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    if i == 0 {
                        format!("{v} (latest)")
                    } else {
                        v.clone()
                    }
                })
                .collect();
            items.push("Enter manually".to_string());

            let selection = Select::with_theme(&theme)
                .with_prompt(prompt_label)
                .items(&items)
                .default(0)
                .interact()
                .map_err(|e| e.to_string())?;

            if selection == items.len() - 1 {
                Input::with_theme(&theme)
                    .with_prompt(prompt_label)
                    .validate_with(validate_non_empty(prompt_label))
                    .interact_text()
                    .map_err(|e| e.to_string())
            } else {
                Ok(versions[selection].clone())
            }
        }
        Ok(_) | Err(_) => {
            eprintln!(
                "{} could not fetch versions, entering manually",
                crate::style::warning_prefix()
            );
            Input::with_theme(&theme)
                .with_prompt(prompt_label)
                .validate_with(validate_non_empty(prompt_label))
                .interact_text()
                .map_err(|e| e.to_string())
        }
    }
}

fn prompt_release_version() -> Result<String, String> {
    prompt_version_select(
        "Fetching Kubernetes versions...",
        "Kubernetes version (e.g. 1.35)",
        || husako_core::version_check::discover_recent_releases(5, 0).map_err(|e| e.to_string()),
    )
}

/// Interactively prompt which dependency to remove.
pub fn prompt_remove(deps: &[(String, &'static str, &'static str)]) -> Result<String, String> {
    if deps.is_empty() {
        return Err("no dependencies configured".to_string());
    }

    let theme = crate::theme::husako_theme();
    let items: Vec<String> = deps
        .iter()
        .map(|(name, kind, source)| format!("{name} ({kind}, {source})"))
        .collect();

    let selection = if items.len() > 5 {
        FuzzySelect::with_theme(&theme)
            .with_prompt("Which dependency to remove?")
            .items(&items)
            .default(0)
            .interact()
            .map_err(|e: dialoguer::Error| e.to_string())?
    } else {
        Select::with_theme(&theme)
            .with_prompt("Which dependency to remove?")
            .items(&items)
            .default(0)
            .interact()
            .map_err(|e| e.to_string())?
    };

    Ok(deps[selection].0.clone())
}

/// Interactively prompt what to clean.
pub fn prompt_clean() -> Result<(bool, bool), String> {
    let theme = crate::theme::husako_theme();
    let selection = Select::with_theme(&theme)
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

/// Ask for user confirmation. Returns true if user confirms.
pub fn confirm(prompt: &str) -> Result<bool, String> {
    let theme = crate::theme::husako_theme();
    Confirm::with_theme(&theme)
        .with_prompt(prompt)
        .default(true)
        .interact()
        .map_err(|e| e.to_string())
}

// --- Input validation helpers ---

fn is_valid_name(input: &str) -> Result<(), String> {
    if input.is_empty() {
        return Err("name cannot be empty".to_string());
    }
    if !input
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("must contain only lowercase letters, digits, and hyphens".to_string());
    }
    Ok(())
}

#[allow(clippy::ptr_arg)] // dialoguer validate_with requires Fn(&String)
fn validate_name(input: &String) -> Result<(), String> {
    is_valid_name(input)
}

#[allow(clippy::ptr_arg)] // dialoguer validate_with requires Fn(&String)
fn validate_url(input: &String) -> Result<(), String> {
    if !input.starts_with("https://") && !input.starts_with("http://") {
        return Err("must start with https:// or http://".to_string());
    }
    Ok(())
}

fn validate_non_empty(field: &'static str) -> impl Fn(&String) -> Result<(), String> {
    move |input: &String| {
        if input.trim().is_empty() {
            Err(format!("{field} cannot be empty"))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_valid_name_works_with_str() {
        assert!(is_valid_name("my-chart").is_ok());
        assert!(is_valid_name("postgresql").is_ok());
        assert!(is_valid_name("").is_err());
        assert!(is_valid_name("UPPER").is_err());
        assert!(is_valid_name("has space").is_err());
    }

    #[test]
    fn validate_name_accepts_valid() {
        assert!(validate_name(&"my-chart".to_string()).is_ok());
        assert!(validate_name(&"postgresql".to_string()).is_ok());
        assert!(validate_name(&"k8s-1-35".to_string()).is_ok());
    }

    #[test]
    fn validate_name_rejects_invalid() {
        assert!(validate_name(&String::new()).is_err());
        assert!(validate_name(&"My Chart!".to_string()).is_err());
        assert!(validate_name(&"UPPER".to_string()).is_err());
        assert!(validate_name(&"has space".to_string()).is_err());
    }

    #[test]
    fn validate_url_accepts_valid() {
        assert!(validate_url(&"https://charts.example.com".to_string()).is_ok());
        assert!(validate_url(&"http://localhost:8080".to_string()).is_ok());
    }

    #[test]
    fn validate_url_rejects_invalid() {
        assert!(validate_url(&"ftp://example.com".to_string()).is_err());
        assert!(validate_url(&"example.com".to_string()).is_err());
    }

    #[test]
    fn validate_non_empty_works() {
        let v = validate_non_empty("version");
        assert!(v(&"1.0.0".to_string()).is_ok());
        assert!(v(&String::new()).is_err());
        assert!(v(&"   ".to_string()).is_err());
    }
}
