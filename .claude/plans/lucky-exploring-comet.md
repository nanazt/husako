# Plan: husako version subcommand with build info

## Context

`husako --version` (clap built-in)은 "0.1.0"만 출력하고 git hash/date 정보가 없다.
`husako version` subcommand를 추가해 버전 + commit hash (dirty 포함) + build date를 출력한다.
`--version` / `-V` 플래그는 제거한다.

출력 형식:
```
husako 0.1.0 (abc1234-dirty 2026-02-27)
```

## Files to Modify

- `crates/husako-cli/build.rs` — new file: git hash + build date 삽입
- `crates/husako-cli/src/main.rs` — `version` attr 제거, `Version` variant + handler 추가

## Implementation

### 1. `crates/husako-cli/build.rs` (new)

```rust
fn main() {
    // git log -1 --format="%h %cd" --date=short → e.g. "abc1234 2026-02-27"
    let (hash, date) = std::process::Command::new("git")
        .args(["log", "-1", "--format=%h %cd", "--date=short"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| {
            let s = s.trim().to_string();
            let mut p = s.splitn(2, ' ');
            let h = p.next().unwrap_or("unknown").to_string();
            let d = p.next().unwrap_or("unknown").to_string();
            (h, d)
        })
        .unwrap_or_else(|| ("unknown".to_string(), "unknown".to_string()));

    // dirty check: git status --porcelain
    let dirty = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .map(|o| !o.stdout.trim_ascii().is_empty())
        .unwrap_or(false);

    let hash = if dirty { format!("{hash}-dirty") } else { hash };

    println!("cargo:rustc-env=HUSAKO_GIT_HASH={hash}");
    println!("cargo:rustc-env=HUSAKO_BUILD_DATE={date}");
}
```

`rerun-if-changed` 없음 → 매 빌드마다 실행 (git 명령 2개, 무시할 수준).

### 2. `crates/husako-cli/src/main.rs`

**Remove** `version` from `#[command(...)]`:
```rust
// Before
#[command(name = "husako", version)]

// After
#[command(name = "husako")]
```

**Add** `Version` variant to `Commands` enum (맨 마지막):
```rust
/// Print version, commit hash, and build date.
Version,
```

**Add** handler in `match cli.command`:
```rust
Commands::Version => {
    eprintln!(
        "husako {} ({} {})",
        env!("CARGO_PKG_VERSION"),
        env!("HUSAKO_GIT_HASH"),
        env!("HUSAKO_BUILD_DATE"),
    );
    ExitCode::SUCCESS
}
```

## Tests

`crates/husako-cli/tests/integration.rs`에 추가:

```rust
#[test]
fn version_subcommand() {
    husako_cmd()
        .arg("version")
        .assert()
        .success()
        .stderr(predicates::str::contains("husako "));
}
```

`--version` 플래그 관련 기존 테스트는 없으므로 제거할 것 없음.

## Docs

`.worktrees/docs-site/docs/reference/cli.md` 맨 아래 `## Global flags` 섹션 위에 추가:

```markdown
## husako version

Print the version, commit hash, and build date.

```
husako version
```

Output example:

```
husako 0.1.0 (abc1234-dirty 2026-02-27)
```

The commit hash has a `-dirty` suffix if the working tree had uncommitted changes at build time.
```

## Verification

```bash
cargo build -p husako
./target/debug/husako version
# → husako 0.1.0 (abc1234-dirty 2026-02-27)  (or similar)

cargo test -p husako --test integration version_subcommand
```
