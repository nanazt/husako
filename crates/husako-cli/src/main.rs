mod interactive;
mod name_version_select;
mod progress;
mod search_select;
mod style;
mod text_input;
mod theme;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use husako_core::{GenerateOptions, HusakoError, RenderOptions, ScaffoldOptions, TemplateName};
use husako_runtime_qjs::RuntimeError;

use crate::progress::IndicatifReporter;

const DEFAULT_K8S_VERSION: &str = "1.35";

#[derive(Parser)]
#[command(name = "husako", version)]
struct Cli {
    /// Skip confirmation prompts
    #[arg(long, short = 'y', global = true)]
    yes: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Render TypeScript to Kubernetes YAML
    Render {
        /// Path to the TypeScript entry file, or an entry alias from husako.toml
        file: String,

        /// Allow imports outside the project root
        #[arg(long)]
        allow_outside_root: bool,

        /// Execution timeout in milliseconds
        #[arg(long)]
        timeout_ms: Option<u64>,

        /// Maximum heap memory in megabytes
        #[arg(long)]
        max_heap_mb: Option<usize>,

        /// Print diagnostic traces to stderr
        #[arg(long)]
        verbose: bool,
    },

    /// Generate type definitions and tsconfig.json
    #[command(alias = "gen")]
    Generate {
        /// Kubernetes API server URL (e.g. https://localhost:6443)
        #[arg(long)]
        api_server: Option<String>,

        /// Local directory with pre-fetched OpenAPI spec JSON files
        #[arg(long)]
        spec_dir: Option<PathBuf>,

        /// Skip Kubernetes type generation (only write husako.d.ts + tsconfig)
        #[arg(long)]
        skip_k8s: bool,
    },

    /// Create a new project from a template
    New {
        /// Directory to create
        directory: PathBuf,

        /// Template to use (simple, project, multi-env)
        #[arg(short, long, default_value = "simple")]
        template: TemplateName,
    },

    /// Initialize husako in the current directory
    Init {
        /// Template to use (simple, project, multi-env)
        #[arg(long, default_value = "simple")]
        template: TemplateName,
    },

    /// Clean cache and/or generated types
    Clean {
        /// Remove only the cache directory (.husako/cache/)
        #[arg(long)]
        cache: bool,

        /// Remove only the types directory (.husako/types/)
        #[arg(long)]
        types: bool,

        /// Remove both cache and types
        #[arg(long)]
        all: bool,
    },

    /// List configured dependencies
    #[command(alias = "ls")]
    List {
        /// Show only resources
        #[arg(long)]
        resources: bool,

        /// Show only charts
        #[arg(long)]
        charts: bool,
    },

    /// Add a resource or chart dependency
    Add {
        /// Dependency name
        name: Option<String>,

        /// Add as a resource dependency
        #[arg(long, group = "kind")]
        resource: bool,

        /// Add as a chart dependency
        #[arg(long, group = "kind")]
        chart: bool,

        /// Source type (release, cluster, git, file, registry, artifacthub, oci)
        #[arg(long)]
        source: Option<String>,

        /// Version
        #[arg(long)]
        version: Option<String>,

        /// Repository URL
        #[arg(long)]
        repo: Option<String>,

        /// Git tag
        #[arg(long)]
        tag: Option<String>,

        /// File or directory path
        #[arg(long)]
        path: Option<String>,

        /// Chart name in the repository
        #[arg(long)]
        chart_name: Option<String>,

        /// ArtifactHub package name (e.g. bitnami/postgresql)
        #[arg(long)]
        package: Option<String>,

        /// OCI reference (e.g. oci://ghcr.io/org/chart-name)
        #[arg(long)]
        reference: Option<String>,
    },

    /// Remove a resource or chart dependency
    #[command(alias = "rm")]
    Remove {
        /// Dependency name to remove
        name: Option<String>,
    },

    /// Check for outdated dependencies
    Outdated,

    /// Update dependencies to latest versions
    Update {
        /// Only update this dependency
        name: Option<String>,

        /// Only update resources
        #[arg(long)]
        resources_only: bool,

        /// Only update charts
        #[arg(long)]
        charts_only: bool,

        /// Show what would be updated without applying
        #[arg(long)]
        dry_run: bool,
    },

    /// Show project summary or dependency details
    Info {
        /// Dependency name (omit for project summary)
        name: Option<String>,
    },

    /// Check project health and diagnose issues
    Debug,

