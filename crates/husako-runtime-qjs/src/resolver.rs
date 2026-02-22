use std::path::{Path, PathBuf};

use rquickjs::{Ctx, Error, Result};

/// Resolves `k8s/*` imports to generated `.js` files under `.husako/types/k8s/`.
pub struct HusakoK8sResolver {
    generated_types_dir: Option<PathBuf>,
}

impl HusakoK8sResolver {
    pub fn new(generated_types_dir: Option<PathBuf>) -> Self {
        Self {
            generated_types_dir,
        }
    }
}

impl rquickjs::loader::Resolver for HusakoK8sResolver {
    fn resolve<'js>(&mut self, _ctx: &Ctx<'js>, base: &str, name: &str) -> Result<String> {
        // Only handle k8s/* imports
        if !name.starts_with("k8s/") {
            return Err(Error::new_resolving(base, name));
        }

        let Some(types_dir) = &self.generated_types_dir else {
            return Err(Error::new_resolving_message(
                base,
                name,
                "k8s modules require 'husako generate' to be run first".to_string(),
            ));
        };

        let js_path = types_dir.join(format!("{name}.js"));

        if js_path.is_file() {
            Ok(js_path.to_string_lossy().into_owned())
        } else {
            Err(Error::new_resolving_message(
                base,
                name,
                format!(
                    "module '{}' not found. Run 'husako generate' to generate k8s modules",
                    name
                ),
            ))
        }
    }
}

pub struct HusakoFileResolver {
    project_root: PathBuf,
    allow_outside_root: bool,
    entry_dir: PathBuf,
}

impl HusakoFileResolver {
    pub fn new(project_root: &Path, allow_outside_root: bool, entry_path: &Path) -> Self {
        let entry_dir = entry_path.parent().unwrap_or(entry_path).to_path_buf();
        Self {
            project_root: project_root.to_path_buf(),
            allow_outside_root,
            entry_dir,
        }
    }
}

/// Try candidate path with extension inference.
/// Order: exact, .ts, .js, /index.ts, /index.js
fn resolve_with_extensions(candidate: &Path) -> Option<PathBuf> {
    // Exact match (already has extension)
    if candidate.is_file() {
        return candidate.canonicalize().ok();
    }

    let with_ts = candidate.with_extension("ts");
    if with_ts.is_file() {
        return with_ts.canonicalize().ok();
    }

    let with_js = candidate.with_extension("js");
    if with_js.is_file() {
        return with_js.canonicalize().ok();
    }

    let index_ts = candidate.join("index.ts");
    if index_ts.is_file() {
        return index_ts.canonicalize().ok();
    }

    let index_js = candidate.join("index.js");
    if index_js.is_file() {
        return index_js.canonicalize().ok();
    }

    None
}

