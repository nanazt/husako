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
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Render {
            file,
            allow_outside_root,
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

            let filename = abs_file.to_string_lossy();
            let options = RenderOptions {
                project_root,
                allow_outside_root,
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
    }
}

fn exit_code(err: &HusakoError) -> u8 {
    match err {
        HusakoError::Compile(_) => 3,
        HusakoError::Runtime(RuntimeError::Init(_) | RuntimeError::Execution(_)) => 4,
        HusakoError::Runtime(
            RuntimeError::BuildNotCalled
            | RuntimeError::BuildCalledMultiple(_)
            | RuntimeError::StrictJson { .. },
        ) => 7,
        HusakoError::Emit(_) => 7,
        HusakoError::OpenApi(_) => 6,
    }
}
