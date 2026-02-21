use std::path::{Path, PathBuf};

use rquickjs::{Ctx, Error, Result};

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
