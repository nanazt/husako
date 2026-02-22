/// Trait for reporting progress of long-running operations.
///
/// Core defines this trait; CLI implements it with `indicatif`.
/// Tests use `SilentProgress` (no-op).
pub trait ProgressReporter: Send + Sync {
    fn start_task(&self, message: &str) -> Box<dyn TaskHandle>;
}

/// Handle for a single in-progress task.
pub trait TaskHandle: Send + Sync {
    fn set_message(&self, message: &str);
    fn finish_ok(&self, message: &str);
    fn finish_err(&self, message: &str);
}

/// No-op progress reporter for tests and non-interactive use.
pub struct SilentProgress;

impl ProgressReporter for SilentProgress {
    fn start_task(&self, _message: &str) -> Box<dyn TaskHandle> {
        Box::new(SilentTaskHandle)
    }
}

struct SilentTaskHandle;

impl TaskHandle for SilentTaskHandle {
    fn set_message(&self, _message: &str) {}
    fn finish_ok(&self, _message: &str) {}
    fn finish_err(&self, _message: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silent_progress_no_ops() {
        let progress = SilentProgress;
        let task = progress.start_task("test");
        task.set_message("updated");
        task.finish_ok("done");
    }

    #[test]
    fn silent_progress_err() {
        let progress = SilentProgress;
        let task = progress.start_task("test");
        task.finish_err("failed");
    }
}
