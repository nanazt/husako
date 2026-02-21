use std::path::Path;

use oxc_allocator::Allocator;
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use oxc_transformer::{TransformOptions, Transformer};

#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("parse error: {0}")]
    Parse(String),
    #[error("transform error: {0}")]
    Transform(String),
}

pub fn compile(source: &str, filename: &str) -> Result<String, CompileError> {
    let allocator = Allocator::default();

    let source_type = SourceType::from_path(Path::new(filename))
        .map_err(|e| CompileError::Parse(e.to_string()))?;

    let ret = Parser::new(&allocator, source, source_type).parse();
    if ret.panicked {
        let msgs: Vec<String> = ret.errors.iter().map(|e| e.to_string()).collect();
        return Err(CompileError::Parse(msgs.join("; ")));
    }
    let mut program = ret.program;

    let semantic_ret = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&program);
    if !semantic_ret.errors.is_empty() {
        let msgs: Vec<String> = semantic_ret.errors.iter().map(|e| e.to_string()).collect();
        return Err(CompileError::Parse(msgs.join("; ")));
    }
    let scoping = semantic_ret.semantic.into_scoping();

    let options = TransformOptions::default();
    let transformer = Transformer::new(&allocator, Path::new(filename), &options);
    let transform_ret = transformer.build_with_scoping(scoping, &mut program);
    if !transform_ret.errors.is_empty() {
        let msgs: Vec<String> = transform_ret.errors.iter().map(|e| e.to_string()).collect();
        return Err(CompileError::Transform(msgs.join("; ")));
    }

    let codegen_ret = Codegen::new()
        .with_scoping(Some(transform_ret.scoping))
        .build(&program);

    Ok(codegen_ret.code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_type_annotations() {
        let ts = r#"const x: number = 42; export { x };"#;
        let js = compile(ts, "test.ts").unwrap();
        assert!(js.contains("const x = 42;"));
        assert!(!js.contains("number"));
    }

    #[test]
    fn invalid_syntax() {
        let ts = "const = ;";
        let result = compile(ts, "bad.ts");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CompileError::Parse(_)));
    }

    #[test]
    fn preserve_esm_import() {
        let ts = r#"import { build } from "husako"; build([]);"#;
        let js = compile(ts, "test.ts").unwrap();
        assert!(js.contains("import"));
        assert!(js.contains("husako"));
    }
}
