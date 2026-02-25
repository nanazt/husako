use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

// ── CLI args ─────────────────────────────────────────────────────────────────

struct Args {
    criterion_dir: PathBuf,
    output_dir: PathBuf,
}

fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().collect();
    let mut criterion_dir = None::<PathBuf>;
    let mut output_dir = None::<PathBuf>;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--criterion-dir" => {
                i += 1;
                criterion_dir = args.get(i).map(PathBuf::from);
            }
            "--output-dir" => {
                i += 1;
                output_dir = args.get(i).map(PathBuf::from);
            }
            _ => {}
        }
        i += 1;
    }
    // Default criterion dir: walk up from CARGO_MANIFEST_DIR to workspace root, then target/criterion
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("."));
    let default_criterion = workspace.join("target/criterion");
    let criterion_dir = criterion_dir.unwrap_or(default_criterion.clone());
    let output_dir = output_dir.unwrap_or(default_criterion);
    Args {
        criterion_dir,
        output_dir,
    }
}

// ── Time formatting ───────────────────────────────────────────────────────────

fn format_ns(ns: f64) -> String {
    if ns < 1_000.0 {
        format!("{ns:.2} ns")
    } else if ns < 1_000_000.0 {
        format!("{:.3} µs", ns / 1_000.0)
    } else if ns < 1_000_000_000.0 {
        format!("{:.3} ms", ns / 1_000_000.0)
    } else {
        format!("{:.3} s", ns / 1_000_000_000.0)
    }
}

/// Format a SystemTime as "YYYY-MM-DD HH:MM:SS" using only std.
fn format_datetime(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    // Days since 1970-01-01
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let h = time_of_day / 3600;
    let m = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;

    // Compute year/month/day from days (proleptic Gregorian)
    let mut remaining = days;
    let mut year = 1970u64;
    loop {
        let leap = is_leap(year);
        let days_in_year = if leap { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }
    let leap = is_leap(year);
    let month_days = [
        31u64,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u64;
    for &md in &month_days {
        if remaining < md {
            break;
        }
        remaining -= md;
        month += 1;
    }
    let day = remaining + 1;
    format!("{year:04}-{month:02}-{day:02} {h:02}:{m:02}:{s:02}")
}

fn is_leap(year: u64) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

// ── Metadata ─────────────────────────────────────────────────────────────────

fn git_commit() -> String {
    let rev = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into());
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);
    if dirty { format!("{rev}-dirty") } else { rev }
}

fn husako_version() -> String {
    // CARGO_MANIFEST_DIR = crates/husako-bench → ../../ = workspace root
    let manifest =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/husako-cli/Cargo.toml");
    std::fs::read_to_string(manifest)
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.trim_start().starts_with("version"))
                .and_then(|l| l.split('"').nth(1))
                .map(|v| v.to_owned())
        })
        .unwrap_or_else(|| "unknown".into())
}

fn platform() -> String {
    format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
}

fn cpu_name() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/cpuinfo") {
            if let Some(line) = content.lines().find(|l| l.starts_with("model name")) {
                if let Some(name) = line.splitn(2, ':').nth(1) {
                    return name.trim().to_owned();
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            if !s.is_empty() {
                return s;
            }
        }
    }
    std::env::consts::ARCH.to_owned()
}

fn cpu_cores() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0)
}

fn total_memory_gib() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(s) = std::fs::read_to_string("/proc/meminfo") {
            if let Some(line) = s.lines().find(|l| l.starts_with("MemTotal:")) {
                if let Some(kb_str) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = kb_str.parse::<f64>() {
                        return Some(format!("{:.0} GiB", kb / (1024.0 * 1024.0)));
                    }
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = Command::new("sysctl").args(["-n", "hw.memsize"]).output() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            if let Ok(bytes) = s.parse::<f64>() {
                return Some(format!("{:.0} GiB", bytes / (1024.0 * 1024.0 * 1024.0)));
            }
        }
    }
    None
}

