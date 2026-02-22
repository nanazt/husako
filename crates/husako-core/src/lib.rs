pub mod quantity;
pub mod validate;

use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use husako_runtime_qjs::ExecuteOptions;

#[derive(Debug, thiserror::Error)]
pub enum HusakoError {
    #[error(transparent)]
    Compile(#[from] husako_compile_oxc::CompileError),
    #[error(transparent)]
    Runtime(#[from] husako_runtime_qjs::RuntimeError),
    #[error(transparent)]
    Emit(#[from] husako_yaml::EmitError),
    #[error(transparent)]
    OpenApi(#[from] husako_openapi::OpenApiError),
    #[error(transparent)]
    Dts(#[from] husako_dts::DtsError),
    #[error(transparent)]
    Config(#[from] husako_config::ConfigError),
    #[error("{0}")]
    Validation(String),
    #[error("init I/O error: {0}")]
    InitIo(String),
}

pub struct RenderOptions {
    pub project_root: PathBuf,
    pub allow_outside_root: bool,
    pub schema_store: Option<validate::SchemaStore>,
    pub timeout_ms: Option<u64>,
    pub max_heap_mb: Option<usize>,
    pub verbose: bool,
}

pub fn render(
    source: &str,
    filename: &str,
    options: &RenderOptions,
) -> Result<String, HusakoError> {
    let js = husako_compile_oxc::compile(source, filename)?;

    if options.verbose {
        eprintln!(
            "[compile] {} ({} bytes → {} bytes JS)",
            filename,
            source.len(),
            js.len()
        );
    }

    let entry_path = std::path::Path::new(filename)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(filename));

    let generated_types_dir = options
        .project_root
        .join(".husako/types")
        .canonicalize()
        .ok();

    let exec_options = ExecuteOptions {
        entry_path,
        project_root: options.project_root.clone(),
        allow_outside_root: options.allow_outside_root,
        timeout_ms: options.timeout_ms,
        max_heap_mb: options.max_heap_mb,
        generated_types_dir,
    };

    if options.verbose {
        eprintln!(
            "[execute] QuickJS: timeout={}ms, heap={}MB",
            options
                .timeout_ms
                .map_or("none".to_string(), |ms| ms.to_string()),
            options
                .max_heap_mb
                .map_or("none".to_string(), |mb| mb.to_string()),
        );
    }

    let execute_start = std::time::Instant::now();
    let value = husako_runtime_qjs::execute(&js, &exec_options)?;

    if options.verbose {
        eprintln!("[execute] done ({}ms)", execute_start.elapsed().as_millis());
    }

    let validate_mode = if options.schema_store.is_some() {
        "schema-based"
    } else {
        "fallback"
    };
    let doc_count = if let serde_json::Value::Array(arr) = &value {
        arr.len()
    } else {
        1
    };

    if options.verbose {
        eprintln!("[validate] {} documents, {}", doc_count, validate_mode);
    }

    let validate_start = std::time::Instant::now();
    if let Err(errors) = validate::validate(&value, options.schema_store.as_ref()) {
        let msg = errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        return Err(HusakoError::Validation(msg));
    }

    if options.verbose {
        eprintln!(
            "[validate] done ({}ms), 0 errors",
            validate_start.elapsed().as_millis()
        );
    }

    let yaml = husako_yaml::emit_yaml(&value)?;

    if options.verbose {
        let line_count = yaml.lines().count();
        eprintln!("[emit] {} documents ({} lines YAML)", doc_count, line_count);
    }

    Ok(yaml)
}

/// Load a `SchemaStore` from `.husako/types/k8s/_schema.json` if it exists.
pub fn load_schema_store(project_root: &Path) -> Option<validate::SchemaStore> {
    validate::load_schema_store(project_root)
}

pub struct InitOptions {
    pub project_root: PathBuf,
    pub openapi: Option<husako_openapi::FetchOptions>,
    pub skip_k8s: bool,
}

pub fn init(options: &InitOptions) -> Result<(), HusakoError> {
    let types_dir = options.project_root.join(".husako/types");

    // 1. Write static husako.d.ts
    write_file(&types_dir.join("husako.d.ts"), husako_sdk::HUSAKO_DTS)?;

    // 2. Write static husako/_base.d.ts
    write_file(
        &types_dir.join("husako/_base.d.ts"),
        husako_sdk::HUSAKO_BASE_DTS,
    )?;

    // 3. Generate k8s types if not skipped
    if !options.skip_k8s
        && let Some(openapi_opts) = &options.openapi
    {
        let client = husako_openapi::OpenApiClient::new(husako_openapi::FetchOptions {
            source: match &openapi_opts.source {
                husako_openapi::OpenApiSource::Url {
                    base_url,
                    bearer_token,
                } => husako_openapi::OpenApiSource::Url {
                    base_url: base_url.clone(),
                    bearer_token: bearer_token.clone(),
                },
                husako_openapi::OpenApiSource::Directory(p) => {
                    husako_openapi::OpenApiSource::Directory(p.clone())
                }
            },
            cache_dir: options.project_root.join(".husako/cache"),
            offline: openapi_opts.offline,
        })?;

        let specs = client.fetch_all_specs()?;

        let gen_options = husako_dts::GenerateOptions { specs };

        let result = husako_dts::generate(&gen_options)?;

        for (rel_path, content) in &result.files {
            write_file(&types_dir.join(rel_path), content)?;
        }
    }

    // 4. Write/update tsconfig.json
    write_tsconfig(&options.project_root)?;

    Ok(())
}

fn write_file(path: &std::path::Path, content: &str) -> Result<(), HusakoError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| HusakoError::InitIo(format!("create dir {}: {e}", parent.display())))?;
    }
    std::fs::write(path, content)
        .map_err(|e| HusakoError::InitIo(format!("write {}: {e}", path.display())))
}

fn write_tsconfig(project_root: &std::path::Path) -> Result<(), HusakoError> {
    let tsconfig_path = project_root.join("tsconfig.json");

    let husako_paths: serde_json::Value = serde_json::json!({
        "husako": [".husako/types/husako.d.ts"],
        "husako/_base": [".husako/types/husako/_base.d.ts"],
        "k8s/*": [".husako/types/k8s/*"]
    });

    let config = if tsconfig_path.exists() {
        let content = std::fs::read_to_string(&tsconfig_path)
            .map_err(|e| HusakoError::InitIo(format!("read {}: {e}", tsconfig_path.display())))?;

        match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(mut root) => {
                // Merge paths into existing compilerOptions
                let compiler_options = root
                    .as_object_mut()
                    .and_then(|obj| {
                        if !obj.contains_key("compilerOptions") {
                            obj.insert("compilerOptions".to_string(), serde_json::json!({}));
                        }
                        obj.get_mut("compilerOptions")
                    })
                    .and_then(|co| co.as_object_mut());

                if let Some(co) = compiler_options {
                    let paths = co.entry("paths").or_insert_with(|| serde_json::json!({}));
                    if let Some(paths_obj) = paths.as_object_mut()
                        && let Some(husako_obj) = husako_paths.as_object()
                    {
                        for (k, v) in husako_obj {
                            paths_obj.insert(k.clone(), v.clone());
                        }
                    }
                }

                root
            }
            Err(_) => {
                eprintln!("warning: could not parse existing tsconfig.json, creating new one");
                new_tsconfig(husako_paths)
            }
        }
    } else {
        new_tsconfig(husako_paths)
    };

    let formatted = serde_json::to_string_pretty(&config)
        .map_err(|e| HusakoError::InitIo(format!("serialize tsconfig.json: {e}")))?;

    std::fs::write(&tsconfig_path, formatted + "\n")
        .map_err(|e| HusakoError::InitIo(format!("write {}: {e}", tsconfig_path.display())))
}

// --- husako new ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateName {
    Simple,
    Project,
    MultiEnv,
}