    /// Validate TypeScript without rendering output
    Validate {
        /// TypeScript entry file or alias
        file: String,
    },

    /// Manage plugins
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
}

#[derive(clap::Subcommand)]
enum PluginAction {
    /// Add a plugin
    Add {
        /// Plugin name
        name: String,

        /// Git repository URL
        #[arg(long)]
        url: Option<String>,

        /// Local directory path
        #[arg(long)]
        path: Option<String>,
    },

    /// Remove a plugin
    Remove {
        /// Plugin name
        name: String,
    },

    /// List installed plugins
    List,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Render {
            file,
            allow_outside_root,
            timeout_ms,
            max_heap_mb,
            verbose,
        } => {
            let project_root = cwd();

            let resolved = match resolve_entry(&file, &project_root) {
                Ok(p) => p,
                Err(msg) => {
                    eprintln!("{} {msg}", style::error_prefix());
                    return ExitCode::from(2);
                }
            };

            let source = match std::fs::read_to_string(&resolved) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!(
                        "{} could not read {}: {e}",
                        style::error_prefix(),
                        resolved.display()
                    );
                    return ExitCode::from(1);
                }
            };

            let abs_file = match resolved.canonicalize() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!(
                        "{} could not resolve {}: {e}",
                        style::error_prefix(),
                        resolved.display()
                    );
                    return ExitCode::from(1);
                }
            };

            let schema_store = husako_core::load_schema_store(&project_root);

            let filename = abs_file.to_string_lossy();
            let options = RenderOptions {
                project_root,
                allow_outside_root,
                schema_store,
                timeout_ms,
                max_heap_mb,
                verbose,
            };

            match husako_core::render(&source, &filename, &options) {
                Ok(yaml) => {
                    print!("{yaml}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{} {e}", style::error_prefix());
                    ExitCode::from(exit_code(&e))
                }
            }
        }
        Commands::Generate {
            api_server,
            spec_dir,
            skip_k8s,
        } => {
            let project_root = cwd();
            let progress = IndicatifReporter::new();

            let openapi = if skip_k8s {
                None
            } else if let Some(dir) = spec_dir {
                Some(husako_openapi::FetchOptions {
                    source: husako_openapi::OpenApiSource::Directory(dir),
                    cache_dir: project_root.join(".husako/cache"),
                    offline: true,
                })
            } else {
                api_server.map(|url| husako_openapi::FetchOptions {
                    source: husako_openapi::OpenApiSource::Url {
                        base_url: url,
                        bearer_token: None,
                    },
                    cache_dir: project_root.join(".husako/cache"),
                    offline: false,
                })
            };

            let config = match husako_config::load(&project_root) {
                Ok(cfg) => cfg,
                Err(e) => {
                    eprintln!("{} {e}", style::error_prefix());
                    return ExitCode::from(2);
                }
            };

            let options = GenerateOptions {
                project_root,
                openapi,
                skip_k8s,
                config,
            };

            match husako_core::generate(&options, &progress) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{} {e}", style::error_prefix());
                    ExitCode::from(exit_code(&e))
                }
            }
        }
        Commands::New {
            directory,
            template,
        } => {
            let k8s_version = match select_k8s_version() {
                Ok(v) => v,
                Err(None) => return ExitCode::SUCCESS, // Escape pressed
                Err(Some(msg)) => {
                    eprintln!("{} {msg}", style::error_prefix());
                    return ExitCode::from(1);
                }
            };

            let options = ScaffoldOptions {
                directory: directory.clone(),
                template,
                k8s_version,
            };

            match husako_core::scaffold(&options) {
                Ok(()) => {
                    eprintln!(
                        "{} Created '{}' project in {}",
                        style::check_mark(),
                        template,
                        directory.display()
                    );
                    eprintln!();
                    eprintln!("Next steps:");
                    eprintln!("  cd {}", directory.display());
                    eprintln!("  husako generate");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{} {e}", style::error_prefix());
                    ExitCode::from(exit_code(&e))
                }
            }
        }

        // --- M16 ---
        Commands::Init { template } => {
            let project_root = cwd();

            let k8s_version = match select_k8s_version() {
                Ok(v) => v,
                Err(None) => return ExitCode::SUCCESS, // Escape pressed
                Err(Some(msg)) => {
                    eprintln!("{} {msg}", style::error_prefix());
                    return ExitCode::from(1);
                }
            };

            let options = husako_core::InitOptions {
                directory: project_root,
                template,
                k8s_version,
            };

            match husako_core::init(&options) {
                Ok(()) => {
                    eprintln!(
                        "{} Created '{template}' project in current directory",
                        style::check_mark()
                    );
                    eprintln!();
                    eprintln!("Next steps:");
                    eprintln!("  husako generate");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{} {e}", style::error_prefix());
                    ExitCode::from(exit_code(&e))
                }
            }
        }
        Commands::Clean { cache, types, all } => {
            let project_root = cwd();

            let (do_cache, do_types) = if all {
                (true, true)
            } else if cache || types {
                (cache, types)
            } else {
                // Interactive mode
                match interactive::prompt_clean() {
                    Ok(result) => result,
                    Err(e) => {
                        eprintln!("{} {e}", style::error_prefix());
                        return ExitCode::from(1);
                    }
                }
            };

            if !cli.yes {
                let targets = match (do_cache, do_types) {
                    (true, true) => "cache and types",
                    (true, false) => "cache",
                    (false, true) => "types",
                    _ => unreachable!(),
                };
                match interactive::confirm(&format!("Remove {targets}?")) {
                    Ok(true) => {}
                    Ok(false) => return ExitCode::SUCCESS,
                    Err(e) => {
                        eprintln!("{} {e}", style::error_prefix());
                        return ExitCode::from(1);
                    }
                }
            }

            let options = husako_core::CleanOptions {
                project_root,
                cache: do_cache,
                types: do_types,
            };

            match husako_core::clean(&options) {
                Ok(result) => {
                    if result.cache_removed {
                        eprintln!(
                            "{} Removed .husako/cache/ ({})",
                            style::check_mark(),
                            format_size(result.cache_size)
                        );
                    }
                    if result.types_removed {
                        eprintln!(
                            "{} Removed .husako/types/ ({})",
                            style::check_mark(),
                            format_size(result.types_size)
                        );
                    }
                    if !result.cache_removed && !result.types_removed {
                        eprintln!("Nothing to clean");
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{} {e}", style::error_prefix());
                    ExitCode::from(exit_code(&e))
                }
            }
        }
        Commands::List { resources, charts } => {
            let project_root = cwd();

            match husako_core::list_dependencies(&project_root) {
                Ok(deps) => {
                    let show_resources = !charts || resources;
                    let show_charts = !resources || charts;

                    if show_resources && !deps.resources.is_empty() {
                        eprintln!("{}", style::bold("Resources:"));
                        for dep in &deps.resources {
                            eprintln!(
                                "  {:<16} {:<12} {:<10}{}",
                                style::dep_name(&dep.name),
                                dep.source_type,
                                dep.version.as_deref().unwrap_or("-"),
                                if dep.details.is_empty() {
                                    String::new()
                                } else {
                                    format!("  {}", style::dim(&dep.details))
                                }
                            );
                        }
                    }

                    if show_charts && !deps.charts.is_empty() {
                        if show_resources && !deps.resources.is_empty() {
                            eprintln!();
                        }
                        eprintln!("{}", style::bold("Charts:"));
                        for dep in &deps.charts {
                            eprintln!(
                                "  {:<16} {:<12} {:<10}{}",
                                style::dep_name(&dep.name),
                                dep.source_type,
                                dep.version.as_deref().unwrap_or("-"),
                                if dep.details.is_empty() {
                                    String::new()
                                } else {
                                    format!("  {}", style::dim(&dep.details))
                                }
                            );
                        }
                    }

                    if deps.resources.is_empty() && deps.charts.is_empty() {
                        eprintln!("No dependencies configured");
                    }

                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{} {e}", style::error_prefix());
                    ExitCode::from(exit_code(&e))
                }
            }
        }

        // --- M17 ---
        Commands::Add {
            name,
            resource: _,
            chart,
            source,
            version,
            repo,
            tag,
            path,
            chart_name,
            package,
            reference,
        } => {
            let project_root = cwd();

            let target = if let Some(src) = source {
                // Non-interactive mode
                if chart {
                    // For charts, derive name from chart_name, package, or reference if not provided
                    let dep_name = name
                        .or_else(|| chart_name.clone())
                        .or_else(|| {
                            package
                                .as_deref()
                                .and_then(|p| p.rsplit('/').next())
                                .map(String::from)
                        })
                        .or_else(|| {
                            reference.as_deref().map(|r| {
                                let without_scheme = r.strip_prefix("oci://").unwrap_or(r);
                                let last =
                                    without_scheme.rsplit('/').next().unwrap_or(without_scheme);
                                last.split(':').next().unwrap_or(last).to_string()
                            })
                        });
                    let Some(dep_name) = dep_name else {
                        eprintln!(
                            "{} name is required (provide as positional arg, or use --chart-name / --package / --reference)",
                            style::error_prefix()
                        );
                        return ExitCode::from(2);
                    };
                    build_chart_target(
                        dep_name, src, version, repo, tag, path, chart_name, package, reference,
                    )
                } else {
                    let Some(dep_name) = name else {
                        eprintln!(
                            "{} name is required for resource dependencies",
                            style::error_prefix()
                        );
                        return ExitCode::from(2);
                    };
                    build_resource_target(dep_name, src, version, repo, tag, path)
                }
            } else {
                // Interactive mode
                interactive::prompt_add()
            };

            match target {
                Ok(target) => match husako_core::add_dependency(&project_root, &target) {
                    Ok(()) => {
                        let (dep_name, section) = match &target {
                            husako_core::AddTarget::Resource { name, .. } => (name, "resources"),
                            husako_core::AddTarget::Chart { name, .. } => (name, "charts"),
                        };
                        eprintln!(
                            "{} Added {} to [{section}]",
                            style::check_mark(),
                            style::dep_name(dep_name)
                        );
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("{} {e}", style::error_prefix());
                        ExitCode::from(exit_code(&e))
                    }
                },
                Err(e) => {
                    eprintln!("{} {e}", style::error_prefix());
                    ExitCode::from(1)
                }
            }
        }
        Commands::Remove { name } => {
            let project_root = cwd();

            let (dep_name, from_cli) = if let Some(n) = name {
                (n, true)
            } else {
                // Interactive mode: list deps and let user choose
                match husako_core::list_dependencies(&project_root) {
                    Ok(deps) => {
                        let mut items: Vec<(String, &'static str, &'static str)> = Vec::new();
                        for dep in &deps.resources {
                            items.push((dep.name.clone(), "resource", dep.source_type));
                        }
                        for dep in &deps.charts {
                            items.push((dep.name.clone(), "chart", dep.source_type));
                        }

                        match interactive::prompt_remove(&items) {
                            Ok(n) => (n, false),
                            Err(e) => {
                                eprintln!("{} {e}", style::error_prefix());
                                return ExitCode::from(1);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("{} {e}", style::error_prefix());
                        return ExitCode::from(exit_code(&e));
                    }
                }
            };

            // Confirm removal only in CLI mode (not interactive, user already chose)
            if from_cli && !cli.yes {
                match interactive::confirm(&format!("Remove '{dep_name}'?")) {
                    Ok(true) => {}
                    Ok(false) => return ExitCode::SUCCESS,
                    Err(e) => {
                        eprintln!("{} {e}", style::error_prefix());
                        return ExitCode::from(1);
                    }
                }
            }

            match husako_core::remove_dependency(&project_root, &dep_name) {
                Ok(result) => {
                    eprintln!(
                        "{} Removed {} from [{}]",
                        style::check_mark(),
                        style::dep_name(&result.name),
                        result.section
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{} {e}", style::error_prefix());
                    ExitCode::from(exit_code(&e))
                }
            }
        }

        // --- M18 ---
        Commands::Outdated => {
            let project_root = cwd();
            let progress = IndicatifReporter::new();

            match husako_core::check_outdated(&project_root, &progress) {
                Ok(entries) => {
                    if entries.is_empty() {
                        eprintln!("No versioned dependencies found");
                        return ExitCode::SUCCESS;
                    }

                    eprintln!(
                        "{:<16} {:<10} {:<12} {:<10} {:<10}",
                        "Name", "Kind", "Source", "Current", "Latest"
                    );
                    for entry in &entries {
                        let latest = entry.latest.as_deref().unwrap_or("?");
                        let mark = if entry.up_to_date {
                            format!(" {}", style::check_mark())
                        } else {
                            String::new()
                        };
                        eprintln!(
                            "{:<16} {:<10} {:<12} {:<10} {:<10}{}",
                            style::dep_name(&entry.name),
                            entry.kind,
                            entry.source_type,
                            entry.current,
                            latest,
                            mark,
                        );
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{} {e}", style::error_prefix());
                    ExitCode::from(exit_code(&e))
                }
            }
        }

        // --- M19 ---
        Commands::Update {
            name,
            resources_only,
            charts_only,
            dry_run,
        } => {
            let project_root = cwd();
            let progress = IndicatifReporter::new();

            let options = husako_core::UpdateOptions {
                project_root,
                name,
                resources_only,
                charts_only,
                dry_run,
            };

            match husako_core::update_dependencies(&options, &progress) {
                Ok(result) => {
                    for entry in &result.updated {
                        let prefix = if dry_run { "Would update" } else { "Updated" };
                        eprintln!(
                            "{} {prefix} {}: {} {} {} ({})",
                            style::check_mark(),
                            style::dep_name(&entry.name),
                            entry.old_version,
                            style::arrow_mark(),
                            entry.new_version,
                            entry.kind
                        );
                    }
                    for name in &result.skipped {
                        eprintln!(
                            "{} {}: up to date",
                            style::check_mark(),
                            style::dep_name(name)
                        );
                    }
                    for (name, err) in &result.failed {
                        eprintln!("{} {}: {err}", style::cross_mark(), style::dep_name(name));
                    }
                    if result.updated.is_empty()
                        && result.skipped.is_empty()
                        && result.failed.is_empty()
                    {
                        eprintln!("No versioned dependencies found");
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{} {e}", style::error_prefix());
                    ExitCode::from(exit_code(&e))
                }
            }
        }

        // --- M20 ---
        Commands::Info { name } => {
            let project_root = cwd();

            if let Some(dep_name) = name {
                match husako_core::dependency_detail(&project_root, &dep_name) {
                    Ok(detail) => {
                        eprintln!(
                            "{} ({})",
                            style::dep_name(&detail.info.name),
                            detail.info.source_type
                        );
                        if let Some(ref v) = detail.info.version {
                            eprintln!("  Version:    {v}");
                        }
                        if !detail.info.details.is_empty() {
                            eprintln!("  Details:    {}", detail.info.details);
                        }
                        if let Some(ref p) = detail.cache_path {
                            eprintln!(
                                "  Cache:      {} ({})",
                                p.display(),
                                format_size(detail.cache_size)
                            );
                        }
                        if !detail.type_files.is_empty() {
                            eprintln!("  Type files: {}", detail.type_files.len());
                        }
                        if let Some((total, top)) = detail.schema_property_count {
                            eprintln!("  Values schema: {total} properties ({top} top-level)");
                        }
                        if !detail.group_versions.is_empty() {
                            eprintln!("  Group-Versions ({}):", detail.group_versions.len());
                            for (gv, _) in &detail.group_versions {
                                eprintln!("    {gv}");
                            }
                        }
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("{} {e}", style::error_prefix());
                        ExitCode::from(exit_code(&e))
                    }
                }
            } else {
                match husako_core::project_summary(&project_root) {
                    Ok(summary) => {
                        eprintln!("Project: {}", summary.project_root.display());
                        eprintln!(
                            "Config:  husako.toml ({})",
                            if summary.config_valid {
                                "valid"
                            } else {
                                "invalid"
                            }
                        );
                        eprintln!();

                        if !summary.resources.is_empty() {
                            eprintln!(
                                "{}",
                                style::bold(&format!("Resources ({}):", summary.resources.len()))
                            );
                            for dep in &summary.resources {
                                eprintln!(
                                    "  {:<16} {:<12} {}",
                                    style::dep_name(&dep.name),
                                    dep.source_type,
                                    dep.version.as_deref().unwrap_or("-")
                                );
                            }
                            eprintln!();
                        }

                        if !summary.charts.is_empty() {
                            eprintln!(
                                "{}",
                                style::bold(&format!("Charts ({}):", summary.charts.len()))
                            );
                            for dep in &summary.charts {
                                eprintln!(
                                    "  {:<16} {:<12} {}",
                                    style::dep_name(&dep.name),
                                    dep.source_type,
                                    dep.version.as_deref().unwrap_or("-")
                                );
                            }
                            eprintln!();
                        }

                        eprintln!(
                            "Cache:   .husako/cache/ ({})",
                            format_size(summary.cache_size)
                        );
                        eprintln!(
                            "Types:   .husako/types/ ({} files, {})",
                            summary.type_file_count,
                            format_size(summary.types_size)
                        );

                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("{} {e}", style::error_prefix());
                        ExitCode::from(exit_code(&e))
                    }
                }
            }
        }
        Commands::Debug => {
            let project_root = cwd();

            match husako_core::debug_project(&project_root) {
                Ok(report) => {
                    match report.config_ok {
                        Some(true) => {
                            eprintln!("{} husako.toml found and valid", style::check_mark())
                        }
                        Some(false) => {
                            eprintln!("{} husako.toml has errors", style::cross_mark())
                        }
                        None => eprintln!("{} husako.toml not found", style::cross_mark()),
                    }

                    if report.types_exist {
                        eprintln!(
                            "{} .husako/types/ exists ({} type files)",
                            style::check_mark(),
                            report.type_file_count
                        );
                    } else {
                        eprintln!("{} .husako/types/ directory not found", style::cross_mark());
                    }

                    if report.tsconfig_ok {
                        if report.tsconfig_has_paths {
                            eprintln!(
                                "{} tsconfig.json has husako path mappings",
                                style::check_mark()
                            );
                        } else {
                            eprintln!(
                                "{} tsconfig.json is missing husako path mappings",
                                style::cross_mark()
                            );
                        }
                    } else {
                        eprintln!("{} tsconfig.json not found or invalid", style::cross_mark());
                    }

                    if report.stale {
                        eprintln!(
                            "{} Types may be stale (husako.toml newer than .husako/types/)",
                            style::cross_mark()
                        );
                    }

                    if report.cache_size > 0 {
                        eprintln!(
                            "{} .husako/cache/ exists ({})",
                            style::check_mark(),
                            format_size(report.cache_size)
                        );
                    }

                    for suggestion in &report.suggestions {
                        eprintln!("  {} {suggestion}", style::arrow_mark());
                    }

                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{} {e}", style::error_prefix());
                    ExitCode::from(exit_code(&e))
                }
            }
        }
        Commands::Plugin { action } => {
            let project_root = cwd();

            match action {
                PluginAction::Add { name, url, path } => {
                    let source = if let Some(url) = url {
                        husako_config::PluginSource::Git { url, path: None }
                    } else if let Some(path) = path {
                        husako_config::PluginSource::Path { path }
                    } else {
                        eprintln!(
                            "{} specify --url or --path for the plugin source",
                            style::error_prefix()
                        );
                        return ExitCode::from(2);
                    };

                    let (mut doc, doc_path) =
                        match husako_config::edit::load_document(&project_root) {
                            Ok(v) => v,
                            Err(e) => {
                                eprintln!("{} {e}", style::error_prefix());
                                return ExitCode::from(2);
                            }
                        };

                    husako_config::edit::add_plugin(&mut doc, &name, &source);

                    if let Err(e) = husako_config::edit::save_document(&doc, &doc_path) {
                        eprintln!("{} {e}", style::error_prefix());
                        return ExitCode::from(1);
                    }

                    eprintln!(
                        "{} Added plugin {} to [plugins]",
                        style::check_mark(),
                        style::dep_name(&name)
                    );
                    eprintln!();
                    eprintln!("Run 'husako generate' to install the plugin and generate types.");
                    ExitCode::SUCCESS
                }
                PluginAction::Remove { name } => {
                    // Remove from config
                    let (mut doc, doc_path) =
                        match husako_config::edit::load_document(&project_root) {
                            Ok(v) => v,
                            Err(e) => {
                                eprintln!("{} {e}", style::error_prefix());
                                return ExitCode::from(2);
                            }
                        };

                    let removed_from_config = husako_config::edit::remove_plugin(&mut doc, &name);

                    if removed_from_config
                        && let Err(e) = husako_config::edit::save_document(&doc, &doc_path)
                    {
                        eprintln!("{} {e}", style::error_prefix());
                        return ExitCode::from(1);
                    }

                    // Remove installed files
                    let removed_files =
                        match husako_core::plugin::remove_plugin(&project_root, &name) {
                            Ok(r) => r,
                            Err(e) => {
                                eprintln!("{} {e}", style::error_prefix());
                                return ExitCode::from(1);
                            }
                        };

                    if removed_from_config || removed_files {
                        eprintln!(
                            "{} Removed plugin {}",
                            style::check_mark(),
                            style::dep_name(&name)
                        );
                    } else {
                        eprintln!("{} Plugin '{}' not found", style::cross_mark(), name);
                        return ExitCode::from(1);
                    }

                    ExitCode::SUCCESS
                }
                PluginAction::List => {
                    let plugins = husako_core::plugin::list_plugins(&project_root);

                    if plugins.is_empty() {
                        eprintln!("No plugins installed");
                    } else {
                        eprintln!("{}", style::bold("Plugins:"));
                        for p in &plugins {
                            let desc = p.manifest.plugin.description.as_deref().unwrap_or("");
                            eprintln!(
                                "  {:<16} {:<10} {} modules{}",
                                style::dep_name(&p.name),
                                p.manifest.plugin.version,
                                p.manifest.modules.len(),
                                if desc.is_empty() {
                                    String::new()
                                } else {
                                    format!("  {}", style::dim(desc))
                                }
                            );
                        }
                    }

                    ExitCode::SUCCESS
                }
            }
        }
        Commands::Validate { file } => {
            let project_root = cwd();

            let resolved = match resolve_entry(&file, &project_root) {
                Ok(p) => p,
                Err(msg) => {
                    eprintln!("{} {msg}", style::error_prefix());
                    return ExitCode::from(2);
                }
            };

            let source = match std::fs::read_to_string(&resolved) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!(
                        "{} could not read {}: {e}",
                        style::error_prefix(),
                        resolved.display()
                    );
                    return ExitCode::from(1);
                }
            };

            let abs_file = match resolved.canonicalize() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!(
                        "{} could not resolve {}: {e}",
                        style::error_prefix(),
                        resolved.display()
                    );
                    return ExitCode::from(1);
                }
            };

            let schema_store = husako_core::load_schema_store(&project_root);

            let filename = abs_file.to_string_lossy();
            let options = RenderOptions {
                project_root,
                allow_outside_root: false,
                schema_store,
                timeout_ms: None,
                max_heap_mb: None,
                verbose: false,
            };

            match husako_core::validate_file(&source, &filename, &options) {
                Ok(result) => {
                    eprintln!("{} {} compiles successfully", style::check_mark(), file);
                    eprintln!(
                        "{} husako.build() called with {} resources",
                        style::check_mark(),
                        result.resource_count
                    );
                    eprintln!(
                        "{} All resources pass schema validation",
                        style::check_mark()
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{} {e}", style::error_prefix());
                    ExitCode::from(exit_code(&e))
                }
            }
        }
    }
}

fn cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Resolve a file argument to a path. Tries as a direct file path first,
/// then as an entry alias from `husako.toml`.
fn resolve_entry(file_arg: &str, project_root: &std::path::Path) -> Result<PathBuf, String> {
    // 1. Try as direct file path
    let as_path = project_root.join(file_arg);
    if as_path.exists() {
        return Ok(as_path);
    }

    // Also check if it was given as an absolute path
    let abs_path = PathBuf::from(file_arg);
    if abs_path.is_absolute() && abs_path.exists() {
        return Ok(abs_path);
    }

    // 2. Try as alias from config
    let config = husako_config::load(project_root).map_err(|e| e.to_string())?;

    if let Some(cfg) = &config
        && let Some(mapped) = cfg.entries.get(file_arg)
    {
        let resolved = project_root.join(mapped);
        if resolved.exists() {
            return Ok(resolved);
        }
        return Err(format!(
            "entry alias '{file_arg}' maps to '{mapped}', but file not found at {}",
            resolved.display()
        ));
    }

    // 3. Not found
    let mut msg = format!("'{file_arg}' is not a file or entry alias");
    if let Some(cfg) = &config
        && !cfg.entries.is_empty()
    {
        msg.push_str("\n\navailable entry aliases:");
        let mut aliases: Vec<_> = cfg.entries.iter().collect();
        aliases.sort_by_key(|(k, _)| k.as_str());
        for (alias, path) in aliases {
            msg.push_str(&format!("\n  {alias} = \"{path}\""));
        }
    }
    Err(msg)
}

/// Interactively select a Kubernetes version.
///
/// Returns `Ok(version)` on success, `Err(None)` on Escape (abort),
/// `Err(Some(msg))` on fatal error.
///
/// Falls back to `DEFAULT_K8S_VERSION` when not running in a terminal
/// (e.g. piped or in CI) or when the network request fails.
fn select_k8s_version() -> Result<String, Option<String>> {
    // Non-interactive: use default without prompting
    if !console::Term::stderr().is_term() {
        return Ok(DEFAULT_K8S_VERSION.to_string());
    }

    // Fetch initial page of versions (show feedback while loading)
    eprintln!("{}", style::dim("Fetching Kubernetes versions..."));
    let initial = husako_core::version_check::discover_recent_releases(10, 0);
    // Clear the loading message
    let _ = console::Term::stderr().clear_last_lines(1);

    match initial {
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
            let mut has_more = versions.len() == 10;
            let mut next_offset: usize = 10;

            let result =
                search_select::run("Kubernetes version:", &mut items, &mut has_more, || {
                    let new_versions =
                        husako_core::version_check::discover_recent_releases(10, next_offset)
                            .map_err(|e| e.to_string())?;
                    let more = new_versions.len() == 10;
                    next_offset += 10;
                    Ok((new_versions, more))
                });

            match result {
                Ok(Some(idx)) => {
                    let selected = &items[idx];
                    let version = selected
                        .strip_suffix(" (latest)")
                        .unwrap_or(selected)
                        .to_string();
                    Ok(version)
                }
                Ok(None) => Err(None), // Escape
                Err(e) => Err(Some(e)),
            }
        }
        _ => {
            eprintln!(
                "{} could not fetch Kubernetes versions, using default ({DEFAULT_K8S_VERSION})",
                style::warning_prefix()
            );
            Ok(DEFAULT_K8S_VERSION.to_string())
        }
    }
}

