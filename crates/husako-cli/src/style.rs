use console::Style;

pub fn error_prefix() -> String {
    Style::new().red().bold().apply_to("error:").to_string()
}

pub fn warning_prefix() -> String {
    Style::new().yellow().bold().apply_to("warning:").to_string()
}

pub fn check_mark() -> String {
    Style::new().green().bold().apply_to("\u{2714}").to_string()
}

pub fn cross_mark() -> String {
    Style::new().red().bold().apply_to("\u{2718}").to_string()
}

pub fn arrow_mark() -> String {
    Style::new().cyan().apply_to("\u{2192}").to_string()
}

pub fn dep_name(s: &str) -> String {
    Style::new().cyan().apply_to(s).to_string()
}

pub fn dim(s: &str) -> String {
    Style::new().dim().apply_to(s).to_string()
}

pub fn bold(s: &str) -> String {
    Style::new().bold().apply_to(s).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_return_non_empty() {
        assert!(!error_prefix().is_empty());
        assert!(!check_mark().is_empty());
        assert!(!cross_mark().is_empty());
        assert!(!arrow_mark().is_empty());
        assert!(!dep_name("test").is_empty());
    }

    #[test]
    fn warning_prefix_is_non_empty() {
        assert!(!warning_prefix().is_empty());
    }

    #[test]
    fn dim_returns_non_empty() {
        assert!(!dim("test").is_empty());
    }

    #[test]
    fn bold_returns_non_empty() {
        assert!(!bold("test").is_empty());
    }
}
