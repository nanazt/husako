mod loader;
mod resolver;

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

use rquickjs::loader::{BuiltinLoader, BuiltinResolver};
use rquickjs::{Context, Ctx, Error, Function, Module, Runtime, Value};

use loader::HusakoFileLoader;
use resolver::HusakoFileResolver;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("runtime init failed: {0}")]
    Init(String),
    #[error("execution failed: {0}")]
    Execution(String),
    #[error("build() was not called")]
    BuildNotCalled,
    #[error("build() was called {0} times (expected exactly 1)")]
    BuildCalledMultiple(u32),
    #[error("strict JSON violation at {path}: {message}")]
    StrictJson { path: String, message: String },
}

pub struct ExecuteOptions {
    pub entry_path: PathBuf,
    pub project_root: PathBuf,
    pub allow_outside_root: bool,
}

/// Extract a meaningful error message from rquickjs errors.
/// For `Error::Exception`, retrieves the actual JS exception from the context.
fn execution_error(ctx: &Ctx<'_>, err: Error) -> RuntimeError {
    if matches!(err, Error::Exception) {
        let caught = ctx.catch();
        if let Some(exc) = caught.as_exception() {
            let msg = exc.message().unwrap_or_default();
            let stack = exc.stack().unwrap_or_default();
            if stack.is_empty() {
                return RuntimeError::Execution(msg);
            }
            return RuntimeError::Execution(format!("{msg}\n{stack}"));
        }
        if let Ok(s) = caught.get::<String>() {
            return RuntimeError::Execution(s);
        }
    }
    RuntimeError::Execution(err.to_string())
}

pub fn execute(
    js_source: &str,
    options: &ExecuteOptions,
) -> Result<serde_json::Value, RuntimeError> {
    let rt = Runtime::new().map_err(|e| RuntimeError::Init(e.to_string()))?;
    let ctx = Context::full(&rt).map_err(|e| RuntimeError::Init(e.to_string()))?;

    let resolver = (
        BuiltinResolver::default()
            .with_module("husako")
            .with_module("husako/_base")
            .with_module("k8s/apps/v1")
            .with_module("k8s/core/v1"),
        HusakoFileResolver::new(
            &options.project_root,
            options.allow_outside_root,
            &options.entry_path,
        ),
    );
    let loader = (
        BuiltinLoader::default()
            .with_module("husako", husako_sdk::HUSAKO_MODULE)
            .with_module("husako/_base", husako_sdk::HUSAKO_BASE)
            .with_module("k8s/apps/v1", husako_sdk::K8S_APPS_V1)
            .with_module("k8s/core/v1", husako_sdk::K8S_CORE_V1),
        HusakoFileLoader::new(),
    );
    rt.set_loader(resolver, loader);

    let result: Rc<RefCell<Option<serde_json::Value>>> = Rc::new(RefCell::new(None));
    let call_count: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
    let capture_error: Rc<RefCell<Option<RuntimeError>>> = Rc::new(RefCell::new(None));

    let eval_result: Result<(), RuntimeError> = ctx.with(|ctx| {
        let result_clone = result.clone();
        let count_clone = call_count.clone();
        let error_clone = capture_error.clone();

        let build_fn = Function::new(ctx.clone(), move |val: Value<'_>| {
            let mut count = count_clone.borrow_mut();
            *count += 1;
            if *count > 1 {
                return;
            }

            match validate_and_convert(&val, "$") {
                Ok(json) => {
                    *result_clone.borrow_mut() = Some(json);
                }
                Err(e) => {
                    *error_clone.borrow_mut() = Some(e);
                }
            }
        })
        .map_err(|e| RuntimeError::Init(e.to_string()))?;

        ctx.globals()
            .set("__husako_build", build_fn)
            .map_err(|e| RuntimeError::Init(e.to_string()))?;

        let promise = Module::evaluate(ctx.clone(), "main", js_source)
            .map_err(|e| execution_error(&ctx, e))?;

        promise
            .finish::<()>()
            .map_err(|e| execution_error(&ctx, e))?;

        Ok(())
    });

    eval_result?;

    if let Some(err) = capture_error.borrow_mut().take() {
        return Err(err);
    }

    let count = *call_count.borrow();
    match count {
        0 => Err(RuntimeError::BuildNotCalled),
        1 => result
            .borrow_mut()
            .take()
            .ok_or_else(|| RuntimeError::Execution("build() captured no value".into())),
        n => Err(RuntimeError::BuildCalledMultiple(n)),
    }
}

fn validate_and_convert(val: &Value<'_>, path: &str) -> Result<serde_json::Value, RuntimeError> {
    let mut visited = HashSet::new();
    convert_value(val, path, &mut visited)
}

