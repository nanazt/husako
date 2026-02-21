use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// Root of the fixtures directory.
fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("openapi")
}

/// Load all k8s fixture specs into a HashMap suitable for `GenerateOptions`.
///
/// Scans the `k8s/` fixture directory for `.json` files, mirroring
/// `scan_spec_files` in `husako-openapi`.
pub fn load_k8s_fixtures() -> HashMap<String, Value> {
    let dir = fixtures_dir().join("k8s");
    scan_json_files(&dir, &dir)
}

/// Load CRD fixtures for a specific project (e.g. "cert-manager", "fluxcd", "cnpg").
pub fn load_crd_fixtures(name: &str) -> HashMap<String, Value> {
    let dir = fixtures_dir().join("crds").join(name);
    scan_json_files(&dir, &dir)
}

/// Recursively scan a directory for `.json` files, building a discovery-key map.
///
/// Discovery keys are relative paths without the `.json` extension,
/// e.g. `"apis/apps/v1"`.
fn scan_json_files(base: &Path, dir: &Path) -> HashMap<String, Value> {
    let mut result = HashMap::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return result,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            result.extend(scan_json_files(base, &path));
        } else if path.extension().is_some_and(|ext| ext == "json") {
            let rel = path.strip_prefix(base).expect("path should be under base");
            let key = rel
                .to_str()
                .expect("path should be valid UTF-8")
                .strip_suffix(".json")
                .unwrap_or_else(|| rel.to_str().unwrap());

            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
            let value: Value = serde_json::from_str(&content)
                .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));
            result.insert(key.to_string(), value);
        }
    }

    result
}
