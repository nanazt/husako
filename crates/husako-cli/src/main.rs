use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use husako_core::{HusakoError, RenderOptions};
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
        /// Path to the TypeScript entry file
        file: PathBuf,

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

    /// Initialize project: generate type definitions and tsconfig.json
    Init {
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
            let source = match std::fs::read_to_string(&file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error: could not read {}: {e}", file.display());
                    return ExitCode::from(1);
                }
            };

            let abs_file = match file.canonicalize() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("error: could not resolve {}: {e}", file.display());
                    return ExitCode::from(1);
                }
            };

            let project_root = std::env::current_dir()
                .unwrap_or_else(|_| abs_file.parent().unwrap_or(&abs_file).to_path_buf());

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
        Commands::Init {
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

            let options = husako_core::InitOptions {
                project_root,
                openapi,
                skip_k8s,
            };

            match husako_core::init(&options) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(exit_code(&e))
                }
            }
        }
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
        HusakoError::InitIo(_) => 1,
    }
}
