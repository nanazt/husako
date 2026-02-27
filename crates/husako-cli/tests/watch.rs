/// Integration test for `husako render --watch`.
///
/// Verifies the full watch lifecycle using a real subprocess:
///   1. Initial render on startup
///   2. Re-render when the source file changes
///
/// No network access or pre-generated k8s types required — the entry files use
/// only the builtin `"husako"` module.
use std::time::{Duration, Instant};

// ── RAII guard — kills subprocess on drop even if the test panics ─────────────

struct KillOnDrop(std::process::Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

// ── Entry file fixtures ───────────────────────────────────────────────────────

const ENTRY_V1: &str = r#"import { build } from "husako";
build([{
  _render() {
    return { apiVersion: "v1", kind: "ConfigMap", metadata: { name: "initial" } };
  }
}]);
"#;

const ENTRY_V2: &str = r#"import { build } from "husako";
build([{
  _render() {
    return { apiVersion: "v1", kind: "ConfigMap", metadata: { name: "watch-test" } };
  }
}]);
"#;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Polls `cond` every `interval` until it returns `Some(T)` or `deadline` is reached.
fn poll_until<T, F: Fn() -> Option<T>>(
    deadline: Instant,
    interval: Duration,
    cond: F,
) -> Option<T> {
    loop {
        if let Some(v) = cond() {
            return Some(v);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(interval);
    }
}

// ── Test ──────────────────────────────────────────────────────────────────────

/// Full watch lifecycle: initial render + re-render on file change.
///
/// Uses a 5-second timeout per phase and a 100 ms poll interval, matching the
/// timeouts recommended in the plan to stay reliable across CI.
#[test]
fn watch_rerenders_on_file_change() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let root = dir.path();

    // Minimal project — no k8s types needed.
    std::fs::write(root.join("husako.toml"), "[entries]\n").unwrap();
    let entry = root.join("entry.ts");
    std::fs::write(&entry, ENTRY_V1).unwrap();

    let out_yaml = root.join("out.yaml");
    let child = std::process::Command::new(env!("CARGO_BIN_EXE_husako"))
        .args(["render", "entry.ts", "--watch", "-o", "out.yaml"])
        .current_dir(root)
        .spawn()
        .expect("failed to spawn husako render --watch");
    let _guard = KillOnDrop(child);

    // ── Phase 1: wait for initial render ──────────────────────────────────
    let poll = Duration::from_millis(100);
    let initial_content = poll_until(Instant::now() + Duration::from_secs(5), poll, || {
        std::fs::read_to_string(&out_yaml)
            .ok()
            .filter(|s| !s.is_empty())
    });
    let initial_content =
        initial_content.expect("initial render did not produce out.yaml within 5 s");
    assert!(
        initial_content.contains("initial"),
        "initial render should contain 'initial':\n{initial_content}"
    );

    // ── Phase 2: trigger re-render by overwriting entry.ts ────────────────
    std::fs::write(&entry, ENTRY_V2).unwrap();

    let final_content = poll_until(Instant::now() + Duration::from_secs(5), poll, || {
        std::fs::read_to_string(&out_yaml)
            .ok()
            .filter(|s| s != &initial_content && s.contains("watch-test"))
    });
    let final_content =
        final_content.expect("watch did not re-render within 5 s after file change");
    assert!(
        final_content.contains("watch-test"),
        "re-rendered output should contain 'watch-test':\n{final_content}"
    );
}
