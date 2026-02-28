//! Code completion: context-filtered methods, auto-import, quantity values.

use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, InsertTextFormat, Position};

use crate::analysis::{ChainContext, context_at};
use crate::workspace::Workspace;

/// Hardcoded common `cpu()` quantity values.
const CPU_COMPLETIONS: &[&str] = &["100m", "250m", "500m", "1000m", "1", "2", "4"];

/// Hardcoded common `memory()` quantity values.
const MEMORY_COMPLETIONS: &[&str] = &[
    "64Mi", "128Mi", "256Mi", "512Mi", "1Gi", "2Gi", "4Gi", "8Gi",
];

/// Generate completion items for the given source text and cursor position.
pub fn completions(source: &str, position: Position, ws: &Workspace) -> Vec<CompletionItem> {
    // Check if cursor is inside a cpu("") or memory("") string argument.
    if let Some(items) = quantity_completions(source, position) {
        return items;
    }

    let ctx = context_at(source, position);
    method_completions(&ctx, ws)
}

/// Return quantity value completions when cursor is inside `cpu("...")` or `memory("...")`.
fn quantity_completions(source: &str, position: Position) -> Option<Vec<CompletionItem>> {
    use crate::analysis::lsp_position_to_offset;
    let offset = lsp_position_to_offset(source, position) as usize;

    // Look backwards for `cpu("` or `memory("` before the cursor.
    let before = &source[..offset.min(source.len())];

    let kind = if before.contains("cpu(\"") || before.ends_with("cpu(\"") {
        // Scan backwards for the most recent cpu(" prefix
        if let Some(idx) = before.rfind("cpu(\"") {
            // Make sure cursor is inside the string (after the opening quote)
            let after_quote = &source[idx + 5..];
            if !after_quote.contains('"')
                || offset <= idx + 5 + after_quote.find('"').unwrap_or(usize::MAX)
            {
                Some("cpu")
            } else {
                None
            }
        } else {
            None
        }
    } else if before.contains("memory(\"") {
        if let Some(idx) = before.rfind("memory(\"") {
            let after_quote = &source[idx + 8..];
            if !after_quote.contains('"')
                || offset <= idx + 8 + after_quote.find('"').unwrap_or(usize::MAX)
            {
                Some("memory")
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    kind.map(|k| {
        let values: &[&str] = if k == "cpu" {
            CPU_COMPLETIONS
        } else {
            MEMORY_COMPLETIONS
        };
        values
            .iter()
            .map(|v| CompletionItem {
                label: v.to_string(),
                kind: Some(CompletionItemKind::VALUE),
                insert_text: Some(v.to_string()),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                ..Default::default()
            })
            .collect()
    })
}

/// Return method completions filtered by chain context.
fn method_completions(ctx: &ChainContext, ws: &Workspace) -> Vec<CompletionItem> {
    let chain_name = match ctx.chain_name() {
        Some(n) => n,
        None => return vec![],
    };

    let fields = match ws.chain_fields(chain_name) {
        Some(f) => f,
        None => return vec![],
    };

    fields
        .iter()
        .map(|(name, meta)| {
            let required = meta.required.unwrap_or(false);
            let detail = if required {
                Some(format!(
                    "{} (required)",
                    meta.field_type.as_deref().unwrap_or("any")
                ))
            } else {
                meta.field_type.clone()
            };

            let label = format!("{}()", name);

            // If enum type, include values in documentation
            let documentation = meta.values.as_ref().map(|vals| {
                tower_lsp::lsp_types::Documentation::String(format!(
                    "Allowed values: {}",
                    vals.join(", ")
                ))
            });

            CompletionItem {
                label: label.clone(),
                kind: Some(CompletionItemKind::METHOD),
                detail,
                documentation,
                insert_text: Some(format!("{}($1)", name)),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Workspace;

    fn empty_ws() -> Workspace {
        Workspace::new()
    }

    #[test]
    fn cpu_completions_trigger() {
        let source = r#"cpu(""#;
        let pos = Position {
            line: 0,
            character: 5,
        };
        let items = completions(source, pos, &empty_ws());
        assert!(!items.is_empty());
        assert!(items.iter().any(|i| i.label == "100m"));
        assert!(items.iter().any(|i| i.label == "500m"));
    }

    #[test]
    fn memory_completions_trigger() {
        let source = r#"memory(""#;
        let pos = Position {
            line: 0,
            character: 8,
        };
        let items = completions(source, pos, &empty_ws());
        assert!(!items.is_empty());
        assert!(items.iter().any(|i| i.label == "128Mi"));
        assert!(items.iter().any(|i| i.label == "1Gi"));
    }

    #[test]
    fn no_completions_without_meta() {
        let source = ".metadata(\n  name(\"nginx\").\n)";
        let pos = Position {
            line: 1,
            character: 17,
        };
        // No _chains.meta.json loaded → empty completions
        let items = completions(source, pos, &empty_ws());
        assert!(items.is_empty());
    }
}
