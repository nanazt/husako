use console::{Style, style};
use dialoguer::theme::{ColorfulTheme, Theme};
use fuzzy_matcher::skim::SkimMatcherV2;
use std::fmt;

/// Custom theme for husako CLI prompts.
///
/// Visual rules:
/// - No prompt prefix (no `?` or other leading character)
/// - Colon suffix: `Prompt:`
/// - Active item: cyan+bold `>`
/// - After-selection: `✔ Prompt: value`
/// - Selected values and default hints: cyan
pub struct HusakoTheme {
    inner: ColorfulTheme,
}

/// Returns the husako CLI theme.
pub fn husako_theme() -> HusakoTheme {
    HusakoTheme {
        inner: ColorfulTheme {
            values_style: Style::new().for_stderr().cyan(),
            active_item_prefix: style(">".to_string()).for_stderr().cyan().bold(),
            active_item_style: Style::new().for_stderr().cyan().bold(),
            defaults_style: Style::new().for_stderr().cyan(),
            ..ColorfulTheme::default()
        },
    }
}

impl Theme for HusakoTheme {
    /// `Prompt:` — bold text, no prefix, colon suffix.
    fn format_prompt(&self, f: &mut dyn fmt::Write, prompt: &str) -> fmt::Result {
        write!(f, "{}:", Style::new().for_stderr().bold().apply_to(prompt))
    }

    fn format_error(&self, f: &mut dyn fmt::Write, err: &str) -> fmt::Result {
        self.inner.format_error(f, err)
    }

    /// `Prompt (default): ` or `Prompt: `
    fn format_input_prompt(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        default: Option<&str>,
    ) -> fmt::Result {
        let bold_prompt = Style::new().for_stderr().bold().apply_to(prompt);
        match default {
            Some(default) => write!(
                f,
                "{} {}: ",
                bold_prompt,
                Style::new()
                    .for_stderr()
                    .cyan()
                    .apply_to(format!("({})", default))
            ),
            None => write!(f, "{}: ", bold_prompt),
        }
    }

    /// `Prompt (y/n): `
    fn format_confirm_prompt(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        _default: Option<bool>,
    ) -> fmt::Result {
        write!(
            f,
            "{} {}: ",
            Style::new().for_stderr().bold().apply_to(prompt),
            Style::new().for_stderr().dim().apply_to("(y/n)")
        )
    }

    /// `✔ Prompt: value`
    fn format_input_prompt_selection(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        sel: &str,
    ) -> fmt::Result {
        write!(
            f,
            "{} {}: {}",
            style("\u{2714}").for_stderr().green().bold(),
            Style::new().for_stderr().bold().apply_to(prompt),
            Style::new().for_stderr().cyan().apply_to(sel)
        )
    }

    /// `✔ Prompt: yes` or `✔ Prompt: no`
    fn format_confirm_prompt_selection(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        selection: Option<bool>,
    ) -> fmt::Result {
        let value = match selection {
            Some(true) => "yes",
            Some(false) => "no",
            None => "",
        };
        write!(
            f,
            "{} {}: {}",
            style("\u{2714}").for_stderr().green().bold(),
            Style::new().for_stderr().bold().apply_to(prompt),
            Style::new().for_stderr().cyan().apply_to(value)
        )
    }

    fn format_select_prompt_item(
        &self,
        f: &mut dyn fmt::Write,
        text: &str,
        active: bool,
    ) -> fmt::Result {
        self.inner.format_select_prompt_item(f, text, active)
    }

    fn format_multi_select_prompt_item(
        &self,
        f: &mut dyn fmt::Write,
        text: &str,
        checked: bool,
        active: bool,
    ) -> fmt::Result {
        self.inner
            .format_multi_select_prompt_item(f, text, checked, active)
    }

    /// `Prompt: search_term_with_cursor` — no prefix.
    fn format_fuzzy_select_prompt(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        search_term: &str,
        bytes_pos: usize,
    ) -> fmt::Result {
        write!(f, "{}: ", Style::new().for_stderr().bold().apply_to(prompt))?;

        let (st_head, remaining) = search_term.split_at(bytes_pos);
        let mut chars = remaining.chars();
        let chr = chars.next().unwrap_or(' ');
        let st_cursor = self.inner.fuzzy_cursor_style.apply_to(chr);
        let st_tail = chars.as_str();

        write!(f, "{st_head}{st_cursor}{st_tail}")
    }

    fn format_fuzzy_select_prompt_item(
        &self,
        f: &mut dyn fmt::Write,
        text: &str,
        active: bool,
        highlight_matches: bool,
        matcher: &SkimMatcherV2,
        search_term: &str,
    ) -> fmt::Result {
        self.inner.format_fuzzy_select_prompt_item(
            f,
            text,
            active,
            highlight_matches,
            matcher,
            search_term,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(f: impl FnOnce(&mut String) -> fmt::Result) -> String {
        let mut buf = String::new();
        f(&mut buf).unwrap();
        buf
    }

    #[test]
    fn format_prompt_no_prefix() {
        let theme = husako_theme();
        let out = render(|buf| theme.format_prompt(buf, "Source type"));
        assert!(!out.starts_with('?'), "should not have ? prefix");
        assert!(out.contains("Source type"), "should have prompt text");
        assert!(out.ends_with(':'), "should end with colon");
    }

    #[test]
    fn format_input_with_default() {
        let theme = husako_theme();
        let out = render(|buf| theme.format_input_prompt(buf, "Name", Some("postgresql")));
        assert!(out.contains("Name"), "should have prompt text");
        assert!(out.contains("postgresql"), "should have default value");
    }

    #[test]
    fn format_selection_has_colon() {
        let theme = husako_theme();
        let out = render(|buf| theme.format_input_prompt_selection(buf, "Name", "my-chart"));
        assert!(out.contains("Name"), "should have prompt text");
        assert!(out.contains(": "), "should have colon separator");
        assert!(out.contains("my-chart"), "should have selected value");
    }

    #[test]
    fn format_confirm_selection() {
        let theme = husako_theme();
        let out =
            render(|buf| theme.format_confirm_prompt_selection(buf, "Remove cache", Some(true)));
        assert!(out.contains(": "), "should have colon separator");
        assert!(out.contains("yes"), "should show 'yes' for true");
    }
}