impl FromStr for TemplateName {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "simple" => Ok(Self::Simple),
            "project" => Ok(Self::Project),
            "multi-env" => Ok(Self::MultiEnv),
            _ => Err(format!(
                "unknown template '{s}'. Available: simple, project, multi-env"
            )),
        }
    }
}

impl fmt::Display for TemplateName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Simple => write!(f, "simple"),
            Self::Project => write!(f, "project"),
            Self::MultiEnv => write!(f, "multi-env"),
        }
    }
}

pub struct ScaffoldOptions {
    pub directory: PathBuf,
    pub template: TemplateName,
}

pub fn scaffold(options: &ScaffoldOptions) -> Result<(), HusakoError> {
    let dir = &options.directory;

    // Reject non-empty existing directories
    if dir.exists() {
        let is_empty = std::fs::read_dir(dir)
            .map_err(|e| HusakoError::InitIo(format!("read dir {}: {e}", dir.display())))?
            .next()
            .is_none();
        if !is_empty {
            return Err(HusakoError::InitIo(format!(
                "directory '{}' is not empty",
                dir.display()
            )));
        }
    }

    // Create directory
    std::fs::create_dir_all(dir)
        .map_err(|e| HusakoError::InitIo(format!("create dir {}: {e}", dir.display())))?;

    // Write .gitignore (shared across all templates)
    write_file(&dir.join(".gitignore"), husako_sdk::TEMPLATE_GITIGNORE)?;

    match options.template {
        TemplateName::Simple => {
            write_file(
                &dir.join(husako_config::CONFIG_FILENAME),
                husako_sdk::TEMPLATE_SIMPLE_CONFIG,
            )?;
            write_file(&dir.join("entry.ts"), husako_sdk::TEMPLATE_SIMPLE_ENTRY)?;
        }
        TemplateName::Project => {
            write_file(
                &dir.join(husako_config::CONFIG_FILENAME),
                husako_sdk::TEMPLATE_PROJECT_CONFIG,
            )?;
            write_file(
                &dir.join("env/dev.ts"),
                husako_sdk::TEMPLATE_PROJECT_ENV_DEV,
            )?;
            write_file(
                &dir.join("deployments/nginx.ts"),
                husako_sdk::TEMPLATE_PROJECT_DEPLOY_NGINX,
            )?;
            write_file(
                &dir.join("lib/index.ts"),
                husako_sdk::TEMPLATE_PROJECT_LIB_INDEX,
            )?;
            write_file(
                &dir.join("lib/metadata.ts"),
                husako_sdk::TEMPLATE_PROJECT_LIB_METADATA,
            )?;
        }
        TemplateName::MultiEnv => {
            write_file(
                &dir.join(husako_config::CONFIG_FILENAME),
                husako_sdk::TEMPLATE_MULTI_ENV_CONFIG,
            )?;
            write_file(
                &dir.join("base/nginx.ts"),
                husako_sdk::TEMPLATE_MULTI_ENV_BASE_NGINX,
            )?;
            write_file(
                &dir.join("base/service.ts"),
                husako_sdk::TEMPLATE_MULTI_ENV_BASE_SERVICE,
            )?;
            write_file(
                &dir.join("dev/main.ts"),
                husako_sdk::TEMPLATE_MULTI_ENV_DEV_MAIN,
            )?;
            write_file(
                &dir.join("staging/main.ts"),
                husako_sdk::TEMPLATE_MULTI_ENV_STAGING_MAIN,
            )?;
            write_file(
                &dir.join("release/main.ts"),
                husako_sdk::TEMPLATE_MULTI_ENV_RELEASE_MAIN,
            )?;
        }
    }

    Ok(())
}

