mod interactive;
mod progress;
mod style;
mod theme;
mod url_detect;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use husako_core::{
    GenerateOptions, HusakoError, RenderOptions, ScaffoldOptions, TemplateName, TestOptions,
};
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

        /// Write output to a file or directory instead of stdout.
        /// If path ends with .yaml/.yml, writes to that file directly.
        /// Otherwise treated as a directory: writes <dir>/<name>.yaml
        /// where <name> is the entry alias or the entry file's stem.
        #[arg(long, short = 'o', value_name = "PATH")]
        output: Option<PathBuf>,

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
    #[command(name = "gen", alias = "generate")]
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
        /// URL, ArtifactHub package (e.g. bitnami/postgresql), or local path
        #[arg(value_name = "URL")]
        url: Option<String>,

        /// Chart name (for registry URLs: second positional or --name)
        #[arg(value_name = "CHART")]
        extra: Option<String>,

        /// Override the derived dependency name
        #[arg(long)]
        name: Option<String>,

        /// Add a Kubernetes release resource (e.g. --release 1.35)
        #[arg(long)]
        release: Option<String>,

        /// Add a cluster resource; optionally specify a named cluster
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        cluster: Option<String>,

        /// Pin version or partial semver prefix (16, 16.4, 16.4.0); v prefix optional
        #[arg(long)]
        version: Option<String>,

        /// Pin to a specific git tag
        #[arg(long)]
        tag: Option<String>,

        /// Pin to a git branch (clones HEAD; overrides tag/release default)
        #[arg(long)]
        branch: Option<String>,

        /// Override git sub-path
        #[arg(long)]
        path: Option<String>,
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

    /// Run test files
    Test {
        /// Test files to run (discovers *.test.ts and *.spec.ts if omitted)
        #[arg(value_name = "FILE")]
        files: Vec<PathBuf>,

        /// Execution timeout per file in milliseconds
        #[arg(long)]
        timeout_ms: Option<u64>,

        /// Maximum heap memory per file in megabytes
        #[arg(long)]
        max_heap_mb: Option<usize>,
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

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Render {
            file,
            output,
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

            // Pre-flight: if types are missing, auto-generate before rendering.
            let types_dir = project_root.join(".husako").join("types");
            if !types_dir.exists()
                && let Err(e) = run_auto_generate(&project_root).await
            {
                eprintln!("{} Could not generate types: {e}", style::error_prefix());
                return ExitCode::from(exit_code(&e));
            }

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

            match husako_core::render(&source, &filename, &options).await {
                Ok(yaml) => {
                    if let Some(out_path) = output {
                        let file_path = if out_path
                            .extension()
                            .is_some_and(|e| e == "yaml" || e == "yml")
                        {
                            out_path
                        } else {
                            let name = derive_out_name(&file, &options.project_root);
                            out_path.join(format!("{name}.yaml"))
                        };
                        if let Some(parent) = file_path.parent()
                            && let Err(e) = std::fs::create_dir_all(parent)
                        {
                            eprintln!("{} {e}", style::error_prefix());
                            return ExitCode::from(1);
                        }
                        if let Err(e) = std::fs::write(&file_path, &yaml) {
                            eprintln!("{} {e}", style::error_prefix());
                            return ExitCode::from(1);
                        }
                        eprintln!(
                            "{} Written to {}",
                            style::check_mark(),
                            style::bold(&file_path.display().to_string())
                        );
                    } else {
                        print!("{yaml}");
                    }
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

            match husako_core::generate(&options, &progress).await {
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
            let k8s_version = latest_k8s_version();

            let options = ScaffoldOptions {
                directory: directory.clone(),
                template,
                k8s_version: k8s_version.clone(),
            };

            match husako_core::scaffold(&options) {
                Ok(()) => {
                    eprintln!(
                        "{} Created '{}' project in {}",
                        style::check_mark(),
                        template,
                        directory.display()
                    );
                    eprintln!(
                        "  kubernetes {}  {}",
                        style::bold(&k8s_version),
                        style::dim("· edit husako.toml to use a different version")
                    );
                    eprintln!();
                    eprintln!("Next steps:");
                    eprintln!("  cd {}", directory.display());
                    eprintln!("  husako gen");
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
            let k8s_version = latest_k8s_version();

            let options = husako_core::InitOptions {
                directory: project_root,
                template,
                k8s_version: k8s_version.clone(),
            };

            match husako_core::init(&options) {
                Ok(()) => {
                    eprintln!(
                        "{} Created '{template}' project in current directory",
                        style::check_mark()
                    );
                    eprintln!(
                        "  kubernetes {}  {}",
                        style::bold(&k8s_version),
                        style::dim("· edit husako.toml to use a different version")
                    );
                    eprintln!();
                    eprintln!("Next steps:");
                    eprintln!("  husako gen");
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
            url,
            extra,
            name,
            release,
            cluster,
            version,
            tag,
            branch,
            path,
        } => {
            let project_root = cwd();

            match resolve_add_target(
                url,
                extra,
                name,
                release,
                cluster,
                version,
                tag,
                branch,
                path,
                cli.yes,
                &project_root,
            )
            .await
            {
                Ok(None) => ExitCode::SUCCESS, // user cancelled cluster confirmation
                Ok(Some(result)) => {
                    // Write [cluster] / [clusters.*] if URL came from kubeconfig
                    if let AddResult::Resource {
                        cluster_config: Some(ref cc),
                        ..
                    } = result
                    {
                        let (mut doc, doc_path) =
                            match husako_config::edit::load_document(&project_root) {
                                Ok(d) => d,
                                Err(e) => {
                                    eprintln!("{} {e}", style::error_prefix());
                                    return ExitCode::from(2u8);
                                }
                            };
                        husako_config::edit::add_cluster_config(
                            &mut doc,
                            cc.cluster_name.as_deref(),
                            &cc.server,
                        );
                        if let Err(e) = husako_config::edit::save_document(&doc, &doc_path) {
                            eprintln!("{} {e}", style::error_prefix());
                            return ExitCode::from(2u8);
                        }
                        let section = match &cc.cluster_name {
                            None => "[cluster]".to_string(),
                            Some(n) => format!("[clusters.{}]", n),
                        };
                        eprintln!(
                            "{} Added {} to husako.toml",
                            style::check_mark(),
                            style::bold(&section)
                        );
                    }

                    let target = match &result {
                        AddResult::Resource { name, source, .. } => {
                            husako_core::AddTarget::Resource {
                                name: name.clone(),
                                source: source.clone(),
                            }
                        }
                        AddResult::Chart { name, source } => husako_core::AddTarget::Chart {
                            name: name.clone(),
                            source: source.clone(),
                        },
                    };

                    match husako_core::add_dependency(&project_root, &target) {
                        Ok(()) => {
                            let (dep_name, section) = match &result {
                                AddResult::Resource { name, .. } => (name.as_str(), "resources"),
                                AddResult::Chart { name, .. } => (name.as_str(), "charts"),
                            };
                            eprintln!(
                                "{} Added {} to [{}]\n  {}",
                                style::check_mark(),
                                style::dep_name(dep_name),
                                section,
                                style::dim(&format_source_detail(&result))
                            );
                            eprintln!();
                            if let Err(e) = run_auto_generate(&project_root).await {
                                eprintln!(
                                    "{} Type generation failed: {e}",
                                    style::warning_prefix()
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
                Err(e) => {
                    eprintln!("{} {e}", style::error_prefix());
                    ExitCode::from(2)
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
                    eprintln!();
                    if let Err(e) = run_auto_generate(&project_root).await {
                        eprintln!("{} Type generation failed: {e}", style::warning_prefix());
                    }
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

            match husako_core::check_outdated(&project_root, &progress).await {
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

            match husako_core::update_dependencies(&options, &progress).await {
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
                    if let Err(e) = run_auto_generate(&project_root).await {
                        eprintln!("{} Type generation failed: {e}", style::warning_prefix());
                    }
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
                        eprintln!();
                        if let Err(e) = run_auto_generate(&project_root).await {
                            eprintln!("{} Type generation failed: {e}", style::warning_prefix());
                        }
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

            match husako_core::validate_file(&source, &filename, &options).await {
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

        Commands::Test {
            files,
            timeout_ms,
            max_heap_mb,
        } => {
            let project_root = cwd();

            // Resolve explicit file paths relative to cwd
            let resolved_files: Vec<PathBuf> = files
                .iter()
                .map(|f| {
                    if f.is_absolute() {
                        f.clone()
                    } else {
                        project_root.join(f)
                    }
                })
                .collect();

            let options = TestOptions {
                project_root: project_root.clone(),
                files: resolved_files,
                timeout_ms,
                max_heap_mb,
                allow_outside_root: false,
            };

            let results = match husako_core::run_tests(&options).await {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("{} {e}", style::error_prefix());
                    return ExitCode::from(exit_code(&e));
                }
            };

            if results.is_empty() {
                eprintln!("No test files found");
                return ExitCode::SUCCESS;
            }

            let mut total_passed = 0usize;
            let mut total_failed = 0usize;

            for result in &results {
                eprintln!("{}", style::bold(&result.file.to_string_lossy()));
                for case in &result.cases {
                    if case.passed {
                        eprintln!("  {} {}", style::check_mark(), case.name);
                        total_passed += 1;
                    } else {
                        eprintln!("  {} {}", style::cross_mark(), case.name);
                        if let Some(ref err) = case.error {
                            eprintln!("    {}", style::dim(err));
                        }
                        total_failed += 1;
                    }
                }
            }

            eprintln!();
            let summary = format!("{total_passed} passed, {total_failed} failed");
            if total_failed > 0 {
                eprintln!("{} {summary}", style::cross_mark());
                ExitCode::from(1u8)
            } else {
                eprintln!("{} {summary}", style::check_mark());
                ExitCode::SUCCESS
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

/// Derive the output file name (no extension) for a render `--output` directory.
///
/// - If `file_arg` matches an entry alias in `husako.toml`, returns the alias string as-is
///   (e.g. `"apps/my-app"` → `"apps/my-app"`, producing `dist/apps/my-app.yaml`).
/// - Otherwise returns the file stem of `file_arg`
///   (e.g. `"src/entry.ts"` → `"entry"`).
fn derive_out_name(file_arg: &str, project_root: &std::path::Path) -> String {
    if let Ok(Some(cfg)) = husako_config::load(project_root)
        && cfg.entries.contains_key(file_arg)
    {
        return file_arg.to_string();
    }
    std::path::Path::new(file_arg)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_arg.to_string())
}

/// Interactively select a Kubernetes version.
///
/// Returns `Ok(version)` on success, `Err(None)` on Escape (abort),
/// Fetches the latest Kubernetes version from GitHub.
/// Falls back to `DEFAULT_K8S_VERSION` silently on any network failure.
fn latest_k8s_version() -> String {
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(husako_core::version_check::discover_recent_releases(1, 0))
    });
    match result {
        Ok(versions) if !versions.is_empty() => versions.into_iter().next().unwrap(),
        _ => DEFAULT_K8S_VERSION.to_string(),
    }
}

/// Run generate with default options derived from husako.toml.
/// Returns Ok(()) on success, or a HusakoError on failure.
/// Used by commands that need fresh types after config changes.
async fn run_auto_generate(project_root: &std::path::Path) -> Result<(), HusakoError> {
    let progress = IndicatifReporter::new();
    let config = husako_config::load(project_root).ok().flatten();
    let options = GenerateOptions {
        project_root: project_root.to_path_buf(),
        openapi: None,
        skip_k8s: false,
        config,
    };
    husako_core::generate(&options, &progress).await
}

// --- husako add (URL auto-detect, no interactive) ---

struct ClusterConfigToAdd {
    cluster_name: Option<String>, // None = [cluster], Some("dev") = [clusters.dev]
    server: String,
}

enum AddResult {
    Resource {
        name: String,
        source: husako_config::SchemaSource,
        cluster_config: Option<ClusterConfigToAdd>,
    },
    Chart {
        name: String,
        source: husako_config::ChartSource,
    },
}

#[allow(clippy::too_many_arguments)]
async fn resolve_add_target(
    url: Option<String>,
    extra: Option<String>,
    name: Option<String>,
    release: Option<String>,
    cluster: Option<String>,
    version: Option<String>,
    tag: Option<String>,
    branch: Option<String>,
    path_override: Option<String>,
    yes: bool,
    project_root: &std::path::Path,
) -> Result<Option<AddResult>, String> {
    use husako_config::{ChartSource, SchemaSource};
    use url_detect::{SourceKind, UrlDetected, detect_url};

    // 1. Kubernetes release resource
    if let Some(ver) = release {
        let dep_name = name.unwrap_or_else(|| "kubernetes".to_string());
        return Ok(Some(AddResult::Resource {
            name: dep_name,
            source: SchemaSource::Release { version: ver },
            cluster_config: None,
        }));
    }

    // 2. Cluster resource
    if let Some(cluster_val) = cluster {
        let display_name = if cluster_val.is_empty() {
            "default"
        } else {
            &cluster_val
        };

        // Stage 1: husako.toml
        let config_server = husako_config::load(project_root)
            .ok()
            .flatten()
            .and_then(|cfg| {
                if cluster_val.is_empty() {
                    cfg.cluster.map(|c| c.server)
                } else {
                    cfg.clusters.get(&cluster_val).map(|c| c.server.clone())
                }
            });

        // Stage 2: kubeconfig fallback (only if husako.toml had nothing)
        let kube_server = if config_server.is_none() {
            let ctx = if cluster_val.is_empty() {
                None
            } else {
                Some(cluster_val.as_str())
            };
            husako_openapi::kubeconfig::server_for_context(ctx)
        } else {
            None
        };

        // Fail early if the cluster is not configured anywhere
        let server_url = match config_server.as_deref().or(kube_server.as_deref()) {
            Some(url) => url.to_string(),
            None => {
                let msg = if cluster_val.is_empty() {
                    "cluster is not configured — add [cluster] to husako.toml or set a current-context in kubeconfig".to_string()
                } else {
                    format!(
                        "cluster {:?} is not configured — add [clusters.{}] to husako.toml or ensure context {:?} exists in kubeconfig",
                        cluster_val, cluster_val, cluster_val
                    )
                };
                return Err(msg);
            }
        };

        // cluster_config is Some only when URL came from kubeconfig (not husako.toml)
        let cluster_config = kube_server.map(|s| ClusterConfigToAdd {
            cluster_name: if cluster_val.is_empty() {
                None
            } else {
                Some(cluster_val.clone())
            },
            server: s,
        });

        // Always show cluster identity (visible even with --yes, useful for audit)
        eprintln!(
            "  Cluster: {}  {}",
            style::dep_name(display_name),
            style::dim(&server_url),
        );
        if cluster_config.is_some() {
            let section = if cluster_val.is_empty() {
                "[cluster]".to_string()
            } else {
                format!("[clusters.{}]", cluster_val)
            };
            eprintln!(
                "  {} will add {} to husako.toml",
                style::arrow_mark(),
                style::bold(&section)
            );
        }

        if !yes {
            eprintln!(
                "{} Adding a cluster resource will fetch ALL CRDs from the cluster, which may be a large set.",
                style::warning_prefix()
            );
            match interactive::confirm("Continue?") {
                Ok(true) => {}
                Ok(false) => return Ok(None),
                Err(e) => return Err(e),
            }
        }

        // cluster_val == "" → --cluster without value → use "cluster" as name
        let cluster_name = if cluster_val.is_empty() {
            None
        } else {
            Some(cluster_val)
        };
        let dep_name =
            name.unwrap_or_else(|| cluster_name.as_deref().unwrap_or("cluster").to_string());
        return Ok(Some(AddResult::Resource {
            name: dep_name,
            source: SchemaSource::Cluster {
                cluster: cluster_name,
            },
            cluster_config,
        }));
    }

    // 3. URL-based detection
    if let Some(input) = url {
        let prefix = version.as_deref();

        match detect_url(&input) {
            Some(UrlDetected::ArtifactHub { package }) => {
                let dep_name = name.unwrap_or_else(|| after_slash(&package));
                let ver = husako_core::version_check::discover_latest_artifacthub(&package, prefix)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(Some(AddResult::Chart {
                    name: dep_name,
                    source: ChartSource::ArtifactHub {
                        package,
                        version: ver,
                    },
                }))
            }

            Some(UrlDetected::Oci { reference }) => {
                let dep_name = name.unwrap_or_else(|| last_path_component(&reference));
                let ver = if let Some(v) = version {
                    v
                } else {
                    husako_core::version_check::discover_latest_oci(&reference)
                        .await
                        .map_err(|e| e.to_string())?
                        .ok_or_else(|| {
                            format!(
                                "could not detect latest version for '{reference}'; use --version"
                            )
                        })?
                };
                Ok(Some(AddResult::Chart {
                    name: dep_name,
                    source: ChartSource::Oci {
                        reference,
                        version: ver,
                    },
                }))
            }

            Some(UrlDetected::Git {
                repo,
                sub_path,
                branch: url_branch,
            }) => {
                let effective_branch = branch.or(url_branch);
                let dep_name = name.unwrap_or_else(|| repo_name(&repo));
                let tempdir = tempfile::tempdir().map_err(|e| e.to_string())?;

                if let Some(br) = effective_branch {
                    git_clone_sparse(&repo, &br, tempdir.path()).await?;
                    let look_in = path_override
                        .as_deref()
                        .or(sub_path.as_deref())
                        .unwrap_or(".");
                    let kind = url_detect::detect_git_kind(tempdir.path(), look_in)?;
                    let path = path_override
                        .or(sub_path)
                        .unwrap_or_else(|| ".".to_string());
                    match kind {
                        SourceKind::Resource => Ok(Some(AddResult::Resource {
                            name: dep_name,
                            source: SchemaSource::Git {
                                repo,
                                tag: br,
                                path,
                            },
                            cluster_config: None,
                        })),
                        SourceKind::Chart => Ok(Some(AddResult::Chart {
                            name: dep_name,
                            source: ChartSource::Git {
                                repo,
                                tag: br,
                                path,
                            },
                        })),
                    }
                } else {
                    let resolved_tag = if let Some(t) = tag {
                        t
                    } else {
                        husako_core::version_check::discover_latest_git_tag(&repo, prefix)
                            .map_err(|e| e.to_string())?
                            .ok_or_else(|| {
                                format!("no release tags found in '{repo}'; use --tag or --branch")
                            })?
                    };
                    git_clone_sparse(&repo, &resolved_tag, tempdir.path()).await?;
                    let look_in = path_override
                        .as_deref()
                        .or(sub_path.as_deref())
                        .unwrap_or(".");
                    let kind = url_detect::detect_git_kind(tempdir.path(), look_in)?;
                    let path = path_override
                        .or(sub_path)
                        .unwrap_or_else(|| ".".to_string());
                    match kind {
                        SourceKind::Resource => Ok(Some(AddResult::Resource {
                            name: dep_name,
                            source: SchemaSource::Git {
                                repo,
                                tag: resolved_tag,
                                path,
                            },
                            cluster_config: None,
                        })),
                        SourceKind::Chart => Ok(Some(AddResult::Chart {
                            name: dep_name,
                            source: ChartSource::Git {
                                repo,
                                tag: resolved_tag,
                                path,
                            },
                        })),
                    }
                }
            }

            Some(UrlDetected::HelmRegistry { repo }) => {
                // chart name: --name takes priority over second positional
                let chart_name = name.or(extra).ok_or_else(|| {
                    "--name <chart> or second argument required for registry URL\nexamples:\n  husako add https://charts.example.com cert-manager\n  husako add https://charts.example.com --name cert-manager"
                        .to_string()
                })?;
                let ver = husako_core::version_check::discover_latest_registry(
                    &repo,
                    &chart_name,
                    prefix,
                )
                .await
                .map_err(|e| e.to_string())?;
                Ok(Some(AddResult::Chart {
                    name: chart_name.clone(),
                    source: ChartSource::Registry {
                        repo,
                        chart: chart_name,
                        version: ver,
                    },
                }))
            }

            Some(UrlDetected::LocalPath { path }) => {
                let kind = url_detect::detect_local_kind(&path)?;
                let dep_name = name.unwrap_or_else(|| file_stem(&path));
                match kind {
                    SourceKind::Resource => Ok(Some(AddResult::Resource {
                        name: dep_name,
                        source: SchemaSource::File { path },
                        cluster_config: None,
                    })),
                    SourceKind::Chart => Ok(Some(AddResult::Chart {
                        name: dep_name,
                        source: ChartSource::File { path },
                    })),
                }
            }

            None => Err(format!(
                "'{input}' is not a recognized URL or package\nexamples:\n  husako add bitnami/postgresql\n  husako add https://github.com/cert-manager/cert-manager\n  husako add --release 1.35"
            )),
        }
    } else {
        Err(
            "url required, or use --release <version> / --cluster [name]\nsee: husako add --help"
                .to_string(),
        )
    }
}

/// Shallow-clone a git repo at a specific tag or branch (depth 1).
async fn git_clone_sparse(repo: &str, tag: &str, dir: &std::path::Path) -> Result<(), String> {
    let output = tokio::process::Command::new("git")
        .args(["clone", "--depth", "1", "--branch", tag, repo])
        .arg(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("git clone: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git clone failed for '{repo}' @ '{tag}': {stderr}"));
    }
    Ok(())
}

fn format_source_detail(result: &AddResult) -> String {
    use husako_config::{ChartSource, SchemaSource};
    match result {
        AddResult::Resource {
            source: SchemaSource::Release { version },
            ..
        } => {
            format!("release  {version}")
        }
        AddResult::Resource {
            source: SchemaSource::Cluster { cluster },
            ..
        } => {
            format!("cluster  {}", cluster.as_deref().unwrap_or("default"))
        }
        AddResult::Resource {
            source: SchemaSource::Git { repo, tag, .. },
            ..
        } => {
            format!("git  {repo} @ {tag}")
        }
        AddResult::Resource {
            source: SchemaSource::File { path },
            ..
        } => {
            format!("file  {path}")
        }
        AddResult::Chart {
            source: ChartSource::ArtifactHub { package, version },
            ..
        } => {
            format!("artifacthub  {package} @ {version}")
        }
        AddResult::Chart {
            source:
                ChartSource::Registry {
                    repo,
                    chart,
                    version,
                },
            ..
        } => {
            format!("registry  {chart} @ {version}  ({repo})")
        }
        AddResult::Chart {
            source: ChartSource::Oci { reference, version },
            ..
        } => {
            format!("oci  {reference} @ {version}")
        }
        AddResult::Chart {
            source: ChartSource::Git { repo, tag, .. },
            ..
        } => {
            format!("git  {repo} @ {tag}")
        }
        AddResult::Chart {
            source: ChartSource::File { path },
            ..
        } => {
            format!("file  {path}")
        }
    }
}

// --- String helpers ---

fn after_slash(s: &str) -> String {
    s.rsplit('/').next().unwrap_or(s).to_string()
}

fn last_path_component(s: &str) -> String {
    let without_query = s.split('?').next().unwrap_or(s);
    let without_fragment = without_query.split('#').next().unwrap_or(without_query);
    without_fragment
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(without_fragment)
        .split(':')
        .next()
        .unwrap_or(without_fragment)
        .to_string()
}

fn repo_name(url: &str) -> String {
    // Last path segment of scheme://host/org/repo
    url.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(url)
        .trim_end_matches(".git")
        .to_string()
}

fn file_stem(path: &str) -> String {
    let p = std::path::Path::new(path);
    if p.is_dir() {
        p.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path)
            .to_string()
    } else {
        p.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(path)
            .to_string()
    }
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
