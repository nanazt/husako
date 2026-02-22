use console::{style, Key, Term};

const MAX_VISIBLE: usize = 10;
const LOAD_THRESHOLD: usize = 3;

/// Interactive scrollable select with infinite scroll.
///
/// - Selected item is highlighted with `>` prefix and bold+cyan text
/// - Up/down arrows navigate without wrapping
/// - Auto-loads more items when cursor approaches the bottom
/// - Enter confirms, Escape cancels
///
/// Returns `Some(index)` on Enter, `None` on Escape.
pub fn run<F>(
    prompt: &str,
    items: &mut Vec<String>,
    has_more: &mut bool,
    mut load_more: F,
) -> Result<Option<usize>, String>
where
    F: FnMut() -> Result<(Vec<String>, bool), String>,
{
    if items.is_empty() {
        return Ok(None);
    }

    let term = Term::stderr();
    let mut cursor: usize = 0;
    let mut offset: usize = 0;
    let mut rendered: usize = 0;

    loop {
        // Infinite scroll: auto-load when cursor approaches the bottom
        if *has_more && cursor + LOAD_THRESHOLD >= items.len() {
            clear(&term, rendered)?;
            rendered = render_loading(&term, prompt)?;

            match load_more() {
                Ok((new_items, more)) => {
                    items.extend(new_items);
                    *has_more = more;
                }
                Err(_) => {
                    *has_more = false;
                }
            }
        }

        // Adjust scroll offset to keep cursor in view
        let visible = MAX_VISIBLE.min(items.len());
        if cursor >= offset + visible {
            offset = cursor + 1 - visible;
        }
        if cursor < offset {
            offset = cursor;
        }

        // Render
        clear(&term, rendered)?;
        rendered = render(&term, prompt, items, cursor, offset, visible)?;

        // Handle input
        match term.read_key().map_err(|e| e.to_string())? {
            Key::ArrowUp => {
                // At top: do nothing (no wrap)
                cursor = cursor.saturating_sub(1);
            }
            Key::ArrowDown => {
                if cursor + 1 < items.len() {
                    cursor += 1;
                }
                // At bottom: do nothing (no wrap)
            }
            Key::Enter => {
                clear(&term, rendered)?;
                // Show confirmed selection
                term.write_line(&format!(
                    "{} {} {}",
                    style("\u{2714}").green().bold(),
                    style(prompt).bold(),
                    style(&items[cursor]).cyan()
                ))
                .map_err(|e| e.to_string())?;
                return Ok(Some(cursor));
            }
            Key::Escape => {
                clear(&term, rendered)?;
                return Ok(None);
            }
            _ => {}
        }
    }
}

fn clear(term: &Term, lines: usize) -> Result<(), String> {
    if lines > 0 {
        term.clear_last_lines(lines).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn render(
    term: &Term,
    prompt: &str,
    items: &[String],
    cursor: usize,
    offset: usize,
    visible: usize,
) -> Result<usize, String> {
    let mut lines = 0;

    // Prompt line
    term.write_line(&format!(
        "{} {}",
        style("?").green().bold(),
        style(prompt).bold(),
    ))
    .map_err(|e| e.to_string())?;
    lines += 1;

    // Scroll indicator (top)
    if offset > 0 {
        term.write_line(&format!("    {}", style("\u{2191} more above").dim()))
            .map_err(|e| e.to_string())?;
        lines += 1;
    }

    // Items
    let end = (offset + visible).min(items.len());
    for (i, item) in items.iter().enumerate().take(end).skip(offset) {
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

    // Scroll indicator (bottom)
    if end < items.len() {
        term.write_line(&format!("    {}", style("\u{2193} more below").dim()))
            .map_err(|e| e.to_string())?;
        lines += 1;
    }

    Ok(lines)
}

fn render_loading(term: &Term, prompt: &str) -> Result<usize, String> {
    term.write_line(&format!(
        "{} {} {}",
        style("?").green().bold(),
        style(prompt).bold(),
        style("(loading...)").dim()
    ))
    .map_err(|e| e.to_string())?;
    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_valid() {
        const { assert!(LOAD_THRESHOLD > 0) };
        const { assert!(MAX_VISIBLE > LOAD_THRESHOLD) };
    }
}