fn ci_runner() -> Option<String> {
    if std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true") {
        Some("GitHub Actions".to_owned())
    } else {
        None
    }
}

struct Meta {
    datetime: String,
    commit: String,
    version: String,
    platform: String,
    cpu: String,
    memory: Option<String>,
    runner: Option<String>,
}

impl Meta {
    fn collect() -> Self {
        let cores = cpu_cores();
        let cpu = if cores > 0 {
            format!("{} ({} cores)", cpu_name(), cores)
        } else {
            cpu_name()
        };
        Meta {
            datetime: format_datetime(SystemTime::now()),
            commit: git_commit(),
            version: husako_version(),
            platform: platform(),
            cpu,
            memory: total_memory_gib(),
            runner: ci_runner(),
        }
    }

    fn write_header(&self, out: &mut String) {
        out.push_str(&format!("> Generated: {}\n", self.datetime));
        out.push_str(&format!("> Commit: {}\n", self.commit));
        out.push_str(&format!("> Version: husako v{}\n", self.version));
        out.push_str(&format!("> Platform: {}\n", self.platform));
        out.push_str(&format!("> CPU: {}\n", self.cpu));
        if let Some(m) = &self.memory {
            out.push_str(&format!("> Memory: {}\n", m));
        }
        if let Some(r) = &self.runner {
            out.push_str(&format!("> Runner: {}\n", r));
        }
    }
}

// ── Criterion parsing ─────────────────────────────────────────────────────────

#[derive(Clone)]
struct Estimates {
    mean: f64,
    mean_lower: f64,
    mean_upper: f64,
    std_dev: f64,
    slope: Option<f64>,
}

fn parse_estimates(path: &Path) -> Option<Estimates> {
    let content = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&content).ok()?;
    let mean = v["mean"]["point_estimate"].as_f64()?;
    let mean_lower = v["mean"]["confidence_interval"]["lower_bound"].as_f64()?;
    let mean_upper = v["mean"]["confidence_interval"]["upper_bound"].as_f64()?;
    let std_dev = v["std_dev"]["point_estimate"].as_f64()?;
    let slope = v["slope"]["point_estimate"].as_f64();
    Some(Estimates {
        mean,
        mean_lower,
        mean_upper,
        std_dev,
        slope,
    })
}

// ── Discovery ─────────────────────────────────────────────────────────────────

/// Returns BTreeMap<group, BTreeMap<bench, estimates_path>>
fn discover(criterion_dir: &Path) -> BTreeMap<String, BTreeMap<String, PathBuf>> {
    let mut result: BTreeMap<String, BTreeMap<String, PathBuf>> = BTreeMap::new();
    let Ok(groups) = std::fs::read_dir(criterion_dir) else {
        return result;
    };
    for group_entry in groups.flatten() {
        let group_path = group_entry.path();
        if !group_path.is_dir() {
            continue;
        }
        let group_name = group_entry.file_name().to_string_lossy().to_string();
        // Skip non-benchmark directories (e.g. files we write ourselves)
        let Ok(benches) = std::fs::read_dir(&group_path) else {
            continue;
        };
        for bench_entry in benches.flatten() {
            let bench_path = bench_entry.path();
            if !bench_path.is_dir() {
                continue;
            }
            let estimates = bench_path.join("new/estimates.json");
            if estimates.exists() {
                let bench_name = bench_entry.file_name().to_string_lossy().to_string();
                result
                    .entry(group_name.clone())
                    .or_default()
                    .insert(bench_name, estimates);
            }
        }
    }
    result
}

// ── Report generation ─────────────────────────────────────────────────────────

