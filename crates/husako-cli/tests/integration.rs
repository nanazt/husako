use std::io::Write;
use std::path::Path;

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

// --- Milestone 3: SDK Builders ---

#[test]
fn render_canonical() {
    let root = repo_root();
    husako_at(&root)
        .args(["render", "examples/canonical.ts"])
        .assert()
        .success()
        .stdout(predicates::str::contains("apiVersion: apps/v1"))
        .stdout(predicates::str::contains("kind: Deployment"))
        .stdout(predicates::str::contains("name: nginx"))
        .stdout(predicates::str::contains("namespace: nginx-ns"))
        .stdout(predicates::str::contains("key1: value1"))
        .stdout(predicates::str::contains("key5: value5"))
        .stdout(predicates::str::contains("cpu: '1'"))
        .stdout(predicates::str::contains("memory: 2Gi"))
        .stdout(predicates::str::contains("cpu: 500m"))
        .stdout(predicates::str::contains("memory: 1Gi"));
}

#[test]
fn render_canonical_snapshot() {
    let root = repo_root();
    let output = husako_at(&root)
        .args(["render", "examples/canonical.ts"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let yaml = String::from_utf8(output.stdout).unwrap();
    insta::assert_snapshot!(yaml);
}

#[test]
fn metadata_fragment_reuse() {
    let f = write_temp_ts(
        r#"
import { build, label } from "husako";
import { Deployment } from "k8s/apps/v1";
const base = label("env", "dev");
const a = base.label("team", "a");
const b = base.label("team", "b");
const da = new Deployment().metadata(a);
const db = new Deployment().metadata(b);
build([da, db]);
"#,
    );
    let output = husako()
        .args(["render", f.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    let yaml = String::from_utf8(output.stdout).unwrap();
    // Both should have env: dev
    assert_eq!(yaml.matches("env: dev").count(), 2);
    // a should have team: a, b should have team: b
    assert!(yaml.contains("team: a"));
    assert!(yaml.contains("team: b"));
}

#[test]
fn merge_labels_deep() {
    let f = write_temp_ts(
        r#"
import { build, name, label, merge } from "husako";
import { Deployment } from "k8s/apps/v1";
const m = merge([name("test"), label("a", "1"), label("b", "2"), label("c", "3")]);
const d = new Deployment().metadata(m);
build([d]);
"#,
    );
    husako()
        .args(["render", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("name: test"))
        .stdout(predicates::str::contains("a: '1'"))
        .stdout(predicates::str::contains("b: '2'"))
        .stdout(predicates::str::contains("c: '3'"));
}

#[test]
fn cpu_normalization() {
    let f = write_temp_ts(
        r#"
import { build, cpu, requests } from "husako";
import { Deployment } from "k8s/apps/v1";
const d = new Deployment().resources(requests(cpu(0.5)));
build([d]);
"#,
    );
    husako()
        .args(["render", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("cpu: 500m"));
}

#[test]
fn memory_normalization() {
    let f = write_temp_ts(
        r#"
import { build, memory, requests } from "husako";
import { Deployment } from "k8s/apps/v1";
const d = new Deployment().resources(requests(memory(4)));
build([d]);
"#,
    );
    husako()
        .args(["render", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("memory: 4Gi"));
}

#[test]
fn k8s_core_v1_namespace() {
    let f = write_temp_ts(
        r#"
import { build, name } from "husako";
import { Namespace } from "k8s/core/v1";
const ns = new Namespace().metadata(name("my-ns"));
build([ns]);
"#,
    );
    husako()
        .args(["render", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("apiVersion: v1"))
        .stdout(predicates::str::contains("kind: Namespace"))
        .stdout(predicates::str::contains("name: my-ns"));
}

#[test]
fn backward_compat_plain_objects() {
    let f = write_temp_ts(
        r#"
import { build } from "husako";
build([{ apiVersion: "v1", kind: "Namespace", metadata: { name: "test" } }]);
"#,
    );
    husako()
        .args(["render", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("apiVersion: v1"))
        .stdout(predicates::str::contains("kind: Namespace"))
        .stdout(predicates::str::contains("name: test"));
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

// --- Milestone 6: Schema-aware Quantity Validation ---

#[test]
fn invalid_quantity_fallback_exit_7() {
    let f = write_temp_ts(
        r#"
import { build } from "husako";
build([{
    apiVersion: "apps/v1",
    kind: "Deployment",
    spec: {
        template: {
            spec: {
                containers: [{
                    resources: {
                        requests: { cpu: "2gb" }
                    }
                }]
            }
        }
    }
}]);
"#,
    );
    husako()
        .args(["render", f.path().to_str().unwrap()])
        .assert()
        .code(7)
        .stderr(predicates::str::contains("invalid quantity"))
        .stderr(predicates::str::contains("2gb"));
}

#[test]
fn valid_quantities_exit_0() {
    let f = write_temp_ts(
        r#"
import { build } from "husako";
build([{
    apiVersion: "apps/v1",
    kind: "Deployment",
    spec: {
        template: {
            spec: {
                containers: [{
                    resources: {
                        requests: { cpu: "500m", memory: "1Gi" },
                        limits: { cpu: "1", memory: "2Gi" }
                    }
                }]
            }
        }
    }
}]);
"#,
    );
    husako()
        .args(["render", f.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn numbers_at_quantity_positions_exit_0() {
    let f = write_temp_ts(
        r#"
import { build } from "husako";
build([{
    apiVersion: "apps/v1",
    kind: "Deployment",
    spec: {
        template: {
            spec: {
                containers: [{
                    resources: {
                        requests: { cpu: 1, memory: 2 }
                    }
                }]
            }
        }
    }
}]);
"#,
    );
    husako()
        .args(["render", f.path().to_str().unwrap()])
        .assert()
        .success();
}

// --- Milestone 5: Type Generation + husako init ---

#[test]
fn init_skip_k8s() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    husako_at(root)
        .args(["init", "--skip-k8s"])
        .assert()
        .success();

    // Static .d.ts files should exist
    assert!(root.join(".husako/types/husako.d.ts").exists());
    assert!(root.join(".husako/types/husako/_base.d.ts").exists());

    // tsconfig.json should exist with husako paths
    let tsconfig = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&tsconfig).unwrap();
    assert!(parsed["compilerOptions"]["paths"]["husako"].is_array());
    assert!(parsed["compilerOptions"]["paths"]["k8s/*"].is_array());

    // No k8s/ directory since we skipped k8s types
    assert!(!root.join(".husako/types/k8s").exists());
}

fn write_mock_spec(dir: &Path, group_path: &str) {
    let spec = serde_json::json!({
        "components": {
            "schemas": {
                "io.k8s.api.apps.v1.Deployment": {
                    "description": "Deployment enables declarative updates.",
                    "properties": {
                        "apiVersion": {"type": "string"},
                        "kind": {"type": "string"},
                        "metadata": {"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta"},
                        "spec": {"$ref": "#/components/schemas/io.k8s.api.apps.v1.DeploymentSpec"}
                    },
                    "x-kubernetes-group-version-kind": [
                        {"group": "apps", "version": "v1", "kind": "Deployment"}
                    ]
                },
                "io.k8s.api.apps.v1.DeploymentSpec": {
                    "properties": {
                        "replicas": {"type": "integer"}
                    }
                },
                "io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta": {
                    "description": "Standard object metadata.",
                    "properties": {
                        "name": {"type": "string"},
                        "namespace": {"type": "string"}
                    }
                }
            }
        }
    });

    let spec_path = dir.join(format!("{group_path}.json"));
    std::fs::create_dir_all(spec_path.parent().unwrap()).unwrap();
    std::fs::write(spec_path, spec.to_string()).unwrap();
}

#[test]
fn init_spec_dir() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Create mock spec directory
    let spec_dir = root.join("specs");
    std::fs::create_dir_all(&spec_dir).unwrap();
    write_mock_spec(&spec_dir, "apis/apps/v1");

    husako_at(root)
        .args(["init", "--spec-dir", spec_dir.to_str().unwrap()])
        .assert()
        .success();

    // Static .d.ts files should exist
    assert!(root.join(".husako/types/husako.d.ts").exists());
    assert!(root.join(".husako/types/husako/_base.d.ts").exists());

    // Generated k8s types should exist
    assert!(root.join(".husako/types/k8s/_common.d.ts").exists());
    assert!(root.join(".husako/types/k8s/apps/v1.d.ts").exists());

    // Content should contain Deployment
    let apps_v1 = std::fs::read_to_string(root.join(".husako/types/k8s/apps/v1.d.ts")).unwrap();
    assert!(apps_v1.contains("class Deployment"));
    assert!(apps_v1.contains("_ResourceBuilder"));

    // tsconfig.json should exist
    assert!(root.join("tsconfig.json").exists());
}

#[test]
fn init_updates_existing_tsconfig() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

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

    husako_at(root)
        .args(["init", "--skip-k8s"])
        .assert()
        .success();

    let tsconfig = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&tsconfig).unwrap();

    // Original fields preserved
    assert_eq!(parsed["compilerOptions"]["target"], "ES2020");
    assert!(parsed["include"].is_array());

    // Original path preserved
    assert!(parsed["compilerOptions"]["paths"]["mylib/*"].is_array());

    // husako paths added
    assert!(parsed["compilerOptions"]["paths"]["husako"].is_array());
    assert!(parsed["compilerOptions"]["paths"]["husako/_base"].is_array());
    assert!(parsed["compilerOptions"]["paths"]["k8s/*"].is_array());
}
