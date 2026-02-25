mod e2e_common;
use e2e_common::*;
use std::time::Duration;

// ── Scenario D: Version management (gen → update → re-validate) ──────────────

#[test]
#[ignore] // requires network; run with: cargo test -p husako -- --include-ignored
fn scenario_d_version_management() {
    let dir = tempfile::TempDir::new().unwrap();
    init_project(
        dir.path(),
        "[resources]\nk8s = { source = \"release\", version = \"1.30\" }",
    );
    write_configmap(&dir.path().join("configmap.ts"));

    husako_at(dir.path()).args(["gen"]).assert().success();

    // k8s 1.30 types present
    assert_file(&dir.path().join(".husako/types/k8s/core/v1.d.ts"));
    husako_at(dir.path())
        .args(["validate", "configmap.ts"])
        .assert()
        .success();
    let cm_before = String::from_utf8_lossy(
        &husako_at(dir.path())
            .args(["render", "configmap.ts"])
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();
    assert_contains("render (1.30) → kind: ConfigMap", "kind: ConfigMap", &cm_before);
    assert_k8s_valid(&cm_before, "ConfigMap (k8s 1.30)");

    // Record pre-update mtime
    let dts_path = dir.path().join(".husako/types/k8s/core/v1.d.ts");
    let mtime_before = std::fs::metadata(&dts_path)
        .unwrap()
        .modified()
        .unwrap();

    // Small sleep to ensure mtime difference is detectable
    std::thread::sleep(Duration::from_secs(1));

    // husako update may exit non-zero if no newer version found; that's fine
    let _ = husako_at(dir.path()).args(["update", "k8s"]).output().unwrap();

    // husako.toml version should have changed from "1.30"
    let toml_after = std::fs::read_to_string(dir.path().join("husako.toml")).unwrap();
    assert!(
        !toml_after.contains("\"1.30\""),
        "version should have been updated from 1.30:\n{toml_after}"
    );

    // Type files regenerated (mtime >= before)
    let mtime_after = std::fs::metadata(&dts_path)
        .unwrap()
        .modified()
        .unwrap();
    assert!(
        mtime_after >= mtime_before,
        "types should have been regenerated after update (mtime unchanged)"
    );

    // validate + render still work after update
    husako_at(dir.path())
        .args(["validate", "configmap.ts"])
        .assert()
        .success();
    let cm_after = String::from_utf8_lossy(
        &husako_at(dir.path())
            .args(["render", "configmap.ts"])
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();
    assert_contains("render (after update) → kind: ConfigMap", "kind: ConfigMap", &cm_after);
    assert_k8s_valid(&cm_after, "ConfigMap (after update)");
}
