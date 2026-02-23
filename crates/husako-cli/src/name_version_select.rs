use std::time::Duration;

use console::{Key, Style, Term, style};

const MAX_VISIBLE: usize = 8;
const LOAD_THRESHOLD: usize = 3;
const VERSION_PAGE_SIZE: usize = 10;

pub struct NameVersionResult {
    pub name: String,
    pub version: String,
}

/// Combined Name input + Version infinite-scroll select on one screen.
///
/// Both controls are active simultaneously:
/// - Character keys / Backspace edit the Name field
/// - Arrow Up/Down move the Version cursor
/// - Enter confirms both Name and selected Version
/// - Escape cancels
///
/// Layout:
/// ```text
/// Name: cert-manager  (Enter to confirm)
/// Version:
///   > 1.16.3 (latest)
///     1.16.2
///     ↓ more below
/// ```
pub fn run<F>(
    default_name: &str,
    validate_name: impl Fn(&str) -> Result<(), String>,
    mut fetch_versions: F,
) -> Result<Option<NameVersionResult>, String>
where
    F: FnMut(usize, usize) -> Result<Vec<String>, String>,
{
    let term = Term::stderr();
    // State
    let mut name_input = String::new();
    let mut name_error: Option<String> = None;
    let mut cursor: usize = 0;
    let mut scroll_offset: usize = 0;

    // --- Initial version fetch with loading screen ---
    let mut rendered = render(
        &term, &name_input, default_name, name_error.as_deref(), &[], 0, 0, 0, true,
    )?;

    let initial_versions =
        with_echo_suppressed(|| fetch_versions(VERSION_PAGE_SIZE, 0)).unwrap_or_default();
    let mut versions: Vec<String> = format_versions(&initial_versions);
    let mut has_more = initial_versions.len() == VERSION_PAGE_SIZE;
    let mut next_offset: usize = VERSION_PAGE_SIZE;

    // --- Main loop: name editing + version selection simultaneously ---
    loop {
        // Infinite scroll: auto-load when cursor approaches the bottom
        if has_more && !versions.is_empty() && cursor + LOAD_THRESHOLD >= versions.len() {
            clear(&term, rendered)?;
            let visible = MAX_VISIBLE.min(versions.len());
            rendered = render(
                &term,
                &name_input,
                default_name,
                name_error.as_deref(),
                &versions,
                cursor,
                scroll_offset,
                visible,
                true,
            )?;

            match with_echo_suppressed(|| fetch_versions(VERSION_PAGE_SIZE, next_offset)) {
                Ok(new_versions) => {
                    has_more = new_versions.len() == VERSION_PAGE_SIZE;
                    versions.extend(new_versions);
                    next_offset += VERSION_PAGE_SIZE;
                }
                Err(_) => {
                    has_more = false;
                }
            }
        }

        // Adjust scroll offset
        let visible = MAX_VISIBLE.min(versions.len());
        if cursor >= scroll_offset + visible {
            scroll_offset = cursor + 1 - visible;
        }
        if cursor < scroll_offset {
            scroll_offset = cursor;
        }

        // Render
        clear(&term, rendered)?;
        rendered = render(
            &term,
            &name_input,
            default_name,
            name_error.as_deref(),
            &versions,
            cursor,
            scroll_offset,
            visible,
            false,
        )?;

        // Handle input
        match term.read_key().map_err(|e| e.to_string())? {
            Key::ArrowUp => {
                cursor = cursor.saturating_sub(1);
            }
            Key::ArrowDown => {
                if !versions.is_empty() && cursor + 1 < versions.len() {
                    cursor += 1;
                }
            }
            Key::Enter => {
                let name_value = if name_input.is_empty() {
                    default_name.to_string()
                } else {
                    name_input.clone()
                };

                if let Err(msg) = validate_name(&name_value) {
                    name_error = Some(msg);
                    continue;
                }

                if versions.is_empty() {
                    // No versions — fall back to manual entry
                    clear(&term, rendered)?;
                    render_confirmed_line(&term, "Name", &name_value)?;
                    term.write_line(&format!(
                        "  {}",
                        style("no versions found, enter manually").dim()
                    ))
                    .map_err(|e| e.to_string())?;
                    return prompt_manual_version(&term, &name_value);
                }

                let selected = &versions[cursor];
                let version = selected
                    .strip_suffix(" (latest)")
                    .unwrap_or(selected)
                    .to_string();

                clear(&term, rendered)?;
                render_confirmed_line(&term, "Name", &name_value)?;
                render_confirmed_line(&term, "Version", &version)?;

                return Ok(Some(NameVersionResult {
                    name: name_value,
                    version,
                }));
            }
            Key::Escape => {
                clear(&term, rendered)?;
                return Ok(None);
            }
            Key::Backspace => {
                name_input.pop();
                name_error = None;
            }
            Key::Char(c) if !c.is_control() => {
                name_input.push(c);
                name_error = None;
            }
            _ => {}
        }
    }
}

fn format_versions(versions: &[String]) -> Vec<String> {
    versions
        .iter()
        .enumerate()
        .map(|(i, v)| {
            if i == 0 {
                format!("{v} (latest)")
            } else {
                v.clone()
            }
        })
        .collect()
}

