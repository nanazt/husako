use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use husako_core::progress::{ProgressReporter, TaskHandle};
use indicatif::{ProgressBar, ProgressStyle};

use crate::style;

/// Progress reporter that uses `indicatif` spinners.
pub struct IndicatifReporter {
    counter: Arc<AtomicUsize>,
    total: Arc<AtomicUsize>,
}

impl IndicatifReporter {
    pub fn new() -> Self {
        Self {
            counter: Arc::new(AtomicUsize::new(0)),
            total: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl ProgressReporter for IndicatifReporter {
    fn set_total(&self, total: usize) {
        self.total.store(total, Ordering::Relaxed);
        self.counter.store(0, Ordering::Relaxed);
    }

    fn start_task(&self, message: &str) -> Box<dyn TaskHandle> {
        let count = self.counter.fetch_add(1, Ordering::Relaxed) + 1;
        let total = self.total.load(Ordering::Relaxed);

        let prefix = if total > 0 {
            format!("[{count}/{total}] ")
        } else {
            String::new()
        };

        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
        );
        pb.set_message(format!("{prefix}{message}"));
        pb.enable_steady_tick(std::time::Duration::from_millis(80));

        Box::new(IndicatifTaskHandle {
            pb: Arc::new(pb),
            prefix,
            base_message: message.to_string(),
        })
    }
}

struct IndicatifTaskHandle {
    pb: Arc<ProgressBar>,
    /// `[N/M] ` prefix, or empty string when no counter is set.
    prefix: String,
    /// Original message passed to `start_task`, without any progress suffix.
    base_message: String,
}

impl TaskHandle for IndicatifTaskHandle {
    fn set_message(&self, message: &str) {
        self.pb.set_message(format!("{}{message}", self.prefix));
    }

    fn set_progress(&self, bytes: u64, total_bytes: Option<u64>, pct: Option<u8>) {
        // Compute % from bytes when not provided explicitly
        let pct = pct.or_else(|| {
            total_bytes
                .filter(|&t| t > 0)
                .map(|t| (bytes * 100 / t).min(100) as u8)
        });

        let bytes_str = match total_bytes {
            Some(t) => format!(
                "{:.1} MB / {:.1} MB",
                bytes as f64 / 1_048_576.0,
                t as f64 / 1_048_576.0,
            ),
            None if bytes > 0 => format!("{:.1} MB Received", bytes as f64 / 1_048_576.0),
            None => String::new(),
        };

        let suffix = match (pct, bytes_str.is_empty()) {
            (Some(p), false) => format!("({p}% \u{00B7} {bytes_str})"),
            (Some(p), true) => format!("({p}%)"),
            (None, false) => format!("({bytes_str})"),
            (None, true) => String::new(),
        };

        if suffix.is_empty() {
            self.pb
                .set_message(format!("{}{}", self.prefix, self.base_message));
        } else {
            self.pb
                .set_message(format!("{}{} {suffix}", self.prefix, self.base_message));
        }
    }

    fn finish_ok(&self, message: &str) {
        self.pb
            .set_style(ProgressStyle::with_template("{msg}").unwrap());
        self.pb
            .finish_with_message(format!("{} {}{message}", style::check_mark(), self.prefix));
    }

    fn finish_err(&self, message: &str) {
        self.pb
            .set_style(ProgressStyle::with_template("{msg}").unwrap());
        self.pb
            .finish_with_message(format!("{} {}{message}", style::cross_mark(), self.prefix));
    }
}

impl Drop for IndicatifTaskHandle {
    /// If the task was dropped without an explicit `finish_ok`/`finish_err` call
    /// (e.g. due to an early return via `?`), clear the spinner so it does not
    /// remain frozen on the terminal.
    fn drop(&mut self) {
        if !self.pb.is_finished() {
            self.pb
                .set_style(ProgressStyle::with_template("{msg}").unwrap());
            self.pb.finish_and_clear();
        }
    }
}

#[cfg(test)]
mod tests {
    /// Returns just the progress suffix portion of a `set_progress` call by
    /// testing the suffix-building logic directly.
    fn build_suffix(bytes: u64, total_bytes: Option<u64>, pct: Option<u8>) -> String {
        let pct = pct.or_else(|| {
            total_bytes
                .filter(|&t| t > 0)
                .map(|t| (bytes * 100 / t).min(100) as u8)
        });

        let bytes_str = match total_bytes {
            Some(t) => format!(
                "{:.1} MB / {:.1} MB",
                bytes as f64 / 1_048_576.0,
                t as f64 / 1_048_576.0,
            ),
            None if bytes > 0 => format!("{:.1} MB Received", bytes as f64 / 1_048_576.0),
            None => String::new(),
        };

        match (pct, bytes_str.is_empty()) {
            (Some(p), false) => format!("({p}% \u{00B7} {bytes_str})"),
            (Some(p), true) => format!("({p}%)"),
            (None, false) => format!("({bytes_str})"),
            (None, true) => String::new(),
        }
    }

    #[test]
    fn http_with_total() {
        let suffix = build_suffix(5_243_000, Some(10_000_000), None);
        // 5_243_000 / 10_000_000 = 52%
        // 5_243_000 / 1_048_576 ≈ 5.0 MB, 10_000_000 / 1_048_576 ≈ 9.5 MB
        assert!(suffix.contains("52%"), "expected 52% in {suffix}");
        assert!(
            suffix.contains("5.0 MB / 9.5 MB"),
            "expected byte ratio in {suffix}"
        );
    }

    #[test]
    fn http_no_total() {
        let suffix = build_suffix(5_243_000, None, None);
        assert_eq!(suffix, "(5.0 MB Received)");
    }

    #[test]
    fn git_pct_and_bytes() {
        let suffix = build_suffix(5_243_000, None, Some(45));
        assert_eq!(suffix, "(45% \u{00B7} 5.0 MB Received)");
    }

    #[test]
    fn pct_only_no_bytes() {
        let suffix = build_suffix(0, None, Some(30));
        assert_eq!(suffix, "(30%)");
    }

    #[test]
    fn zero_bytes_no_total() {
        let suffix = build_suffix(0, None, None);
        assert_eq!(suffix, "");
    }
}