fn convert_value(
    val: &Value<'_>,
    path: &str,
    visited: &mut HashSet<usize>,
) -> Result<serde_json::Value, RuntimeError> {
    use rquickjs::Type;

    match val.type_of() {
        Type::Null => Ok(serde_json::Value::Null),
        Type::Bool => {
            let b = val.as_bool().unwrap();
            Ok(serde_json::Value::Bool(b))
        }
        Type::Int => {
            let n = val.as_int().unwrap();
            Ok(serde_json::json!(n))
        }
        Type::Float => {
            let n = val.as_float().unwrap();
            if !n.is_finite() {
                return Err(RuntimeError::StrictJson {
                    path: path.to_string(),
                    message: format!("non-finite number: {n}"),
                });
            }
            Ok(serde_json::json!(n))
        }
        Type::String => {
            let s: String = val
                .get()
                .map_err(|e| RuntimeError::Execution(e.to_string()))?;
            Ok(serde_json::Value::String(s))
        }
        Type::Array => {
            let arr = val.as_array().unwrap();
            // SAFETY: reading the raw pointer value for identity-based cycle detection only
            let ptr = unsafe { val.as_raw().u.ptr as usize };
            if !visited.insert(ptr) {
                return Err(RuntimeError::StrictJson {
                    path: path.to_string(),
                    message: "cyclic reference detected".into(),
                });
            }
            let mut vec = Vec::with_capacity(arr.len());
            for i in 0..arr.len() {
                let item: Value = arr
                    .get(i)
                    .map_err(|e| RuntimeError::Execution(e.to_string()))?;
                let item_path = format!("{path}[{i}]");
                vec.push(convert_value(&item, &item_path, visited)?);
            }
            visited.remove(&ptr);
            Ok(serde_json::Value::Array(vec))
        }
        Type::Object => {
            let obj = val.as_object().unwrap();
            // SAFETY: reading the raw pointer value for identity-based cycle detection only
            let ptr = unsafe { val.as_raw().u.ptr as usize };
            if !visited.insert(ptr) {
                return Err(RuntimeError::StrictJson {
                    path: path.to_string(),
                    message: "cyclic reference detected".into(),
                });
            }
            let mut map = serde_json::Map::new();
            for result in obj.props::<String, Value>() {
                let (key, value) = result.map_err(|e| RuntimeError::Execution(e.to_string()))?;
                let prop_path = format!("{path}.{key}");
                map.insert(key, convert_value(&value, &prop_path, visited)?);
            }
            visited.remove(&ptr);
            Ok(serde_json::Value::Object(map))
        }
        Type::Undefined => Err(RuntimeError::StrictJson {
            path: path.to_string(),
            message: "undefined is not valid JSON".into(),
        }),
        Type::Function | Type::Constructor => Err(RuntimeError::StrictJson {
            path: path.to_string(),
            message: "function is not valid JSON".into(),
        }),
        Type::Symbol => Err(RuntimeError::StrictJson {
            path: path.to_string(),
            message: "symbol is not valid JSON".into(),
        }),
        Type::BigInt => Err(RuntimeError::StrictJson {
            path: path.to_string(),
            message: "bigint is not valid JSON".into(),
        }),
        other => Err(RuntimeError::StrictJson {
            path: path.to_string(),
            message: format!("{other:?} is not valid JSON"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_options() -> ExecuteOptions {
        ExecuteOptions {
            entry_path: PathBuf::from("/tmp/test.ts"),
            project_root: PathBuf::from("/tmp"),
            allow_outside_root: false,
        }
    }

    #[test]
    fn basic_build() {
        let js = r#"
            import { build } from "husako";
            build([{ apiVersion: "v1", kind: "Namespace" }]);
        "#;
        let result = execute(js, &test_options()).unwrap();
        assert!(result.is_array());
        assert_eq!(result[0]["kind"], "Namespace");
    }

    #[test]
    fn no_build_call() {
        let js = r#"
            import { build } from "husako";
            const x = 42;
        "#;
        let err = execute(js, &test_options()).unwrap_err();
        assert!(matches!(err, RuntimeError::BuildNotCalled));
    }

    #[test]
    fn double_build_call() {
        let js = r#"
            import { build } from "husako";
            build([]);
            build([]);
        "#;
        let err = execute(js, &test_options()).unwrap_err();
        assert!(matches!(err, RuntimeError::BuildCalledMultiple(2)));
    }

    #[test]
    fn strict_json_undefined() {
        let js = r#"
            import { build } from "husako";
            build({ a: undefined });
        "#;
        let err = execute(js, &test_options()).unwrap_err();
        assert!(matches!(err, RuntimeError::StrictJson { .. }));
        assert!(err.to_string().contains("undefined"));
    }

    #[test]
    fn strict_json_function() {
        let js = r#"
            import { build } from "husako";
            build({ fn: () => {} });
        "#;
        let err = execute(js, &test_options()).unwrap_err();
        assert!(matches!(err, RuntimeError::StrictJson { .. }));
        assert!(err.to_string().contains("function"));
    }

    // --- Milestone 3: SDK builder tests ---

    #[test]
    fn deployment_builder_basic() {
        let js = r#"
            import { build, name } from "husako";
            import { Deployment } from "k8s/apps/v1";
            const d = new Deployment().metadata(name("test"));
            build([d]);
        "#;
        let result = execute(js, &test_options()).unwrap();
        assert_eq!(result[0]["apiVersion"], "apps/v1");
        assert_eq!(result[0]["kind"], "Deployment");
        assert_eq!(result[0]["metadata"]["name"], "test");
    }

    #[test]
    fn namespace_builder() {
        let js = r#"
            import { build, name } from "husako";
            import { Namespace } from "k8s/core/v1";
            const ns = new Namespace().metadata(name("my-ns"));
            build([ns]);
        "#;
        let result = execute(js, &test_options()).unwrap();
        assert_eq!(result[0]["apiVersion"], "v1");
        assert_eq!(result[0]["kind"], "Namespace");
        assert_eq!(result[0]["metadata"]["name"], "my-ns");
    }

    #[test]
    fn metadata_fragment_immutability() {
        let js = r#"
            import { build, label } from "husako";
            import { Deployment } from "k8s/apps/v1";
            const base = label("env", "dev");
            const a = base.label("team", "a");
            const b = base.label("team", "b");
            const da = new Deployment().metadata(a);
            const db = new Deployment().metadata(b);
            build([da, db]);
        "#;
        let result = execute(js, &test_options()).unwrap();
        let a_labels = &result[0]["metadata"]["labels"];
        let b_labels = &result[1]["metadata"]["labels"];
        assert_eq!(a_labels["env"], "dev");
        assert_eq!(a_labels["team"], "a");
        assert_eq!(b_labels["env"], "dev");
        assert_eq!(b_labels["team"], "b");
    }

    #[test]
    fn merge_metadata_labels() {
        let js = r#"
            import { build, name, label, merge } from "husako";
            import { Deployment } from "k8s/apps/v1";
            const m = merge([name("test"), label("a", "1"), label("b", "2")]);
            const d = new Deployment().metadata(m);
            build([d]);
        "#;
        let result = execute(js, &test_options()).unwrap();
        assert_eq!(result[0]["metadata"]["name"], "test");
        assert_eq!(result[0]["metadata"]["labels"]["a"], "1");
        assert_eq!(result[0]["metadata"]["labels"]["b"], "2");
    }

    #[test]
    fn cpu_normalization() {
        let js = r#"
            import { build, cpu, requests } from "husako";
            import { Deployment } from "k8s/apps/v1";
            const d1 = new Deployment().resources(requests(cpu(1)));
            const d2 = new Deployment().resources(requests(cpu(0.5)));
            const d3 = new Deployment().resources(requests(cpu("250m")));
            build([d1, d2, d3]);
        "#;
        let result = execute(js, &test_options()).unwrap();
        assert_eq!(
            result[0]["spec"]["template"]["spec"]["containers"][0]["resources"]["requests"]["cpu"],
            "1"
        );
        assert_eq!(
            result[1]["spec"]["template"]["spec"]["containers"][0]["resources"]["requests"]["cpu"],
            "500m"
        );
        assert_eq!(
            result[2]["spec"]["template"]["spec"]["containers"][0]["resources"]["requests"]["cpu"],
            "250m"
        );
    }

    #[test]
    fn memory_normalization() {
        let js = r#"
            import { build, memory, requests } from "husako";
            import { Deployment } from "k8s/apps/v1";
            const d1 = new Deployment().resources(requests(memory(4)));
            const d2 = new Deployment().resources(requests(memory("512Mi")));
            build([d1, d2]);
        "#;
        let result = execute(js, &test_options()).unwrap();
        assert_eq!(
            result[0]["spec"]["template"]["spec"]["containers"][0]["resources"]["requests"]["memory"],
            "4Gi"
        );
        assert_eq!(
            result[1]["spec"]["template"]["spec"]["containers"][0]["resources"]["requests"]["memory"],
            "512Mi"
        );
    }

    #[test]
    fn resources_requests_and_limits() {
        let js = r#"
            import { build, cpu, memory, requests, limits } from "husako";
            import { Deployment } from "k8s/apps/v1";
            const d = new Deployment().resources(
                requests(cpu(1).memory("2Gi")),
                limits(cpu("500m").memory(1))
            );
            build([d]);
        "#;
        let result = execute(js, &test_options()).unwrap();
        let res = &result[0]["spec"]["template"]["spec"]["containers"][0]["resources"];
        assert_eq!(res["requests"]["cpu"], "1");
        assert_eq!(res["requests"]["memory"], "2Gi");
        assert_eq!(res["limits"]["cpu"], "500m");
        assert_eq!(res["limits"]["memory"], "1Gi");
    }

    #[test]
    fn backward_compat_plain_objects() {
        let js = r#"
            import { build } from "husako";
            build([{ apiVersion: "v1", kind: "Namespace", metadata: { name: "test" } }]);
        "#;
        let result = execute(js, &test_options()).unwrap();
        assert_eq!(result[0]["apiVersion"], "v1");
        assert_eq!(result[0]["kind"], "Namespace");
        assert_eq!(result[0]["metadata"]["name"], "test");
    }
}