fn build_resource_target(
    name: String,
    source: String,
    version: Option<String>,
    repo: Option<String>,
    tag: Option<String>,
    path: Option<String>,
) -> Result<husako_core::AddTarget, String> {
    let schema_source = match source.as_str() {
        "release" => {
            let version = version.ok_or("--version is required for release source")?;
            husako_config::SchemaSource::Release { version }
        }
        "cluster" => husako_config::SchemaSource::Cluster { cluster: None },
        "git" => {
            let repo = repo.ok_or("--repo is required for git source")?;
            let tag = tag.ok_or("--tag is required for git source")?;
            let path = path.ok_or("--path is required for git source")?;
            husako_config::SchemaSource::Git { repo, tag, path }
        }
        "file" => {
            let path = path.ok_or("--path is required for file source")?;
            husako_config::SchemaSource::File { path }
        }
        other => return Err(format!("unknown resource source type: {other}")),
    };
    Ok(husako_core::AddTarget::Resource {
        name,
        source: schema_source,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_chart_target(
    name: String,
    source: String,
    version: Option<String>,
    repo: Option<String>,
    tag: Option<String>,
    path: Option<String>,
    chart_name: Option<String>,
    package: Option<String>,
    reference: Option<String>,
) -> Result<husako_core::AddTarget, String> {
    let chart_source = match source.as_str() {
        "registry" => {
            let repo = repo.ok_or("--repo is required for registry source")?;
            let chart = chart_name.ok_or("--chart-name is required for registry source")?;
            let version = version.ok_or("--version is required for registry source")?;
            husako_config::ChartSource::Registry {
                repo,
                chart,
                version,
            }
        }
        "artifacthub" => {
            let package = package.ok_or("--package is required for artifacthub source")?;
            let version = version.ok_or("--version is required for artifacthub source")?;
            husako_config::ChartSource::ArtifactHub { package, version }
        }
        "git" => {
            let repo = repo.ok_or("--repo is required for git source")?;
            let tag = tag.ok_or("--tag is required for git source")?;
            let path = path.ok_or("--path is required for git source")?;
            husako_config::ChartSource::Git { repo, tag, path }
        }
        "file" => {
            let path = path.ok_or("--path is required for file source")?;
            husako_config::ChartSource::File { path }
        }
        "oci" => {
            let reference = reference.ok_or("--reference is required for oci source")?;
            let version = version.ok_or("--version is required for oci source")?;
            husako_config::ChartSource::Oci { reference, version }
        }
        other => return Err(format!("unknown chart source type: {other}")),
    };
    Ok(husako_core::AddTarget::Chart {
        name,
        source: chart_source,
    })
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{bytes} B")
    }
}

fn exit_code(err: &HusakoError) -> u8 {
    match err {
        HusakoError::Compile(_) => 3,
        HusakoError::Runtime(
            RuntimeError::Init(_)
            | RuntimeError::Execution(_)
            | RuntimeError::Timeout(_)
            | RuntimeError::MemoryLimit(_),
        ) => 4,
        HusakoError::Runtime(
            RuntimeError::BuildNotCalled
            | RuntimeError::BuildCalledMultiple(_)
            | RuntimeError::StrictJson { .. },
        ) => 7,
        HusakoError::Emit(_) => 7,
        HusakoError::Validation(_) => 7,
        HusakoError::Dts(_) => 5,
        HusakoError::OpenApi(_) => 6,
        HusakoError::Chart(_) => 6,
        HusakoError::Config(_) => 2,
        HusakoError::GenerateIo(_) => 1,
    }
}
