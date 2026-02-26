mod e2e_common;
use e2e_common::*;

// ── Scenario B: Chart sources (artifacthub, registry, git) + husako remove ───

#[test]
#[ignore] // requires network; run with: cargo test -p husako -- --include-ignored
fn scenario_b_chart_sources() {
    let dir = tempfile::TempDir::new().unwrap();
    init_project(
        dir.path(),
        "[resources]\nk8s = { source = \"release\", version = \"1.35\" }",
    );
    write_configmap(&dir.path().join("configmap.ts"));

    // Pre-generate k8s types once
    husako_at(dir.path()).args(["gen"]).assert().success();

    // ── B1: artifacthub source (bitnami/postgresql) ───────────────────────────
    husako_at(dir.path())
        .args([
            "-y",
            "add",
            "bitnami/postgresql",
            "-n",
            "pg",
            "--version",
            "18.4.0",
        ])
        .assert()
        .success();

    assert_toml_field(dir.path(), "source", "artifacthub", "pg source=artifacthub");
    assert_toml_field(dir.path(), "package", "bitnami/postgresql", "pg package");
    assert_toml_field(dir.path(), "version", "18.4.0", "pg version");

    husako_at(dir.path()).args(["gen"]).assert().success();
    assert_file(&dir.path().join(".husako/types/helm/pg.d.ts"));
    assert_file(&dir.path().join(".husako/types/helm/pg.js"));
    // chart key "pg" → type name "Pg" (to_pascal_case)
    assert_dts_exports(&dir.path().join(".husako/types/helm/pg.d.ts"), "Pg");

    std::fs::write(
        dir.path().join("pg-values.ts"),
        "import { Pg } from \"helm/pg\";\nimport { build } from \"husako\";\nbuild([Pg()]);\n",
    )
    .unwrap();
    husako_at(dir.path())
        .args(["check", "pg-values.ts"])
        .assert()
        .success();
    let pg_yaml = String::from_utf8_lossy(
        &husako_at(dir.path())
            .args(["render", "pg-values.ts"])
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();
    assert_valid_yaml(&pg_yaml, "render pg");

    // ── B2: registry source (bitnami HTTP → OCI archive delegation) ──────────
    husako_at(dir.path())
        .args([
            "-y",
            "add",
            "https://charts.bitnami.com/bitnami",
            "redis",
            "-n",
            "redis-reg",
            "--version",
            "20.0.1",
        ])
        .assert()
        .success();

    assert_toml_field(
        dir.path(),
        "source",
        "registry",
        "redis-reg source=registry",
    );
    assert_toml_field(dir.path(), "repo", "charts.bitnami.com", "redis-reg repo");
    assert_toml_field(dir.path(), "chart", "redis", "redis-reg chart");
    assert_toml_field(dir.path(), "version", "20.0.1", "redis-reg version");

    husako_at(dir.path()).args(["gen"]).assert().success();
    assert_file(&dir.path().join(".husako/types/helm/redis-reg.d.ts"));
    // "redis-reg" → "RedisReg"
    assert_dts_exports(
        &dir.path().join(".husako/types/helm/redis-reg.d.ts"),
        "RedisReg",
    );

    std::fs::write(
        dir.path().join("redis-reg-values.ts"),
        "import { RedisReg } from \"helm/redis-reg\";\nimport { build } from \"husako\";\nbuild([RedisReg()]);\n",
    )
    .unwrap();
    husako_at(dir.path())
        .args(["check", "redis-reg-values.ts"])
        .assert()
        .success();
    let redis_yaml = String::from_utf8_lossy(
        &husako_at(dir.path())
            .args(["render", "redis-reg-values.ts"])
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();
    assert_valid_yaml(&redis_yaml, "render redis-reg");

    // ── B3: git chart source (prometheus-community/helm-charts) ──────────────
    husako_at(dir.path())
        .args([
            "-y",
            "add",
            "https://github.com/prometheus-community/helm-charts",
            "-n",
            "prom-git",
            "--tag",
            "prometheus-27.0.0",
            "--path",
            "charts/prometheus/values.schema.json",
        ])
        .assert()
        .success();

    assert_toml_field(dir.path(), "source", "git", "prom-git source=git");
    assert_toml_field(dir.path(), "tag", "prometheus-27.0.0", "prom-git tag");
    assert_toml_field(dir.path(), "repo", "prometheus-community", "prom-git repo");

    husako_at(dir.path()).args(["gen"]).assert().success();
    assert_file(&dir.path().join(".husako/types/helm/prom-git.d.ts"));
    // "prom-git" → "PromGit"
    assert_dts_exports(
        &dir.path().join(".husako/types/helm/prom-git.d.ts"),
        "PromGit",
    );

    std::fs::write(
        dir.path().join("prom-git-values.ts"),
        "import { PromGit } from \"helm/prom-git\";\nimport { build } from \"husako\";\nbuild([PromGit()]);\n",
    )
    .unwrap();
    husako_at(dir.path())
        .args(["check", "prom-git-values.ts"])
        .assert()
        .success();
    let prom_yaml = String::from_utf8_lossy(
        &husako_at(dir.path())
            .args(["render", "prom-git-values.ts"])
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();
    assert_valid_yaml(&prom_yaml, "render prom-git");

    // ── B-remove: remove pg, verify TOML key gone, types cleaned up ──────────
    husako_at(dir.path())
        .args(["remove", "pg"])
        .assert()
        .success();
    assert_toml_key_absent(dir.path(), "pg");

    husako_at(dir.path())
        .args(["-y", "clean", "--types"])
        .assert()
        .success();
    husako_at(dir.path()).args(["gen"]).assert().success();

    assert!(
        !dir.path().join(".husako/types/helm/pg.d.ts").exists(),
        "pg.d.ts should be removed after dep removal"
    );

    // k8s types still work after chart removal
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
    assert_k8s_valid(&cm_yaml, "ConfigMap after remove");
}
