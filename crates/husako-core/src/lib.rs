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
}
