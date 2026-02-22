use std::sync::Arc;

use husako_core::progress::{ProgressReporter, TaskHandle};
use indicatif::{ProgressBar, ProgressStyle};

use crate::style;

/// Progress reporter that uses `indicatif` spinners.
pub struct IndicatifReporter;

impl IndicatifReporter {
    pub fn new() -> Self {
        Self
    }
}

impl ProgressReporter for IndicatifReporter {
    fn start_task(&self, message: &str) -> Box<dyn TaskHandle> {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
        );
        pb.set_message(message.to_string());
        pb.enable_steady_tick(std::time::Duration::from_millis(80));
        Box::new(IndicatifTaskHandle { pb: Arc::new(pb) })
    }
}

struct IndicatifTaskHandle {
    pb: Arc<ProgressBar>,
}

impl TaskHandle for IndicatifTaskHandle {
    fn set_message(&self, message: &str) {
        self.pb.set_message(message.to_string());
    }

    fn finish_ok(&self, message: &str) {
        self.pb
            .set_style(ProgressStyle::with_template("{msg}").unwrap());
        self.pb
            .finish_with_message(format!("{} {message}", style::check_mark()));
    }

    fn finish_err(&self, message: &str) {
        self.pb
            .set_style(ProgressStyle::with_template("{msg}").unwrap());
        self.pb
            .finish_with_message(format!("{} {message}", style::cross_mark()));
    }
}
