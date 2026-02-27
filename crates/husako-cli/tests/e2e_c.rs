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
        dir.path().join("example.husako"),
        r#"import husako from "husako";
import { Example } from "k8s/e2e.husako.io/v1";
import { name, namespace } from "k8s/meta/v1";
const ex = Example()
  .metadata(name("test-example").namespace("default"))
  .spec({ message: "hello", replicas: 1 });
husako.build([ex]);
"#,
    )
    .unwrap();
    husako_at(dir.path())
        .args(["check", "example.husako"])
        .assert()
        .success();
    let ex_yaml = String::from_utf8_lossy(
        &husako_at(dir.path())
            .args(["render", "example.husako"])
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
            "add",
            "https://github.com/cert-manager/cert-manager",
            "-n",
            "cert-manager",
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
        dir.path().join("certificate.husako"),
        r#"import husako from "husako";
import { Certificate } from "k8s/cert-manager.io/v1";
import { name, namespace } from "k8s/meta/v1";
const cert = Certificate()
  .metadata(name("my-cert").namespace("default"))
  .spec({
    secretName: "my-tls",
    issuerRef: { name: "letsencrypt", kind: "ClusterIssuer" },
    dnsNames: ["example.com"],
  });
husako.build([cert]);
"#,
    )
    .unwrap();
    husako_at(dir.path())
        .args(["check", "certificate.husako"])
        .assert()
        .success();
    let cert_yaml = String::from_utf8_lossy(
        &husako_at(dir.path())
            .args(["render", "certificate.husako"])
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
        .args(["clean", "--types"])
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
        .args(["check", "example.husako"])
        .assert()
        .success();
    let ex_after = String::from_utf8_lossy(
        &husako_at(dir.path())
            .args(["render", "example.husako"])
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();
    assert_valid_yaml(&ex_after, "render example after remove");
}
