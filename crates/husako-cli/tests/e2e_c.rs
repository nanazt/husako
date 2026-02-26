mod e2e_common;
use e2e_common::*;

// ── Scenario C: Resource sources (file, git) + husako remove ─────────────────

#[test]
#[ignore] // requires network; run with: cargo test -p husako -- --include-ignored
fn scenario_c_resource_sources() {
    let dir = tempfile::TempDir::new().unwrap();

    // Copy test CRD fixture into the temp dir
    std::fs::copy(
        e2e_fixtures_dir().join("test-crd.yaml"),
        dir.path().join("test-crd.yaml"),
    )
    .unwrap();
    init_project(
        dir.path(),
        "[resources]\ntest-crd = { source = \"file\", path = \"test-crd.yaml\" }",
    );

    // ── C1: resource file source (local CRD YAML) ─────────────────────────────
    assert_toml_field(dir.path(), "source", "file", "test-crd source=file");
    assert_toml_field(dir.path(), "path", "test-crd.yaml", "test-crd path");

    husako_at(dir.path()).args(["gen"]).assert().success();
    assert_file(&dir.path().join(".husako/types/k8s/e2e.husako.io/v1.d.ts"));
    assert_dts_exports(
        &dir.path().join(".husako/types/k8s/e2e.husako.io/v1.d.ts"),
        "Example",
    );

    std::fs::write(
        dir.path().join("example.ts"),
        r#"import { Example } from "k8s/e2e.husako.io/v1";
import { metadata, build } from "husako";
const ex = Example()
  .metadata(metadata().name("test-example").namespace("default"))
  .spec({ message: "hello", replicas: 1 });
build([ex]);
"#,
    )
    .unwrap();
    husako_at(dir.path())
        .args(["check", "example.ts"])
        .assert()
        .success();
    let ex_yaml = String::from_utf8_lossy(
        &husako_at(dir.path())
            .args(["render", "example.ts"])
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();
    assert_contains("render → kind: Example", "kind: Example", &ex_yaml);
    assert_contains("render → group e2e.husako.io", "e2e.husako.io", &ex_yaml);
    assert_contains("render → spec.message: hello", "hello", &ex_yaml);
    assert_valid_yaml(&ex_yaml, "render example");

    // ── C2: resource git source (cert-manager CRDs) ───────────────────────────
    husako_at(dir.path())
        .args([
            "-y",
            "add",
            "cert-manager",
            "--resource",
            "--source",
            "git",
            "--repo",
            "https://github.com/cert-manager/cert-manager",
            "--tag",
            "v1.16.3",
            "--path",
            "deploy/crds",
        ])
        .assert()
        .success();

    assert_toml_field(dir.path(), "source", "git", "cert-manager source=git");
    assert_toml_field(
        dir.path(),
        "repo",
        "cert-manager/cert-manager",
        "cert-manager repo",
    );
    assert_toml_field(dir.path(), "tag", "v1.16.3", "cert-manager tag");

    husako_at(dir.path()).args(["gen"]).assert().success();
    assert_file(&dir.path().join(".husako/types/k8s/cert-manager.io/v1.d.ts"));
    assert_dts_exports(
        &dir.path().join(".husako/types/k8s/cert-manager.io/v1.d.ts"),
        "Certificate",
    );

    std::fs::write(
        dir.path().join("certificate.ts"),
        r#"import { Certificate } from "k8s/cert-manager.io/v1";
import { metadata, build } from "husako";
const cert = Certificate()
  .metadata(metadata().name("my-cert").namespace("default"))
  .spec({
    secretName: "my-tls",
    issuerRef: { name: "letsencrypt", kind: "ClusterIssuer" },
    dnsNames: ["example.com"],
  });
build([cert]);
"#,
    )
    .unwrap();
    husako_at(dir.path())
        .args(["check", "certificate.ts"])
        .assert()
        .success();
    let cert_yaml = String::from_utf8_lossy(
        &husako_at(dir.path())
            .args(["render", "certificate.ts"])
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();
    assert_contains(
        "render → kind: Certificate",
        "kind: Certificate",
        &cert_yaml,
    );
    assert_contains("render → secretName: my-tls", "my-tls", &cert_yaml);
    assert_valid_yaml(&cert_yaml, "render certificate");

    // ── C-remove: remove cert-manager, verify types gone, example still works ─
    husako_at(dir.path())
        .args(["remove", "cert-manager"])
        .assert()
        .success();
    assert_toml_key_absent(dir.path(), "cert-manager");

    husako_at(dir.path())
        .args(["-y", "clean", "--types"])
        .assert()
        .success();
    husako_at(dir.path()).args(["gen"]).assert().success();

    assert!(
        !dir.path()
            .join(".husako/types/k8s/cert-manager.io/v1.d.ts")
            .exists(),
        "cert-manager types should be removed after dep removal"
    );

    husako_at(dir.path())
        .args(["check", "example.ts"])
        .assert()
        .success();
    let ex_after = String::from_utf8_lossy(
        &husako_at(dir.path())
            .args(["render", "example.ts"])
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();
    assert_valid_yaml(&ex_after, "render example after remove");
}
