use std::path::PathBuf;

/// Returns the absolute path to `crates/husako-bench/fixtures/`.
pub fn bench_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .canonicalize()
        .expect("bench fixtures dir not found — run `husako gen` in crates/husako-bench/fixtures/ first")
}

/// Returns the absolute path to `crates/husako-dts/tests/fixtures/openapi/`.
pub fn dts_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../husako-dts/tests/fixtures/openapi")
        .canonicalize()
        .expect("dts fixtures dir not found")
}

/// Small TS: 1 ConfigMap, builtin imports only. (~10 lines)
pub const SMALL_TS: &str = r#"
import { metadata, build } from "husako";
import { _ResourceBuilder } from "husako/_base";

const cm = new _ResourceBuilder("v1", "ConfigMap")
  .metadata(metadata().name("bench-small").namespace("default"))
  .set("data", { key: "value" });

build([cm]);
"#;

/// Medium TS: 5 ConfigMaps, builtin imports only. (~15 lines)
pub const MEDIUM_TS: &str = r#"
import { metadata, build } from "husako";
import { _ResourceBuilder } from "husako/_base";

const resources = [0, 1, 2, 3, 4].map(function(n) {
  return new _ResourceBuilder("v1", "ConfigMap")
    .metadata(
      metadata()
        .name("bench-cm-" + n)
        .namespace("default")
        .label("index", String(n))
    )
    .set("data", { key: "value-" + n, index: String(n) });
});

build(resources);
"#;

/// Large TS: 20 ConfigMaps with annotations, builtin imports only. (~40 lines)
pub const LARGE_TS: &str = r#"
import { metadata, build } from "husako";
import { _ResourceBuilder } from "husako/_base";

function makeCm(n: number) {
  return new _ResourceBuilder("v1", "ConfigMap")
    .metadata(
      metadata()
        .name("bench-large-cm-" + n)
        .namespace("default")
        .label("env", "bench")
        .label("index", String(n))
        .annotation("created-by", "husako-bench")
        .annotation("index", String(n))
    )
    .set("data", {
      key: "value-" + n,
      index: String(n),
      description: "Benchmark ConfigMap number " + n,
    });
}

const resources = [
  makeCm(0),  makeCm(1),  makeCm(2),  makeCm(3),  makeCm(4),
  makeCm(5),  makeCm(6),  makeCm(7),  makeCm(8),  makeCm(9),
  makeCm(10), makeCm(11), makeCm(12), makeCm(13), makeCm(14),
  makeCm(15), makeCm(16), makeCm(17), makeCm(18), makeCm(19),
];

build(resources);
"#;

/// Small k8s TS: 1 Deployment. Requires generated k8s types (`husako gen`).
pub const K8S_SMALL_TS: &str = r#"
import { Deployment } from "k8s/apps/v1";
import { metadata, build } from "husako";

const app = Deployment()
  .metadata(
    metadata().name("bench-app").namespace("default").label("app", "bench")
  )
  .replicas(1);

build([app]);
"#;

/// Medium k8s TS: 5 Deployments. Requires generated k8s types (`husako gen`).
pub const K8S_MEDIUM_TS: &str = r#"
import { Deployment } from "k8s/apps/v1";
import { metadata, build } from "husako";

const names = ["frontend", "backend", "worker", "cache", "proxy"];

const deploys = names.map(function(name) {
  return Deployment()
    .metadata(
      metadata()
        .name("bench-" + name)
        .namespace("default")
        .label("app", name)
        .label("env", "bench")
    )
    .replicas(2);
});

build(deploys);
"#;
