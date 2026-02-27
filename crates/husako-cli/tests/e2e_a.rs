mod e2e_common;
use e2e_common::*;

// ── Scenario A: Static k8s + local Helm chart ────────────────────────────────
//
// Uses the checked-in test/e2e/ fixture directory. k8s types are generated from
// a pre-seeded release cache — no GitHub API network call for husako gen.
// husako outdated is verified against a local mockito server.

#[test]
fn scenario_a_static_k8s_and_local_helm() {
    let e2e_dir = e2e_fixtures_dir();

    // Pre-seed release cache so husako gen is a cache hit (no GitHub API call).
    write_release_cache(&e2e_dir, "1.35");

    // Mock GitHub API for husako outdated (tags endpoint).
    let mut mock_github = mockito::Server::new();
    let _tags_mock = mock_github
        .mock("GET", "/repos/kubernetes/kubernetes/tags")
        .match_query(mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::json!([{"name": "v1.35.0"}]).to_string())
        .create();

    // Generate types (cache hit → no network; resolves local Helm chart from file)
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
        .args(["check", "entry.ts"])
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
        .args(["check", "deploy"])
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
        .args(["check", "helm-values.ts"])
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

    // outdated: mock GitHub tags → current version is latest → exits 0
    husako_at(&e2e_dir)
        .env("HUSAKO_GITHUB_API_URL", mock_github.url())
        .args(["outdated"])
        .assert()
        .success();
}