impl rquickjs::loader::Resolver for HusakoFileResolver {
    fn resolve<'js>(&mut self, _ctx: &Ctx<'js>, base: &str, name: &str) -> Result<String> {
        // Only handle relative imports
        if !name.starts_with("./") && !name.starts_with("../") {
            return Err(Error::new_resolving(base, name));
        }

        // When base is "main" (the entry module), use the entry file's directory
        let base_dir = if base == "main" {
            self.entry_dir.clone()
        } else {
            Path::new(base)
                .parent()
                .unwrap_or(Path::new("."))
                .to_path_buf()
        };

        let candidate = base_dir.join(name);

        let resolved =
            resolve_with_extensions(&candidate).ok_or_else(|| Error::new_resolving(base, name))?;

        // Boundary check
        if !self.allow_outside_root && !resolved.starts_with(&self.project_root) {
            return Err(Error::new_resolving_message(
                base,
                name,
                format!(
                    "import {} is outside project root (use --allow-outside-root to override)",
                    resolved.display()
                ),
            ));
        }

        Ok(resolved.to_string_lossy().into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rquickjs::loader::Resolver;
    use std::fs;

    // --- HusakoK8sResolver tests ---

    #[test]
    fn k8s_resolver_resolves_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let types_dir = dir.path().join("k8s/apps");
        fs::create_dir_all(&types_dir).unwrap();
        fs::write(types_dir.join("v1.js"), "export class Deployment {}").unwrap();

        let mut resolver = HusakoK8sResolver::new(Some(dir.path().to_path_buf()));
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let result = resolver.resolve(&ctx, "main", "k8s/apps/v1").unwrap();
            assert!(result.ends_with("k8s/apps/v1.js"));
        });
    }

    #[test]
    fn k8s_resolver_error_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();

        let mut resolver = HusakoK8sResolver::new(Some(dir.path().to_path_buf()));
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let result = resolver.resolve(&ctx, "main", "k8s/apps/v1");
            assert!(result.is_err());
        });
    }

    #[test]
    fn k8s_resolver_error_when_no_types_dir() {
        let mut resolver = HusakoK8sResolver::new(None);
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let result = resolver.resolve(&ctx, "main", "k8s/apps/v1");
            assert!(result.is_err());
        });
    }

    #[test]
    fn k8s_resolver_ignores_non_k8s() {
        let mut resolver = HusakoK8sResolver::new(Some(PathBuf::from("/tmp")));
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let result = resolver.resolve(&ctx, "main", "husako");
            assert!(result.is_err());
        });
    }

    #[test]
    fn k8s_resolver_crd_path() {
        let dir = tempfile::tempdir().unwrap();
        let types_dir = dir.path().join("k8s/postgresql.cnpg.io");
        fs::create_dir_all(&types_dir).unwrap();
        fs::write(types_dir.join("v1.js"), "export class Cluster {}").unwrap();

        let mut resolver = HusakoK8sResolver::new(Some(dir.path().to_path_buf()));
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let result = resolver
                .resolve(&ctx, "main", "k8s/postgresql.cnpg.io/v1")
                .unwrap();
            assert!(result.ends_with("k8s/postgresql.cnpg.io/v1.js"));
        });
    }

    // --- HusakoFileResolver tests ---

    #[test]
    fn resolve_ts_extension() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::write(root.join("helper.ts"), "export const x = 1;").unwrap();

        let entry = root.join("main.ts");
        fs::write(&entry, "").unwrap();

        let mut resolver = HusakoFileResolver::new(&root, false, &entry);
        let ctx_guard = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&ctx_guard).unwrap();
        ctx.with(|ctx| {
            let result = resolver.resolve(&ctx, "main", "./helper").unwrap();
            assert!(result.ends_with("helper.ts"));
        });
    }

    #[test]
    fn resolve_index_ts() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::create_dir(root.join("lib")).unwrap();
        fs::write(root.join("lib/index.ts"), "export const x = 1;").unwrap();

        let entry = root.join("main.ts");
        fs::write(&entry, "").unwrap();

        let mut resolver = HusakoFileResolver::new(&root, false, &entry);
        let ctx_guard = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&ctx_guard).unwrap();
        ctx.with(|ctx| {
            let result = resolver.resolve(&ctx, "main", "./lib").unwrap();
            assert!(result.ends_with("lib/index.ts"));
        });
    }

    #[test]
    fn reject_outside_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let sub = root.join("sub");
        fs::create_dir(&sub).unwrap();

        // Create a file outside the sub directory
        fs::write(root.join("outside.ts"), "export const x = 1;").unwrap();

        let entry = sub.join("main.ts");
        fs::write(&entry, "").unwrap();

        let mut resolver = HusakoFileResolver::new(&sub, false, &entry);
        let ctx_guard = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&ctx_guard).unwrap();
        ctx.with(|ctx| {
            let result = resolver.resolve(&ctx, "main", "../outside");
            assert!(result.is_err());
        });
    }

    #[test]
    fn allow_outside_root_flag() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let sub = root.join("sub");
        fs::create_dir(&sub).unwrap();

        fs::write(root.join("outside.ts"), "export const x = 1;").unwrap();

        let entry = sub.join("main.ts");
        fs::write(&entry, "").unwrap();

        let mut resolver = HusakoFileResolver::new(&sub, true, &entry);
        let ctx_guard = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&ctx_guard).unwrap();
        ctx.with(|ctx| {
            let result = resolver.resolve(&ctx, "main", "../outside").unwrap();
            assert!(result.ends_with("outside.ts"));
        });
    }

    #[test]
    fn skip_non_relative() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let entry = root.join("main.ts");

        let mut resolver = HusakoFileResolver::new(&root, false, &entry);
        let ctx_guard = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&ctx_guard).unwrap();
        ctx.with(|ctx| {
            let result = resolver.resolve(&ctx, "main", "husako");
            assert!(result.is_err());
        });
    }
}
