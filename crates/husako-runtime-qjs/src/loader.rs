use std::collections::HashMap;
use std::path::Path;

use rquickjs::module::Declared;
use rquickjs::{Ctx, Error, Module, Result};

pub struct HusakoFileLoader {
    cache: HashMap<String, Vec<u8>>,
}

impl HusakoFileLoader {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }
}

impl rquickjs::loader::Loader for HusakoFileLoader {
    fn load<'js>(&mut self, ctx: &Ctx<'js>, name: &str) -> Result<Module<'js, Declared>> {
        // If cached, use the cached source
        if let Some(source) = self.cache.get(name) {
            return Module::declare(ctx.clone(), name, source.clone());
        }

        let path = Path::new(name);

        // Read the file
        let source = std::fs::read_to_string(path)
            .map_err(|e| Error::new_loading_message(name, e.to_string()))?;

        // Compile TypeScript if needed
        let js = if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("ts" | "husako")
        ) {
            husako_compile_oxc::compile(&source, name)
                .map_err(|e| Error::new_loading_message(name, e.to_string()))?
        } else {
            source
        };

        let bytes = js.into_bytes();
        self.cache.insert(name.to_string(), bytes.clone());
        Module::declare(ctx.clone(), name, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rquickjs::loader::Loader;

    #[test]
    fn load_ts_file() {
        let dir = tempfile::tempdir().unwrap();
        let ts_path = dir.path().join("helper.ts");
        std::fs::write(&ts_path, "export const x: number = 42;").unwrap();

        let mut loader = HusakoFileLoader::new();
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let name = ts_path.to_str().unwrap();
            let module = loader.load(&ctx, name);
            assert!(module.is_ok());
        });
    }

    #[test]
    fn load_js_file() {
        let dir = tempfile::tempdir().unwrap();
        let js_path = dir.path().join("helper.js");
        std::fs::write(&js_path, "export const x = 42;").unwrap();

        let mut loader = HusakoFileLoader::new();
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let name = js_path.to_str().unwrap();
            let module = loader.load(&ctx, name);
            assert!(module.is_ok());
        });
    }

    #[test]
    fn load_husako_file() {
        let dir = tempfile::tempdir().unwrap();
        let husako_path = dir.path().join("entry.husako");
        std::fs::write(&husako_path, "export const x: number = 42;").unwrap();

        let mut loader = HusakoFileLoader::new();
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let name = husako_path.to_str().unwrap();
            let module = loader.load(&ctx, name);
            assert!(module.is_ok());
        });
    }

    #[test]
    fn caches_loaded_module() {
        let dir = tempfile::tempdir().unwrap();
        let ts_path = dir.path().join("cached.ts");
        std::fs::write(&ts_path, "export const x: number = 1;").unwrap();

        let mut loader = HusakoFileLoader::new();
        let name = ts_path.to_str().unwrap();

        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let _ = loader.load(&ctx, name);
        });

        assert!(loader.cache.contains_key(name));
    }
}
