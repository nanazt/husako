use console::{Key, Style, Term, style};

/// Interactive text input with dim placeholder support.
///
/// - When the field is empty, shows `default` as dim placeholder text
/// - Typing replaces the placeholder; Backspace edits normally
/// - Enter with empty input returns `default`; with typed text returns the typed value
/// - Optional validation: shows a red error and re-prompts on invalid input
/// - Escape returns `None` (cancelled)
/// - After confirmation: `✔ prompt: value`
///
/// Uses `console::Term` for raw key input (same approach as `search_select`).
pub fn run(
    prompt: &str,
    default: &str,
    validate: impl Fn(&str) -> Result<(), String>,
) -> Result<Option<String>, String> {
    let term = Term::stderr();
    let mut input = String::new();
    let mut error: Option<String> = None;

    loop {
        // Render current state
        let rendered_lines = render(&term, prompt, &input, default, error.as_deref())?;

        // Read one key
        let key = term.read_key().map_err(|e| e.to_string())?;

        // Clear what we rendered before re-rendering or finishing
        if rendered_lines > 0 {
            term.clear_last_lines(rendered_lines)
                .map_err(|e| e.to_string())?;
        }

        match key {
            Key::Enter => {
                let value = if input.is_empty() {
                    default.to_string()
                } else {
                    input.clone()
                };

                match validate(&value) {
                    Ok(()) => {
                        // Show confirmation line
                        term.write_line(&format!(
                            "{} {}: {}",
                            style("\u{2714}").green().bold(),
                            Style::new().bold().apply_to(prompt),
                            Style::new().cyan().apply_to(&value)
                        ))
                        .map_err(|e| e.to_string())?;
                        return Ok(Some(value));
                    }
                    Err(msg) => {
                        error = Some(msg);
                    }
                }
            }
            Key::Escape => {
                return Ok(None);
            }
            Key::Backspace => {
                input.pop();
                error = None;
            }
            Key::Char(c) if !c.is_control() => {
                input.push(c);
                error = None;
            }
            _ => {}
        }
    }
}

/// Renders the prompt and returns the number of lines written.
fn render(
    term: &Term,
    prompt: &str,
    input: &str,
    default: &str,
    error: Option<&str>,
) -> Result<usize, String> {
    let mut lines = 0;

    if input.is_empty() {
        // Show dim placeholder
        term.write_line(&format!(
            "{}: {}",
            Style::new().bold().apply_to(prompt),
            Style::new().dim().apply_to(default)
        ))
        .map_err(|e| e.to_string())?;
    } else {
        // Show typed text
        term.write_line(&format!(
            "{}: {}",
            Style::new().bold().apply_to(prompt),
            input
        ))
        .map_err(|e| e.to_string())?;
    }
    lines += 1;

    if let Some(msg) = error {
        term.write_line(&format!("  {}", Style::new().red().apply_to(msg)))
            .map_err(|e| e.to_string())?;
        lines += 1;
    }

    Ok(lines)
}

#[cfg(test)]
mod tests {
    #[test]
    fn validate_fn_type_is_accepted() {
        // Verify that a validation function of the correct type is accepted by run's signature.
        fn no_validate(s: &str) -> Result<(), String> {
            if s.is_empty() {
                Err("empty".to_string())
            } else {
                Ok(())
            }
        }
        // Just check that the function compiles and has the right type
        let _: fn(&str) -> Result<(), String> = no_validate;
    }
}
