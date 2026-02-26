mod e2e_common;
use e2e_common::*;

// ── Scenario E: Plugin system + husako clean ─────────────────────────────────

#[test]
#[ignore] // requires network (k8s gen); run with: cargo test -p husako -- --include-ignored
fn scenario_e_plugin_system_and_clean() {
    let dir = tempfile::TempDir::new().unwrap();
    init_project(
        dir.path(),
        "[resources]\nk8s = { source = \"release\", version = \"1.35\" }",
    );

    // ── E1: plugin add (path source — bundled FluxCD plugin) ─────────────────
    // Copy the FluxCD plugin from the repo into the tmpdir as a relative path
    let plugin_src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/fluxcd")
        .canonicalize()
        .expect("plugins/fluxcd must exist");
    let plugin_dst = dir.path().join("fluxcd-plugin");
    copy_dir_all(&plugin_src, &plugin_dst);

    husako_at(dir.path())
        .args(["plugin", "add", "fluxcd", "--path", "fluxcd-plugin"])
        .assert()
        .success();

    // Side-effect: husako.toml has fluxcd plugin entry
    assert_toml_field(dir.path(), "source", "path", "fluxcd plugin source=path");
    let toml = std::fs::read_to_string(dir.path().join("husako.toml")).unwrap();
    assert!(toml.contains("fluxcd"), "fluxcd should be in husako.toml");

    husako_at(dir.path()).args(["gen"]).assert().success();

    // plugin list shows fluxcd (output on stderr)
    let plugin_list = husako_at(dir.path())
        .args(["plugin", "list"])
        .output()
        .unwrap();
    let plugin_list_out = String::from_utf8_lossy(&plugin_list.stderr).to_string();
    assert_contains("plugin list shows fluxcd", "fluxcd", &plugin_list_out);

    // Plugin module files installed
    assert_file(&dir.path().join(".husako/plugins/fluxcd/modules/index.js"));

    std::fs::write(
        dir.path().join("helmrelease.ts"),
        r#"import { HelmRelease } from "fluxcd";
import { HelmRepository } from "fluxcd/source";
import { metadata, build } from "husako";

const repo = HelmRepository()
  .metadata(metadata().name("bitnami").namespace("flux-system"))
  .spec({ url: "https://charts.bitnami.com/bitnami", interval: "1h" });

const release = HelmRelease()
  .metadata(metadata().name("redis").namespace("default"))
  .spec({
    chart: { spec: { chart: "redis", version: "25.3.0", sourceRef: repo._sourceRef() } },
    interval: "10m",
  });

build([repo, release]);
"#,
    )
    .unwrap();
    husako_at(dir.path())
        .args(["check", "helmrelease.ts"])
        .assert()
        .success();
    let hr_yaml = String::from_utf8_lossy(
        &husako_at(dir.path())
            .args(["render", "helmrelease.ts"])
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();
    assert_contains("render → HelmRelease", "kind: HelmRelease", &hr_yaml);
    assert_contains("render → HelmRepository", "kind: HelmRepository", &hr_yaml);
    assert_contains("render → bitnami repo URL", "charts.bitnami.com", &hr_yaml);
    assert_valid_yaml(&hr_yaml, "render helmrelease");

    // ── E2: plugin remove ────────────────────────────────────────────────────
    husako_at(dir.path())
        .args(["plugin", "remove", "fluxcd"])
        .assert()
        .success();
    assert_toml_key_absent(dir.path(), "fluxcd");

    let plugin_list_after = husako_at(dir.path())
        .args(["plugin", "list"])
        .output()
        .unwrap();
    let plugin_list_after_out = String::from_utf8_lossy(&plugin_list_after.stderr).to_string();
    assert_not_contains(
        "fluxcd not in plugin list",
        "fluxcd",
        &plugin_list_after_out,
    );

    assert_no_dir(&dir.path().join(".husako/plugins/fluxcd"));

    // ── E3: husako clean ──────────────────────────────────────────────────────
    // Re-gen so .husako/ exists for the clean test
    husako_at(dir.path()).args(["gen"]).assert().success();

    husako_at(dir.path())
        .args(["clean", "--all"])
        .assert()
        .success();
    assert_no_dir(&dir.path().join(".husako"));

    // gen after clean re-downloads and rebuilds everything
    husako_at(dir.path()).args(["gen"]).assert().success();
    assert_file(&dir.path().join(".husako/types/k8s/core/v1.d.ts"));

    write_configmap(&dir.path().join("configmap.ts"));
    husako_at(dir.path())
        .args(["check", "configmap.ts"])
        .assert()
        .success();
    let cm_yaml = String::from_utf8_lossy(
        &husako_at(dir.path())
            .args(["render", "configmap.ts"])
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();
    assert_k8s_valid(&cm_yaml, "ConfigMap after clean + gen");
}
