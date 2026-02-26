fn main() {
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

    let dirty = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .map(|o| !o.stdout.trim_ascii().is_empty())
        .unwrap_or(false);

    let hash = if dirty { format!("{hash}-dirty") } else { hash };

    println!("cargo:rustc-env=HUSAKO_GIT_HASH={hash}");
    println!("cargo:rustc-env=HUSAKO_BUILD_DATE={date}");
}
