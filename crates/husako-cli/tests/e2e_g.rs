mod e2e_common;
use e2e_common::*;

// ── Scenario G: husako test command ──────────────────────────────────────────
//
// All tests are local-only (no network required). Each test gets its own
// isolated TempDir created inside test/e2e/ to avoid macOS /tmp symlink issues.

/// G1: All tests pass → exit 0, output contains "passed".
#[test]
fn g1_passing_tests_exit_zero() {
    let dir = e2e_tmpdir();
    init_project(dir.path(), "");
    husako_at(dir.path())
        .args(["gen", "--skip-k8s"])
        .assert()
        .success();

    std::fs::write(
        dir.path().join("calc.ts"),
        "export function add(a: number, b: number): number { return a + b; }\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("calc.test.ts"),
        r#"import { describe, test, expect } from "husako/test";
import { add } from "./calc";
describe("add", () => {
  test("two plus two", () => { expect(add(2, 2)).toBe(4); });
  test("toEqual", () => { expect({ x: 1 }).toEqual({ x: 1 }); });
});
"#,
    )
    .unwrap();

    let output = husako_at(dir.path())
        .args(["test", "calc.test.ts"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "G1: expected exit 0, got {:?}\n{}",
        output.status.code(),
        output_combined(&output)
    );
    assert_contains(
        "G1: output contains 'passed'",
        "passed",
        &output_combined(&output),
    );
}

/// G2: A failing assertion → exit 1, output contains "failed".
#[test]
fn g2_failing_test_exit_one() {
    let dir = e2e_tmpdir();
    init_project(dir.path(), "");
    husako_at(dir.path())
        .args(["gen", "--skip-k8s"])
        .assert()
        .success();

    std::fs::write(
        dir.path().join("fail.test.ts"),
        r#"import { test, expect } from "husako/test";
test("will fail", () => { expect(1).toBe(999); });
"#,
    )
    .unwrap();

    let output = husako_at(dir.path())
        .args(["test", "fail.test.ts"])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(1),
        "G2: expected exit 1, got {:?}\n{}",
        output.status.code(),
        output_combined(&output)
    );
    assert_contains(
        "G2: output contains 'failed'",
        "failed",
        &output_combined(&output),
    );
}

/// G3: Auto-discovery — `husako test` with no args finds all `*.test.ts` recursively.
#[test]
fn g3_auto_discovery() {
    let dir = e2e_tmpdir();
    init_project(dir.path(), "");
    husako_at(dir.path())
        .args(["gen", "--skip-k8s"])
        .assert()
        .success();

    // Set up all three test files (simulating state from G1 + G2 + new subdir file)
    std::fs::write(
        dir.path().join("calc.ts"),
        "export function add(a: number, b: number): number { return a + b; }\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("calc.test.ts"),
        r#"import { describe, test, expect } from "husako/test";
import { add } from "./calc";
describe("add", () => {
  test("two plus two", () => { expect(add(2, 2)).toBe(4); });
});
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("fail.test.ts"),
        r#"import { test, expect } from "husako/test";
test("will fail", () => { expect(1).toBe(999); });
"#,
    )
    .unwrap();
    std::fs::create_dir(dir.path().join("subdir")).unwrap();
    std::fs::write(
        dir.path().join("subdir/extra.test.ts"),
        r#"import { test, expect } from "husako/test";
test("in subdir", () => { expect(true).toBeTruthy(); });
"#,
    )
    .unwrap();

    // Exit code may be non-zero due to fail.test.ts — we only check file discovery
    let output = husako_at(dir.path()).args(["test"]).output().unwrap();
    let combined = output_combined(&output);
    assert_contains("G3: found calc.test.ts", "calc.test.ts", &combined);
    assert_contains("G3: found fail.test.ts", "fail.test.ts", &combined);
    assert_contains("G3: found extra.test.ts", "extra.test.ts", &combined);
}

/// G4: Plugin module can be imported from a test file.
#[test]
fn g4_plugin_testing() {
    let dir = e2e_tmpdir();
    init_project(dir.path(), "");
    husako_at(dir.path())
        .args(["gen", "--skip-k8s"])
        .assert()
        .success();

    // Create plugin directory
    std::fs::create_dir(dir.path().join("myplugin")).unwrap();
    std::fs::write(
        dir.path().join("myplugin/plugin.toml"),
        "[plugin]\nname = \"myplugin\"\nversion = \"0.1.0\"\n\n[modules]\nmyplugin = \"index.js\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("myplugin/index.js"),
        "export function greet(name) { return \"Hello, \" + name + \"!\"; }\n",
    )
    .unwrap();

    // Write husako.toml with plugin entry
    std::fs::write(
        dir.path().join("husako.toml"),
        "[plugins]\nmyplugin = { source = \"path\", path = \"./myplugin\" }\n",
    )
    .unwrap();
    husako_at(dir.path())
        .args(["gen", "--skip-k8s"])
        .assert()
        .success();

    std::fs::write(
        dir.path().join("plugin.test.ts"),
        r#"import { test, expect } from "husako/test";
import { greet } from "myplugin";
test("greet", () => { expect(greet("World")).toBe("Hello, World!"); });
"#,
    )
    .unwrap();

    let output = husako_at(dir.path())
        .args(["test", "plugin.test.ts"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "G4: plugin test failed (exit {:?})\n{}",
        output.status.code(),
        output_combined(&output)
    );
    assert_contains(
        "G4: output contains 'passed'",
        "passed",
        &output_combined(&output),
    );
}
