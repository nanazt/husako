use std::io::Write;

use assert_cmd::cargo::cargo_bin_cmd;
use tempfile::NamedTempFile;

fn husako() -> assert_cmd::Command {
    cargo_bin_cmd!("husako")
}

fn husako_at(dir: &std::path::Path) -> assert_cmd::Command {
    let mut cmd = cargo_bin_cmd!("husako");
    cmd.current_dir(dir);
    cmd
}

fn write_temp_ts(content: &str) -> NamedTempFile {
    let mut f = tempfile::Builder::new().suffix(".ts").tempfile().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f
}

#[test]
fn render_basic() {
    husako()
        .args(["render", "../../examples/basic.ts"])
        .assert()
        .success()
        .stdout(predicates::str::contains("apiVersion: apps/v1"))
        .stdout(predicates::str::contains("kind: Deployment"))
        .stdout(predicates::str::contains("name: nginx"));
}

#[test]
fn render_basic_yaml_snapshot() {
    let output = husako()
        .args(["render", "../../examples/basic.ts"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let yaml = String::from_utf8(output.stdout).unwrap();
    insta::assert_snapshot!(yaml);
}

#[test]
fn missing_build() {
    let f = write_temp_ts(r#"import { build } from "husako"; const x = 1;"#);
    husako()
        .args(["render", f.path().to_str().unwrap()])
        .assert()
        .code(7)
        .stderr(predicates::str::contains("build() was not called"));
}

#[test]
fn double_build() {
    let f = write_temp_ts(r#"import { build } from "husako"; build([]); build([]);"#);
    husako()
        .args(["render", f.path().to_str().unwrap()])
        .assert()
        .code(7)
        .stderr(predicates::str::contains("called 2 times"));
}

#[test]
fn strict_json_undefined() {
    let f = write_temp_ts(r#"import { build } from "husako"; build({ a: undefined });"#);
    husako()
        .args(["render", f.path().to_str().unwrap()])
        .assert()
        .code(7)
        .stderr(predicates::str::contains("undefined"));
}

#[test]
fn strict_json_function() {
    let f = write_temp_ts(r#"import { build } from "husako"; build({ fn: () => {} });"#);
    husako()
        .args(["render", f.path().to_str().unwrap()])
        .assert()
        .code(7)
        .stderr(predicates::str::contains("function"));
}

#[test]
fn compile_error() {
    let f = write_temp_ts("const = ;");
    husako()
        .args(["render", f.path().to_str().unwrap()])
        .assert()
        .code(3);
}

// --- Milestone 2: Module Loader + Project Imports ---

fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn render_project_imports() {
    let root = repo_root();
    husako_at(&root)
        .args(["render", "examples/project/env/dev.ts"])
        .assert()
        .success()
        .stdout(predicates::str::contains("apiVersion: apps/v1"))
        .stdout(predicates::str::contains("kind: Deployment"))
        .stdout(predicates::str::contains("name: nginx"))
        .stdout(predicates::str::contains("app: nginx"));
}

#[test]
fn render_project_snapshot() {
    let root = repo_root();
    let output = husako_at(&root)
        .args(["render", "examples/project/env/dev.ts"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let yaml = String::from_utf8(output.stdout).unwrap();
    insta::assert_snapshot!(yaml);
}

#[test]
fn reject_outside_root() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let sub = root.join("sub");
    std::fs::create_dir(&sub).unwrap();

    // Create a file outside the sub directory
    std::fs::write(root.join("secret.ts"), "export const x = 1;").unwrap();

    // Create entry file that imports outside root
    let entry = sub.join("entry.ts");
    std::fs::write(
        &entry,
        r#"import { build } from "husako"; import { x } from "../secret"; build([{ v: x }]);"#,
    )
    .unwrap();

    // Run with cwd=sub so project_root=sub
    husako_at(&sub)
        .args(["render", entry.to_str().unwrap()])
        .assert()
        .code(4)
        .stderr(predicates::str::contains("outside project root"));
}

#[test]
fn allow_outside_root_flag() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let sub = root.join("sub");
    std::fs::create_dir(&sub).unwrap();

    std::fs::write(root.join("outside.ts"), "export const val = 42;").unwrap();

    let entry = sub.join("entry.ts");
    std::fs::write(
        &entry,
        r#"import { build } from "husako"; import { val } from "../outside"; build([{ v: val }]);"#,
    )
    .unwrap();

    // With --allow-outside-root, the boundary check is bypassed
    husako_at(&sub)
        .args(["render", "--allow-outside-root", entry.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("v: 42"));
}

#[test]
fn extension_inference() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // import "./lib" should resolve to lib.ts
    std::fs::write(root.join("lib.ts"), "export const x: number = 1;").unwrap();
    let entry = root.join("entry.ts");
    std::fs::write(
        &entry,
        r#"import { build } from "husako"; import { x } from "./lib"; build([{ v: x }]);"#,
    )
    .unwrap();

    husako_at(root)
        .args(["render", entry.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("v: 1"));
}

#[test]
fn index_inference() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // import "./lib" should resolve to lib/index.ts
    let lib = root.join("lib");
    std::fs::create_dir(&lib).unwrap();
    std::fs::write(lib.join("index.ts"), "export const x: number = 99;").unwrap();

    let entry = root.join("entry.ts");
    std::fs::write(
        &entry,
        r#"import { build } from "husako"; import { x } from "./lib"; build([{ v: x }]);"#,
    )
    .unwrap();

    husako_at(root)
        .args(["render", entry.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("v: 99"));
}
