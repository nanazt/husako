use std::path::PathBuf;

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
    #[error("init I/O error: {0}")]
    InitIo(String),
}

pub struct RenderOptions {
    pub project_root: PathBuf,
    pub allow_outside_root: bool,
}

pub fn render(
    source: &str,
    filename: &str,
    options: &RenderOptions,
) -> Result<String, HusakoError> {
    let js = husako_compile_oxc::compile(source, filename)?;

    let entry_path = std::path::Path::new(filename)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(filename));

    let exec_options = ExecuteOptions {
        entry_path,
        project_root: options.project_root.clone(),
        allow_outside_root: options.allow_outside_root,
    };

    let value = husako_runtime_qjs::execute(&js, &exec_options)?;
    let yaml = husako_yaml::emit_yaml(&value)?;
    Ok(yaml)
}

pub struct InitOptions {
    pub project_root: PathBuf,
    pub openapi: Option<husako_openapi::FetchOptions>,
    pub skip_k8s: bool,
}

/// Hardcoded list of kinds that have runtime builders.
fn registered_kinds() -> Vec<husako_dts::RegisteredKind> {
    vec![
        husako_dts::RegisteredKind {
            group: "apps".to_string(),
            version: "v1".to_string(),
            kind: "Deployment".to_string(),
        },
        husako_dts::RegisteredKind {
            group: String::new(),
            version: "v1".to_string(),
            kind: "Namespace".to_string(),
        },
        husako_dts::RegisteredKind {
            group: String::new(),
            version: "v1".to_string(),
            kind: "Service".to_string(),
        },
        husako_dts::RegisteredKind {
            group: String::new(),
            version: "v1".to_string(),
            kind: "ConfigMap".to_string(),
        },
    ]
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

        let gen_options = husako_dts::GenerateOptions {
            specs,
            registered_kinds: registered_kinds(),
        };

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
        }
    }

    #[test]
    fn end_to_end_render() {
        let ts = r#"
            import { build } from "husako";
            build([{ apiVersion: "v1", kind: "Namespace", metadata: { name: "test" } }]);
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
}