fn prompt_manual_version(term: &Term, name: &str) -> Result<Option<NameVersionResult>, String> {
    let theme = crate::theme::husako_theme();
    let version: String = dialoguer::Input::with_theme(&theme)
        .with_prompt("Version")
        .validate_with(|input: &String| {
            if input.trim().is_empty() {
                Err("version cannot be empty".to_string())
            } else {
                Ok(())
            }
        })
        .interact_text()
        .map_err(|e| e.to_string())?;

    term.clear_last_lines(1).map_err(|e| e.to_string())?;

    Ok(Some(NameVersionResult {
        name: name.to_string(),
        version,
    }))
}

// --- Rendering ---

fn clear(term: &Term, lines: usize) -> Result<(), String> {
    if lines > 0 {
        term.clear_last_lines(lines).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn render_confirmed_line(term: &Term, label: &str, value: &str) -> Result<(), String> {
    term.write_line(&format!(
        "{} {}: {}",
        style("\u{2714}").green().bold(),
        Style::new().bold().apply_to(label),
        Style::new().cyan().apply_to(value),
    ))
    .map_err(|e| e.to_string())
}

/// Render the full widget: Name line + Version list.
#[allow(clippy::too_many_arguments)]
fn render(
    term: &Term,
    input: &str,
    default: &str,
    error: Option<&str>,
    versions: &[String],
    cursor: usize,
    offset: usize,
    visible: usize,
    loading: bool,
) -> Result<usize, String> {
    let mut lines = 0;

    // Name line (always one line — hint is inline)
    if input.is_empty() {
        term.write_line(&format!(
            "{}: {}  {}",
            Style::new().bold().apply_to("Name"),
            Style::new().dim().apply_to(default),
            Style::new().dim().apply_to("(Enter to confirm)"),
        ))
        .map_err(|e| e.to_string())?;
    } else {
        term.write_line(&format!(
            "{}: {}",
            Style::new().bold().apply_to("Name"),
            input,
        ))
        .map_err(|e| e.to_string())?;
    }
    lines += 1;

    // Error line
    if let Some(msg) = error {
        term.write_line(&format!("  {}", Style::new().red().apply_to(msg)))
            .map_err(|e| e.to_string())?;
        lines += 1;
    }

    // "Version:" header
    term.write_line(&format!("{}:", Style::new().bold().apply_to("Version")))
        .map_err(|e| e.to_string())?;
    lines += 1;

    if loading && versions.is_empty() {
        term.write_line(&format!("    {}", style("loading\u{2026}").dim()))
            .map_err(|e| e.to_string())?;
        lines += 1;
        return Ok(lines);
    }

    if versions.is_empty() {
        return Ok(lines);
    }

    // Scroll indicator (top)
    if offset > 0 {
        term.write_line(&format!("    {}", style("\u{2191} more above").dim()))
            .map_err(|e| e.to_string())?;
        lines += 1;
    }

    // Items
    let end = (offset + visible).min(versions.len());
    for (i, item) in versions.iter().enumerate().take(end).skip(offset) {
        let line = if i == cursor {
            format!(
                "  {} {}",
                style(">").cyan().bold(),
                style(item).cyan().bold()
            )
        } else {
            format!("    {item}")
        };
        term.write_line(&line).map_err(|e| e.to_string())?;
        lines += 1;
    }

    // Bottom indicator
    if loading {
        term.write_line(&format!("    {}", style("loading\u{2026}").dim()))
            .map_err(|e| e.to_string())?;
        lines += 1;
    } else if end < versions.len() {
        term.write_line(&format!("    {}", style("\u{2193} more below").dim()))
            .map_err(|e| e.to_string())?;
        lines += 1;
    }

    Ok(lines)
}

fn with_echo_suppressed<T>(f: impl FnOnce() -> T) -> T {
    let raw = crossterm::terminal::enable_raw_mode().is_ok();

    let result = f();

    if raw {
        let _ = crossterm::terminal::disable_raw_mode();
        while crossterm::event::poll(Duration::ZERO).unwrap_or(false) {
            let _ = crossterm::event::read();
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_versions_tags_latest() {
        let versions = vec!["1.16.3".to_string(), "1.16.2".to_string()];
        let formatted = format_versions(&versions);
        assert_eq!(formatted[0], "1.16.3 (latest)");
        assert_eq!(formatted[1], "1.16.2");
    }

    #[test]
    fn format_versions_empty() {
        let versions: Vec<String> = vec![];
        let formatted = format_versions(&versions);
        assert!(formatted.is_empty());
    }

    #[test]
    fn strip_latest_suffix() {
        let selected = "1.16.3 (latest)";
        let version = selected.strip_suffix(" (latest)").unwrap_or(selected);
        assert_eq!(version, "1.16.3");

        let selected = "1.16.2";
        let version = selected.strip_suffix(" (latest)").unwrap_or(selected);
        assert_eq!(version, "1.16.2");
    }

    #[test]
    fn constants_are_valid() {
        const { assert!(LOAD_THRESHOLD > 0) };
        const { assert!(MAX_VISIBLE > LOAD_THRESHOLD) };
        const { assert!(VERSION_PAGE_SIZE > 0) };
    }
}
