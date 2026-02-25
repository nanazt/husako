mod e2e_common;
use e2e_common::*;

// ── Scenario A: Static k8s + local Helm chart ────────────────────────────────
//
// Uses the checked-in test/e2e/ fixture directory. Downloads k8s 1.35 types
// from the GitHub release API (cached by CI).

#[test]
#[ignore] // requires network; run with: cargo test -p husako -- --include-ignored
fn scenario_a_static_k8s_and_local_helm() {
    let e2e_dir = e2e_fixtures_dir();

    // Generate types (downloads k8s OpenAPI spec + resolves local Helm chart)
    husako_at(&e2e_dir).args(["gen"]).assert().success();

    // Side-effect: type files generated with correct exports
    assert_file(&e2e_dir.join(".husako/types/helm/local-chart.d.ts"));
    assert_file(&e2e_dir.join(".husako/types/helm/local-chart.js"));
    assert_dts_exports(
        &e2e_dir.join(".husako/types/helm/local-chart.d.ts"),
        "LocalChart",
    );
    assert_file(&e2e_dir.join(".husako/types/k8s/apps/v1.d.ts"));
    assert_dts_exports(
        &e2e_dir.join(".husako/types/k8s/apps/v1.d.ts"),
        "Deployment",
    );

    // husako list shows both k8s and local-chart (output goes to stderr)
    let list_out = husako_at(&e2e_dir).args(["list"]).output().unwrap();
    let list_stderr = String::from_utf8_lossy(&list_out.stderr).to_string();
    assert_contains("list shows k8s", "k8s", &list_stderr);
    assert_contains("list shows local-chart", "local-chart", &list_stderr);

    // validate entry.ts (k8s Deployment via file path)
    husako_at(&e2e_dir)
        .args(["validate", "entry.ts"])
        .assert()
        .success();

    // render entry.ts
    let render = husako_at(&e2e_dir)
        .args(["render", "entry.ts"])
        .output()
        .unwrap();
    let yaml = String::from_utf8_lossy(&render.stdout).to_string();
    assert_contains("render → kind: Deployment", "kind: Deployment", &yaml);
    assert_contains("render → metadata.name: nginx", "name: nginx", &yaml);
    assert_contains("render → image: nginx:1.25", "nginx:1.25", &yaml);
    assert_k8s_valid(&yaml, "entry.ts Deployment");
    assert_valid_yaml(&yaml, "render entry.ts");

    // validate + render via entry alias 'deploy'
    husako_at(&e2e_dir)
        .args(["validate", "deploy"])
        .assert()
        .success();
    let alias_yaml = String::from_utf8_lossy(
        &husako_at(&e2e_dir)
            .args(["render", "deploy"])
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();
    assert_contains("render alias 'deploy'", "kind: Deployment", &alias_yaml);
    assert_k8s_valid(&alias_yaml, "alias 'deploy'");

    // validate + render helm-values.ts (local Helm chart)
    husako_at(&e2e_dir)
        .args(["validate", "helm-values.ts"])
        .assert()
        .success();
    let helm_yaml = String::from_utf8_lossy(
        &husako_at(&e2e_dir)
            .args(["render", "helm-values.ts"])
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();
    assert_contains(
        "render helm → replicaCount: 2",
        "replicaCount: 2",
        &helm_yaml,
    );
    assert_contains(
        "render helm → repository: nginx",
        "repository: nginx",
        &helm_yaml,
    );
    assert_valid_yaml(&helm_yaml, "render helm-values");

    // render via helm alias
    let helm_alias = String::from_utf8_lossy(
        &husako_at(&e2e_dir)
            .args(["render", "helm"])
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();
    assert_contains("render alias 'helm'", "replicaCount", &helm_alias);

    // CLI smoke tests — exit 0 + non-empty stderr output
    let info_out = husako_at(&e2e_dir).args(["info"]).output().unwrap();
    assert!(
        !info_out.stderr.is_empty(),
        "husako info should produce output"
    );

    let debug_out = husako_at(&e2e_dir).args(["debug"]).output().unwrap();
    assert!(
        !debug_out.stderr.is_empty(),
        "husako debug should produce output"
    );

    // outdated may exit non-zero if deps are outdated; just verify it runs
    let _ = husako_at(&e2e_dir).args(["outdated"]).output().unwrap();
}