fn generate_summary(meta: &Meta, data: &BTreeMap<String, BTreeMap<String, PathBuf>>) -> String {
    let mut out = String::new();
    out.push_str("# Benchmark Summary\n\n");
    meta.write_header(&mut out);
    out.push('\n');
    out.push_str("| Group | Benchmark | Mean | ± Std Dev |\n");
    out.push_str("|-------|-----------|------|-----------|\n");
    for (group, benches) in data {
        for (bench, path) in benches {
            if let Some(e) = parse_estimates(path) {
                out.push_str(&format!(
                    "| {} | {} | {} | ± {} |\n",
                    group,
                    bench,
                    format_ns(e.mean),
                    format_ns(e.std_dev),
                ));
            }
        }
    }
    out
}

fn generate_report(meta: &Meta, data: &BTreeMap<String, BTreeMap<String, PathBuf>>) -> String {
    let mut out = String::new();
    out.push_str("# Benchmark Report\n\n");
    meta.write_header(&mut out);
    for (group, benches) in data {
        out.push_str(&format!("\n## {group}\n"));
        for (bench, path) in benches {
            out.push_str(&format!("\n### {bench}\n"));
            if let Some(e) = parse_estimates(path) {
                out.push_str(&format!(
                    "- **Mean**: {} ({} – {}, 95% CI)\n",
                    format_ns(e.mean),
                    format_ns(e.mean_lower),
                    format_ns(e.mean_upper),
                ));
                out.push_str(&format!("- **Std Dev**: {}\n", format_ns(e.std_dev)));
                if let Some(slope) = e.slope {
                    out.push_str(&format!("- **Slope**: {}\n", format_ns(slope)));
                }
            } else {
                out.push_str("- *(no data)*\n");
            }
        }
    }
    out
}

// ── Terminal output ───────────────────────────────────────────────────────────

fn print_terminal(meta: &Meta, data: &BTreeMap<String, BTreeMap<String, PathBuf>>) {
    println!(
        "husako v{}  {}  {}  {}  {}",
        meta.version, meta.commit, meta.platform, meta.cpu, meta.datetime
    );
    for (group, benches) in data {
        println!();
        println!("{group}");
        let name_w = benches.keys().map(|k| k.len()).max().unwrap_or(0);
        let estimates: Vec<(&String, Option<Estimates>)> = benches
            .iter()
            .map(|(k, p)| (k, parse_estimates(p)))
            .collect();
        let mean_w = estimates
            .iter()
            .filter_map(|(_, e)| e.as_ref())
            .map(|e| format_ns(e.mean).len())
            .max()
            .unwrap_or(0);
        for (bench, est) in &estimates {
            if let Some(e) = est {
                println!(
                    "  {:<name_w$}  {:>mean_w$}   ± {}",
                    bench,
                    format_ns(e.mean),
                    format_ns(e.std_dev),
                );
            }
        }
    }
    println!();
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let args = parse_args();

    if !args.criterion_dir.exists() {
        eprintln!(
            "error: criterion directory not found: {}",
            args.criterion_dir.display()
        );
        eprintln!("Run `cargo bench -p husako-bench` first.");
        std::process::exit(1);
    }

    let data = discover(&args.criterion_dir);
    if data.is_empty() {
        eprintln!(
            "error: no estimates.json files found in {}",
            args.criterion_dir.display()
        );
        std::process::exit(1);
    }

    let meta = Meta::collect();

    print_terminal(&meta, &data);

    let summary = generate_summary(&meta, &data);
    let report = generate_report(&meta, &data);

    std::fs::create_dir_all(&args.output_dir).unwrap_or_else(|e| {
        eprintln!("error: cannot create output dir: {e}");
        std::process::exit(1);
    });

    let summary_path = args.output_dir.join("bench-summary.md");
    let report_path = args.output_dir.join("bench-report.md");

    std::fs::write(&summary_path, &summary).unwrap_or_else(|e| {
        eprintln!("error: cannot write {}: {e}", summary_path.display());
        std::process::exit(1);
    });
    std::fs::write(&report_path, &report).unwrap_or_else(|e| {
        eprintln!("error: cannot write {}: {e}", report_path.display());
        std::process::exit(1);
    });

    println!("Written:");
    println!("  {}", summary_path.display());
    println!("  {}", report_path.display());
}
