mod e2e_common;
use e2e_common::*;

// ── Scenario F: OCI chart source ─────────────────────────────────────────────

#[test]
#[ignore] // requires network; run with: cargo test -p husako -- --include-ignored
fn scenario_f_oci_chart_source() {
    let dir = tempfile::TempDir::new().unwrap();
    init_project(
        dir.path(),
        "[resources]\nk8s = { source = \"release\", version = \"1.35\" }",
    );

    // ── F1: add OCI chart via non-interactive flags ───────────────────────────
    husako_at(dir.path())
        .args([
            "-y",
            "add",
            "postgresql",
            "--chart",
            "--source",
            "oci",
            "--reference",
            "oci://registry-1.docker.io/bitnamicharts/postgresql",
            "--version",
            "18.4.0",
        ])
        .assert()
        .success();

    assert_toml_field(dir.path(), "source", "oci", "postgresql source=oci");
    assert_toml_field(
        dir.path(),
        "reference",
        "bitnamicharts/postgresql",
        "postgresql reference",
    );
    assert_toml_field(dir.path(), "version", "18.4.0", "postgresql version");

    husako_at(dir.path()).args(["gen"]).assert().success();

    assert_file(&dir.path().join(".husako/types/helm/postgresql.d.ts"));
    assert_file(&dir.path().join(".husako/types/helm/postgresql.js"));
    assert_dts_exports(
        &dir.path().join(".husako/types/helm/postgresql.d.ts"),
        "Postgresql",
    );

    std::fs::write(
        dir.path().join("pg-oci-values.ts"),
        "import { Postgresql } from \"helm/postgresql\";\nimport { build } from \"husako\";\nbuild([Postgresql()]);\n",
    )
    .unwrap();
    husako_at(dir.path())
        .args(["validate", "pg-oci-values.ts"])
        .assert()
        .success();
    let pg_yaml = String::from_utf8_lossy(
        &husako_at(dir.path())
            .args(["render", "pg-oci-values.ts"])
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();
    assert_valid_yaml(&pg_yaml, "render postgresql OCI");

    // ── F2: husako list shows oci source ─────────────────────────────────────
    let list_out = husako_at(dir.path()).args(["list"]).output().unwrap();
    let list_stderr = String::from_utf8_lossy(&list_out.stderr).to_string();
    assert_contains("list shows postgresql", "postgresql", &list_stderr);
    assert_contains("list shows oci source type", "oci", &list_stderr);
}
