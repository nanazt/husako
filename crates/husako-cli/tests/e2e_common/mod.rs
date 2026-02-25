#![allow(dead_code)]
/// Shared helpers for e2e test scenarios.
///
/// Used by e2e_a.rs through e2e_g.rs. Each scenario file declares `mod e2e_common;`
/// to access these functions.
use assert_cmd::cargo::cargo_bin_cmd;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// ── Binary ────────────────────────────────────────────────────────────────────

pub fn husako_at(dir: &Path) -> assert_cmd::Command {
    let mut cmd = cargo_bin_cmd!("husako");
    cmd.current_dir(dir);
    cmd
}

// ── Paths ─────────────────────────────────────────────────────────────────────

/// Absolute path to the `test/e2e/` fixture directory.
pub fn e2e_fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test/e2e")
        .canonicalize()
        .expect("test/e2e/ must exist")
}

/// Create a TempDir inside `test/e2e/` to avoid macOS `/tmp` → `/private/tmp`
/// symlink mismatch when husako checks project-root boundaries.
pub fn e2e_tmpdir() -> TempDir {
    tempfile::Builder::new()
        .prefix("tmp.")
        .tempdir_in(e2e_fixtures_dir())
        .expect("failed to create tmpdir inside test/e2e/")
}

// ── Filesystem utilities ──────────────────────────────────────────────────────

/// Recursively copy a directory tree from `src` to `dst`.
pub fn copy_dir_all(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let ty = entry.file_type().unwrap();
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name())).unwrap();
        }
    }
}

// ── Project setup ─────────────────────────────────────────────────────────────

/// Write `husako.toml` with the given content (empty → `[entries]\n`) and copy
/// `tsconfig.json` from the fixture directory into `dir`.
pub fn init_project(dir: &Path, toml_content: &str) {
    let content = format!("{}\n", toml_content);
    std::fs::write(dir.join("husako.toml"), content).unwrap();
    std::fs::copy(
        e2e_fixtures_dir().join("tsconfig.json"),
        dir.join("tsconfig.json"),
    )
    .unwrap();
}

/// Write a minimal ConfigMap TypeScript entry to `path`.
pub fn write_configmap(path: &Path) {
    std::fs::write(
        path,
        r#"import { ConfigMap } from "k8s/core/v1";
import { metadata, build } from "husako";
const cm = ConfigMap()
  .metadata(metadata().name("test-cm").namespace("default"))
  .set("data", { key: "value" });
build([cm]);
"#,
    )
    .unwrap();
}

// ── Output helpers ────────────────────────────────────────────────────────────

/// Combine stdout + stderr into one string for assertion (husako writes
/// diagnostics to stderr and YAML to stdout).
pub fn output_combined(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

// ── Assertions ────────────────────────────────────────────────────────────────

#[track_caller]
pub fn assert_contains(desc: &str, pattern: &str, content: &str) {
    assert!(
        content.contains(pattern),
        "{desc}: expected {pattern:?} in output:\n{content}"
    );
}

#[track_caller]
pub fn assert_not_contains(desc: &str, pattern: &str, content: &str) {
    assert!(
        !content.contains(pattern),
        "{desc}: unexpected {pattern:?} in output:\n{content}"
    );
}

#[track_caller]
pub fn assert_file(path: &Path) {
    assert!(path.is_file(), "expected file to exist: {}", path.display());
}

#[track_caller]
pub fn assert_no_dir(path: &Path) {
    assert!(
        !path.is_dir(),
        "expected directory to be absent: {}",
        path.display()
    );
}

/// Assert a `.d.ts` file exports the given symbol name.
#[track_caller]
pub fn assert_dts_exports(dts_path: &Path, symbol: &str) {
    let content = std::fs::read_to_string(dts_path)
        .unwrap_or_else(|_| panic!("could not read {}", dts_path.display()));
    assert!(
        content.contains("export") && content.contains(symbol),
        "{}: missing export {symbol:?}",
        dts_path.display()
    );
}

/// Assert `husako.toml` in `dir` has a line containing both `field` and `value`.
#[track_caller]
pub fn assert_toml_field(dir: &Path, field: &str, value: &str, desc: &str) {
    let content = std::fs::read_to_string(dir.join("husako.toml")).expect("husako.toml must exist");
    let found = content
        .lines()
        .any(|l| l.contains(field) && l.contains(value));
    assert!(
        found,
        "{desc}: husako.toml has no line with {field:?} and {value:?}\n{content}"
    );
}

/// Assert that `husako.toml` in `dir` does NOT have a top-level `key = ...` line.
#[track_caller]
pub fn assert_toml_key_absent(dir: &Path, key: &str) {
    let content = std::fs::read_to_string(dir.join("husako.toml")).expect("husako.toml must exist");
    let found = content.lines().any(|l| {
        let t = l.trim_start();
        t.starts_with(key) && t[key.len()..].trim_start().starts_with('=')
    });
    assert!(
        !found,
        "key {key:?} should be absent from husako.toml:\n{content}"
    );
}

/// Validate YAML is structurally valid using `serde_yaml_ng`.
/// Supports multi-document YAML (separated by `---`).
#[track_caller]
pub fn assert_valid_yaml(yaml: &str, desc: &str) {
    // Split on document boundaries; husako render may emit multiple `---` docs.
    for part in yaml.split("\n---").map(str::trim).filter(|s| !s.is_empty()) {
        serde_yaml_ng::from_str::<serde_yaml_ng::Value>(part)
            .unwrap_or_else(|e| panic!("{desc}: invalid YAML: {e}\n---\n{part}"));
    }
}

/// Validate standard k8s YAML via `kubeconform -strict`.
/// Skips gracefully when `kubeconform` is not in PATH (local dev without it installed).
#[track_caller]
pub fn assert_k8s_valid(yaml: &str, desc: &str) {
    if !kubeconform_available() {
        return;
    }
    use std::io::Write as _;
    let mut child = std::process::Command::new("kubeconform")
        .arg("-strict")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn kubeconform");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(yaml.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "k8s validate failed for {desc}:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn kubeconform_available() -> bool {
    std::process::Command::new("kubeconform")
        .arg("-v")
        .output()
        .is_ok()
}
