use dialoguer::{FuzzySelect, Select};

/// Interactively prompt which dependency to remove.
pub fn prompt_remove(deps: &[(String, &'static str, &'static str)]) -> Result<String, String> {
    if deps.is_empty() {
        return Err("no dependencies configured".to_string());
    }

    let theme = crate::theme::husako_theme();
    let items: Vec<String> = deps
        .iter()
        .map(|(name, kind, source)| format!("{name} ({kind}, {source})"))
        .collect();

    let selection = if items.len() > 5 {
        FuzzySelect::with_theme(&theme)
            .with_prompt("Which dependency to remove?")
            .items(&items)
            .default(0)
            .interact()
            .map_err(|e: dialoguer::Error| e.to_string())?
    } else {
        Select::with_theme(&theme)
            .with_prompt("Which dependency to remove?")
            .items(&items)
            .default(0)
            .interact()
            .map_err(|e| e.to_string())?
    };

    Ok(deps[selection].0.clone())
}

/// Interactively prompt what to clean.
pub fn prompt_clean() -> Result<(bool, bool), String> {
    let theme = crate::theme::husako_theme();
    let selection = Select::with_theme(&theme)
        .with_prompt("What do you want to clean?")
        .items(["Cache", "Types", "Both"])
        .default(2)
        .interact()
        .map_err(|e| e.to_string())?;

    Ok(match selection {
        0 => (true, false),
        1 => (false, true),
        2 => (true, true),
        _ => unreachable!(),
    })
}