fn new_tsconfig(husako_paths: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "compilerOptions": {
            "strict": true,
            "module": "ESNext",
            "moduleResolution": "bundler",
            "paths": husako_paths
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_options() -> RenderOptions {
        RenderOptions {
            project_root: PathBuf::from("/tmp"),
            allow_outside_root: false,
            schema_store: None,
            timeout_ms: None,
            max_heap_mb: None,
            verbose: false,
        }
    }

    #[test]
    fn end_to_end_render() {
        let ts = r#"
            import { build } from "husako";
            build([{ _render() { return { apiVersion: "v1", kind: "Namespace", metadata: { name: "test" } }; } }]);
        "#;
        let yaml = render(ts, "test.ts", &test_options()).unwrap();
        assert!(yaml.contains("apiVersion: v1"));
        assert!(yaml.contains("kind: Namespace"));
        assert!(yaml.contains("name: test"));
    }

    #[test]
    fn compile_error_propagates() {
        let ts = "const = ;";
        let err = render(ts, "bad.ts", &test_options()).unwrap_err();
        assert!(matches!(err, HusakoError::Compile(_)));
    }

    #[test]
    fn missing_build_propagates() {
        let ts = r#"import { build } from "husako"; const x = 1;"#;
        let err = render(ts, "test.ts", &test_options()).unwrap_err();
        assert!(matches!(
            err,
            HusakoError::Runtime(husako_runtime_qjs::RuntimeError::BuildNotCalled)
        ));
    }

    #[test]
    fn init_skip_k8s_writes_static_dts() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();

        let opts = InitOptions {
            project_root: root.clone(),
            openapi: None,
            skip_k8s: true,
        };
        init(&opts).unwrap();

        // Check static .d.ts files exist
        assert!(root.join(".husako/types/husako.d.ts").exists());
        assert!(root.join(".husako/types/husako/_base.d.ts").exists());

        // Check tsconfig.json
        let tsconfig = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&tsconfig).unwrap();
        assert!(parsed["compilerOptions"]["paths"]["husako"].is_array());
        assert!(parsed["compilerOptions"]["paths"]["k8s/*"].is_array());

        // No k8s/ directory
        assert!(!root.join(".husako/types/k8s").exists());
    }

    #[test]
    fn init_updates_existing_tsconfig() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();

        // Pre-create tsconfig.json with existing content
        let existing = serde_json::json!({
            "compilerOptions": {
                "strict": true,
                "target": "ES2020",
                "paths": {
                    "mylib/*": ["./lib/*"]
                }
            },
            "include": ["src/**/*"]
        });
        std::fs::write(
            root.join("tsconfig.json"),
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        let opts = InitOptions {
            project_root: root.clone(),
            openapi: None,
            skip_k8s: true,
        };
        init(&opts).unwrap();

        let tsconfig = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&tsconfig).unwrap();

        // Original fields preserved
        assert_eq!(parsed["compilerOptions"]["target"], "ES2020");
        assert!(parsed["include"].is_array());

        // Original path preserved
        assert!(parsed["compilerOptions"]["paths"]["mylib/*"].is_array());

        // husako paths added
        assert!(parsed["compilerOptions"]["paths"]["husako"].is_array());
        assert!(parsed["compilerOptions"]["paths"]["k8s/*"].is_array());
    }

    #[test]
    fn template_name_from_str() {
        assert_eq!(
            TemplateName::from_str("simple").unwrap(),
            TemplateName::Simple
        );
        assert_eq!(
            TemplateName::from_str("project").unwrap(),
            TemplateName::Project
        );
        assert_eq!(
            TemplateName::from_str("multi-env").unwrap(),
            TemplateName::MultiEnv
        );
        assert!(TemplateName::from_str("unknown").is_err());
    }

    #[test]
    fn template_name_display() {
        assert_eq!(TemplateName::Simple.to_string(), "simple");
        assert_eq!(TemplateName::Project.to_string(), "project");
        assert_eq!(TemplateName::MultiEnv.to_string(), "multi-env");
    }

    #[test]
    fn scaffold_simple_creates_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("my-app");

        let opts = ScaffoldOptions {
            directory: dir.clone(),
            template: TemplateName::Simple,
        };
        scaffold(&opts).unwrap();

        assert!(dir.join(".gitignore").exists());
        assert!(dir.join("husako.toml").exists());
        assert!(dir.join("entry.ts").exists());
    }

    #[test]
    fn scaffold_project_creates_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("my-app");

        let opts = ScaffoldOptions {
            directory: dir.clone(),
            template: TemplateName::Project,
        };
        scaffold(&opts).unwrap();

        assert!(dir.join(".gitignore").exists());
        assert!(dir.join("husako.toml").exists());
        assert!(dir.join("env/dev.ts").exists());
        assert!(dir.join("deployments/nginx.ts").exists());
        assert!(dir.join("lib/index.ts").exists());
        assert!(dir.join("lib/metadata.ts").exists());
    }

    #[test]
    fn scaffold_multi_env_creates_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("my-app");

        let opts = ScaffoldOptions {
            directory: dir.clone(),
            template: TemplateName::MultiEnv,
        };
        scaffold(&opts).unwrap();

        assert!(dir.join(".gitignore").exists());
        assert!(dir.join("husako.toml").exists());
        assert!(dir.join("base/nginx.ts").exists());
        assert!(dir.join("base/service.ts").exists());
        assert!(dir.join("dev/main.ts").exists());
        assert!(dir.join("staging/main.ts").exists());
        assert!(dir.join("release/main.ts").exists());
    }

    #[test]
    fn scaffold_rejects_nonempty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("my-app");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("existing.txt"), "content").unwrap();

        let opts = ScaffoldOptions {
            directory: dir,
            template: TemplateName::Simple,
        };
        let err = scaffold(&opts).unwrap_err();
        assert!(matches!(err, HusakoError::InitIo(_)));
        assert!(err.to_string().contains("not empty"));
    }

    #[test]
    fn scaffold_allows_empty_existing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("my-app");
        std::fs::create_dir_all(&dir).unwrap();

        let opts = ScaffoldOptions {
            directory: dir.clone(),
            template: TemplateName::Simple,
        };
        scaffold(&opts).unwrap();

        assert!(dir.join("entry.ts").exists());
    }
}
