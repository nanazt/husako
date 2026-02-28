use std::path::Path;

use assert_cmd::cargo::cargo_bin_cmd;

fn husako_at(dir: &Path) -> assert_cmd::Command {
    let mut cmd = cargo_bin_cmd!("husako");
    cmd.current_dir(dir);
    cmd
}

/// Path to the real-spec fixtures directory.
fn fixtures_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../husako-dts/tests/fixtures/openapi")
}

/// Copy a fixture directory tree into a target directory.
fn copy_fixture(fixture_name: &str, target: &Path) {
    let src = fixtures_dir().join(fixture_name);
    copy_dir_recursive(&src, target);
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

// ---------------------------------------------------------------------------
// Layer 3: E2E Runtime Tests
// ---------------------------------------------------------------------------

#[test]
fn e2e_render_deployment_from_real_specs() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Copy real k8s fixtures as spec-dir
    let spec_dir = root.join("specs");
    copy_fixture("k8s", &spec_dir);

    // Run husako generate with the real specs
    husako_at(root)
        .args(["generate", "--spec-dir", spec_dir.to_str().unwrap()])
        .assert()
        .success();

    // Verify generated files exist
    assert!(root.join(".husako/types/k8s/apps/v1.d.ts").exists());
    assert!(root.join(".husako/types/k8s/apps/v1.js").exists());
    assert!(root.join(".husako/types/k8s/core/v1.d.ts").exists());
    assert!(root.join(".husako/types/k8s/core/v1.js").exists());

    // Write a TypeScript entry that uses the generated modules.
    // Uses .spec() since per-property methods depend on allOf $ref handling.
    let entry = root.join("deploy.husako");
    std::fs::write(
        &entry,
        r#"
import husako from "husako";
import { name } from "k8s/meta/v1";
import { Deployment } from "k8s/apps/v1";

const d = Deployment()
    .metadata(name("nginx").label("app", "nginx"))
    .spec({
        replicas: 3,
        selector: { matchLabels: { app: "nginx" } },
        template: {
            spec: {
                containers: [{ name: "nginx", image: "nginx:1.27" }]
            }
        }
    });

husako.build([d]);
"#,
    )
    .unwrap();

    // Render and verify YAML output
    husako_at(root)
        .args(["render", entry.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("apiVersion: apps/v1"))
        .stdout(predicates::str::contains("kind: Deployment"))
        .stdout(predicates::str::contains("name: nginx"))
        .stdout(predicates::str::contains("replicas: 3"));
}

#[test]
fn e2e_render_cnpg_cluster() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Copy both k8s (for ObjectMeta common types) and cnpg CRD specs
    let spec_dir = root.join("specs");
    copy_fixture("crds/cnpg", &spec_dir);

    // Run husako generate
    husako_at(root)
        .args(["generate", "--spec-dir", spec_dir.to_str().unwrap()])
        .assert()
        .success();

    // Verify cnpg types were generated
    assert!(
        root.join(".husako/types/k8s/postgresql.cnpg.io/v1.d.ts")
            .exists()
    );
    assert!(
        root.join(".husako/types/k8s/postgresql.cnpg.io/v1.js")
            .exists()
    );

    // Write TypeScript entry using cnpg cluster
    let entry = root.join("cluster.husako");
    std::fs::write(
        &entry,
        r#"
import husako from "husako";
import { name } from "k8s/meta/v1";
import { Cluster } from "k8s/postgresql.cnpg.io/v1";

const c = Cluster()
    .metadata(name("my-pg"))
    .spec({ instances: 3, storage: { size: "10Gi" } });

husako.build([c]);
"#,
    )
    .unwrap();

    husako_at(root)
        .args(["render", entry.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("kind: Cluster"))
        .stdout(predicates::str::contains("name: my-pg"))
        .stdout(predicates::str::contains("instances: 3"));
}

#[test]
fn e2e_render_cert_manager_certificate() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    let spec_dir = root.join("specs");
    copy_fixture("crds/cert-manager", &spec_dir);

    husako_at(root)
        .args(["generate", "--spec-dir", spec_dir.to_str().unwrap()])
        .assert()
        .success();

    assert!(
        root.join(".husako/types/k8s/cert-manager.io/v1.d.ts")
            .exists()
    );
    assert!(
        root.join(".husako/types/k8s/cert-manager.io/v1.js")
            .exists()
    );

    let entry = root.join("cert.husako");
    std::fs::write(
        &entry,
        r#"
import husako from "husako";
import { name } from "k8s/meta/v1";
import { Certificate } from "k8s/cert-manager.io/v1";

const cert = Certificate()
    .metadata(name("my-cert"))
    .spec({
        secretName: "my-cert-tls",
        issuerRef: { name: "letsencrypt", kind: "ClusterIssuer" },
        dnsNames: ["example.com"]
    });

husako.build([cert]);
"#,
    )
    .unwrap();

    husako_at(root)
        .args(["render", entry.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("kind: Certificate"))
        .stdout(predicates::str::contains("name: my-cert"))
        .stdout(predicates::str::contains("secretName: my-cert-tls"));
}
