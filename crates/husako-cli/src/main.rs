use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use husako_core::{GenerateOptions, HusakoError, RenderOptions, ScaffoldOptions, TemplateName};
use husako_runtime_qjs::RuntimeError;

#[derive(Parser)]
#[command(name = "husako", version)]
struct Cli {
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
            let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

            // Resolve entry: try as file path first, then as alias from config
            let resolved = match resolve_entry(&file, &project_root) {
                Ok(p) => p,
                Err(msg) => {
                    eprintln!("error: {msg}");
                    return ExitCode::from(2);
                }
            };

            let source = match std::fs::read_to_string(&resolved) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error: could not read {}: {e}", resolved.display());
                    return ExitCode::from(1);
                }
            };

            let abs_file = match resolved.canonicalize() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("error: could not resolve {}: {e}", resolved.display());
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
                    eprintln!("error: {e}");
                    ExitCode::from(exit_code(&e))
                }
            }
        }
        Commands::Generate {
            api_server,
            spec_dir,
            skip_k8s,
        } => {
            let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

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

            // Load config for config-driven schema resolution
            let config = if openapi.is_none() && !skip_k8s {
                match husako_config::load(&project_root) {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        eprintln!("error: {e}");
                        return ExitCode::from(2);
                    }
                }
            } else {
                None
            };

            let options = GenerateOptions {
                project_root,
                openapi,
                skip_k8s,
                config,
            };

            match husako_core::generate(&options) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(exit_code(&e))
                }
            }
        }
        Commands::New {
            directory,
            template,
        } => {
            let options = ScaffoldOptions {
                directory: directory.clone(),
                template,
            };

            match husako_core::scaffold(&options) {
                Ok(()) => {
                    eprintln!("Created '{}' project in {}", template, directory.display());
                    eprintln!();
                    eprintln!("Next steps:");
                    eprintln!("  cd {}", directory.display());
                    eprintln!("  husako generate");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(exit_code(&e))
                }
            }
        }
    }
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
        HusakoError::Config(_) => 2,
        HusakoError::GenerateIo(_) => 1,
    }
}
