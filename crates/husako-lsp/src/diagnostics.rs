//! 7 diagnostic rules for `.husako` files.
//!
//! All diagnostics are produced synchronously (caller handles debounce).

use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

use crate::analysis::{offset_to_position, scan_build_calls, scan_chain_variables};
use crate::workspace::Workspace;

/// Run all diagnostic rules on the given source text.
/// Returns LSP `Diagnostic` items (no debounce — caller handles timing).
pub fn check(source: &str, ws: &Workspace) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    rule_build_contract(source, &mut diags);
    rule_quantity_literal(source, &mut diags);
    rule_image_format(source, &mut diags);

    // Schema-derived rules require metadata
    if !ws.chains_meta().is_empty() {
        rule_required_fields(source, ws, &mut diags);
        rule_pattern_check(source, ws, &mut diags);
        rule_enum_value_check(source, ws, &mut diags);
        rule_range_check(source, ws, &mut diags);
    }

    diags
}

// ── Rule 3: BuildContractCheck ────────────────────────────────────────────────

fn rule_build_contract(source: &str, diags: &mut Vec<Diagnostic>) {
    let scan = scan_build_calls(source);

    if scan.count == 0 {
        // File-level diagnostic: husako.build() never called
        diags.push(Diagnostic {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 0,
                },
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(tower_lsp::lsp_types::NumberOrString::String(
                "husako/build-contract".to_string(),
            )),
            message: "husako.build() must be called exactly once".to_string(),
            source: Some("husako".to_string()),
            ..Default::default()
        });
    } else if scan.count > 1 {
        // Mark duplicate calls (all after the first)
        for (span_start, span_end) in scan.spans.iter().skip(1) {
            let start = offset_to_position(source, *span_start);
            let end = offset_to_position(source, *span_end);
            diags.push(Diagnostic {
                range: Range { start, end },
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(tower_lsp::lsp_types::NumberOrString::String(
                    "husako/build-contract".to_string(),
                )),
                message: "husako.build() must be called exactly once (duplicate call)".to_string(),
                source: Some("husako".to_string()),
                ..Default::default()
            });
        }
    }
}

// ── Rule 2: QuantityLiteralCheck ──────────────────────────────────────────────

/// Validates Kubernetes quantity grammar for `cpu()` and `memory()` string args.
fn rule_quantity_literal(source: &str, diags: &mut Vec<Diagnostic>) {
    for (fn_name, offset, arg) in extract_string_args(source, &["cpu", "memory"]) {
        if !is_valid_quantity(&arg) {
            let start = offset_to_position(source, offset);
            let end = offset_to_position(source, offset + arg.len() as u32 + 2); // +2 for quotes
            diags.push(Diagnostic {
                range: Range { start, end },
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(tower_lsp::lsp_types::NumberOrString::String(
                    "husako/quantity-literal".to_string(),
                )),
                message: format!(
                    "Invalid Kubernetes quantity \"{arg}\" for {fn_name}(). \
                     Expected format: e.g. \"500m\", \"1\", \"2Gi\", \"128Mi\"."
                ),
                source: Some("husako".to_string()),
                ..Default::default()
            });
        }
    }
}

/// Validate a Kubernetes quantity string (simplified grammar).
/// Valid: decimal integer/fraction + optional suffix (m, K, M, G, T, P, E, Ki, Mi, Gi, Ti, Pi, Ei).
fn is_valid_quantity(s: &str) -> bool {
    // Allow decimal number with optional suffix
    let suffixes = [
        "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "m", "K", "M", "G", "T", "P", "E",
    ];
    let numeric = if let Some(stripped) = suffixes.iter().find_map(|suf| s.strip_suffix(suf)) {
        stripped
    } else {
        s
    };
    !numeric.is_empty() && numeric.chars().all(|c| c.is_ascii_digit() || c == '.')
}

// ── Rule 4: ImageFormatCheck ──────────────────────────────────────────────────

/// OCI image reference regex: `(registry/)?name(:tag)?(@sha256:[a-f0-9]{64})?`
fn rule_image_format(source: &str, diags: &mut Vec<Diagnostic>) {
    for (_fn_name, offset, arg) in extract_string_args(source, &["image"]) {
        if !is_valid_image_ref(&arg) {
            let start = offset_to_position(source, offset);
            let end = offset_to_position(source, offset + arg.len() as u32 + 2);
            diags.push(Diagnostic {
                range: Range { start, end },
                severity: Some(DiagnosticSeverity::WARNING),
                code: Some(tower_lsp::lsp_types::NumberOrString::String(
                    "husako/image-format".to_string(),
                )),
                message: format!(
                    "Invalid OCI image reference \"{arg}\". \
                     Expected format: [registry/]name[:tag][@sha256:...]"
                ),
                source: Some("husako".to_string()),
                ..Default::default()
            });
        }
    }
}

fn is_valid_image_ref(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    // Must not contain spaces or forbidden chars
    if s.chars().any(|c| c.is_whitespace()) {
        return false;
    }
    // Basic check: allow alphanumeric, `/`, `:`, `-`, `.`, `_`, `@`
    s.chars()
        .all(|c| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '/' | ':' | '-' | '.' | '_' | '@'))
}

// ── Rule 1: RequiredFieldCheck ────────────────────────────────────────────────

fn rule_required_fields(source: &str, ws: &Workspace, diags: &mut Vec<Diagnostic>) {
    // For each .containers([chain]) or .metadata(chain) call, check required fields.
    // This is a simplified scan: look for direct variable references in these calls.
    // If the variable's ChainFragment doesn't include all required fields → error.
    let chain_vars = scan_chain_variables(source);

    // Simple heuristic: look for `.metadata(varName)` and `.containers([varName])`
    // If varName is in chain_vars, check required fields.
    check_context_call(source, "metadata", "MetadataChain", &chain_vars, ws, diags);
    check_context_call(
        source,
        "containers",
        "ContainerChain",
        &chain_vars,
        ws,
        diags,
    );
}

fn check_context_call(
    source: &str,
    method: &str,
    chain_name: &str,
    chain_vars: &std::collections::HashMap<String, crate::analysis::ChainFragment>,
    ws: &Workspace,
    diags: &mut Vec<Diagnostic>,
) {
    let required_fields: Vec<String> = ws
        .chain_fields(chain_name)
        .map(|fields| {
            fields
                .iter()
                .filter(|(_, meta)| meta.required.unwrap_or(false))
                .map(|(name, _)| name.clone())
                .collect()
        })
        .unwrap_or_default();

    if required_fields.is_empty() {
        return;
    }

    // Find `.method(...)` call sites in source and check if the arg variable has all required fields
    let pattern = format!(".{method}(");
    let mut search_from = 0;
    while let Some(idx) = source[search_from..].find(&pattern) {
        let call_offset = search_from + idx;
        let after = &source[call_offset + pattern.len()..];

        // Extract simple identifier (ignoring complex expressions for safety)
        let var_name = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>();

        if let Some(frag) = chain_vars.get(&var_name) {
            for req in &required_fields {
                if !frag.fields_set.contains(req.as_str()) {
                    let start = offset_to_position(source, call_offset as u32 + 1);
                    let end = offset_to_position(
                        source,
                        call_offset as u32 + pattern.len() as u32 + var_name.len() as u32,
                    );
                    diags.push(Diagnostic {
                        range: Range { start, end },
                        severity: Some(DiagnosticSeverity::ERROR),
                        code: Some(tower_lsp::lsp_types::NumberOrString::String(
                            "husako/required-field".to_string(),
                        )),
                        message: format!("{chain_name} missing required field: {req}"),
                        source: Some("husako".to_string()),
                        ..Default::default()
                    });
                }
            }
        }

        search_from = call_offset + 1;
    }
}

// ── Rule 5: PatternCheck ──────────────────────────────────────────────────────

fn rule_pattern_check(source: &str, ws: &Workspace, diags: &mut Vec<Diagnostic>) {
    // For each method call with a string arg, check if the chain context has a pattern constraint.
    // Simplified: scan for .fieldName("value") patterns and check against known patterns.
    for chain_name in ws.chains_meta().keys() {
        let fields = ws.chain_fields(chain_name).unwrap();
        for (field_name, meta) in fields {
            let pattern_str = match &meta.pattern {
                Some(p) => p,
                None => continue,
            };
            for (_fn, offset, arg) in extract_string_args(source, &[field_name.as_str()]) {
                if !matches_pattern(&arg, pattern_str) {
                    let start = offset_to_position(source, offset);
                    let end = offset_to_position(source, offset + arg.len() as u32 + 2);
                    diags.push(Diagnostic {
                        range: Range { start, end },
                        severity: Some(DiagnosticSeverity::WARNING),
                        code: Some(tower_lsp::lsp_types::NumberOrString::String(
                            "husako/pattern-check".to_string(),
                        )),
                        message: format!(
                            "Value \"{arg}\" for {field_name}() does not match required pattern: {pattern_str}"
                        ),
                        source: Some("husako".to_string()),
                        ..Default::default()
                    });
                }
            }
        }
    }
}

fn matches_pattern(value: &str, _pattern: &str) -> bool {
    // Simplified: accept all values (full regex support is future work).
    // Returning true prevents false positives until proper regex is wired up.
    let _ = value;
    true
}

// ── Rule 6: EnumValueCheck ────────────────────────────────────────────────────

fn rule_enum_value_check(source: &str, ws: &Workspace, diags: &mut Vec<Diagnostic>) {
    for chain_name in ws.chains_meta().keys() {
        let fields = ws.chain_fields(chain_name).unwrap();
        for (field_name, meta) in fields {
            let values = match &meta.values {
                Some(v) => v,
                None => continue,
            };
            for (_fn, offset, arg) in extract_string_args(source, &[field_name.as_str()]) {
                if !values.iter().any(|v| v == &arg) {
                    let start = offset_to_position(source, offset);
                    let end = offset_to_position(source, offset + arg.len() as u32 + 2);
                    diags.push(Diagnostic {
                        range: Range { start, end },
                        severity: Some(DiagnosticSeverity::WARNING),
                        code: Some(tower_lsp::lsp_types::NumberOrString::String(
                            "husako/enum-value".to_string(),
                        )),
                        message: format!(
                            "Invalid value \"{arg}\" for {field_name}(). \
                             Allowed: {}",
                            values.join(", ")
                        ),
                        source: Some("husako".to_string()),
                        ..Default::default()
                    });
                }
            }
        }
    }
}

// ── Rule 7: RangeCheck ───────────────────────────────────────────────────────

fn rule_range_check(source: &str, ws: &Workspace, diags: &mut Vec<Diagnostic>) {
    for chain_name in ws.chains_meta().keys() {
        let fields = ws.chain_fields(chain_name).unwrap();
        for (field_name, meta) in fields {
            if meta.minimum.is_none() && meta.maximum.is_none() {
                continue;
            }
            for (_fn, offset, arg) in extract_numeric_args(source, &[field_name.as_str()]) {
                if let Ok(v) = arg.parse::<f64>() {
                    let violated = meta.minimum.is_some_and(|min| v < min)
                        || meta.maximum.is_some_and(|max| v > max);
                    if violated {
                        let start = offset_to_position(source, offset);
                        let end = offset_to_position(source, offset + arg.len() as u32);
                        let range_desc = match (meta.minimum, meta.maximum) {
                            (Some(min), Some(max)) => format!("[{min}, {max}]"),
                            (Some(min), None) => format!(">= {min}"),
                            (None, Some(max)) => format!("<= {max}"),
                            _ => unreachable!(),
                        };
                        diags.push(Diagnostic {
                            range: Range { start, end },
                            severity: Some(DiagnosticSeverity::WARNING),
                            code: Some(tower_lsp::lsp_types::NumberOrString::String(
                                "husako/range-check".to_string(),
                            )),
                            message: format!(
                                "Value {arg} for {field_name}() is out of range {range_desc}"
                            ),
                            source: Some("husako".to_string()),
                            ..Default::default()
                        });
                    }
                }
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract `(fn_name, string_start_offset, string_content)` for all
/// `fnName("...")` occurrences in source.
fn extract_string_args(source: &str, fn_names: &[&str]) -> Vec<(String, u32, String)> {
    let mut results = Vec::new();
    for fn_name in fn_names {
        let pattern = format!("{fn_name}(\"");
        let mut search_from = 0;
        while let Some(idx) = source[search_from..].find(pattern.as_str()) {
            let abs = search_from + idx + pattern.len();
            if let Some(end) = source[abs..].find('"') {
                let arg = source[abs..abs + end].to_string();
                results.push((fn_name.to_string(), abs as u32, arg));
                search_from = abs + end + 1;
            } else {
                break;
            }
        }
    }
    results
}

/// Extract `(fn_name, number_start_offset, number_content)` for all
/// `fnName(number)` occurrences in source.
fn extract_numeric_args(source: &str, fn_names: &[&str]) -> Vec<(String, u32, String)> {
    let mut results = Vec::new();
    for fn_name in fn_names {
        let pattern = format!("{fn_name}(");
        let mut search_from = 0;
        while let Some(idx) = source[search_from..].find(pattern.as_str()) {
            let abs = search_from + idx + pattern.len();
            let num: String = source[abs..]
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
                .collect();
            if !num.is_empty() {
                results.push((fn_name.to_string(), abs as u32, num));
            }
            search_from = abs + 1;
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Workspace;

    fn empty_ws() -> Workspace {
        Workspace::new()
    }

    #[test]
    fn build_contract_zero_calls() {
        let source = "const x = 1;";
        let diags = check(source, &empty_ws());
        assert!(diags.iter().any(|d| d.message.contains("exactly once")));
    }

    #[test]
    fn build_contract_one_call_ok() {
        let source = "husako.build([nginx]);";
        let diags = check(source, &empty_ws());
        assert!(!diags.iter().any(|d| d.message.contains("duplicate call")));
    }

    #[test]
    fn build_contract_duplicate_calls() {
        let source = "husako.build([a]); husako.build([b]);";
        let diags = check(source, &empty_ws());
        assert!(diags.iter().any(|d| d.message.contains("duplicate call")));
    }

    #[test]
    fn quantity_literal_valid() {
        let source = r#"cpu("500m")"#;
        let diags = check(source, &empty_ws());
        assert!(!diags.iter().any(|d| d.code.as_ref().is_some_and(|c| {
            matches!(c, tower_lsp::lsp_types::NumberOrString::String(s) if s == "husako/quantity-literal")
        })));
    }

    #[test]
    fn quantity_literal_invalid() {
        let source = r#"cpu("notavalue")"#;
        let diags = check(source, &empty_ws());
        assert!(diags.iter().any(|d| d.code.as_ref().is_some_and(|c| {
            matches!(c, tower_lsp::lsp_types::NumberOrString::String(s) if s == "husako/quantity-literal")
        })));
    }

    #[test]
    fn image_format_valid() {
        let source = r#"image("nginx:1.25")"#;
        let diags = check(source, &empty_ws());
        assert!(!diags.iter().any(|d| d.code.as_ref().is_some_and(|c| {
            matches!(c, tower_lsp::lsp_types::NumberOrString::String(s) if s == "husako/image-format")
        })));
    }

    #[test]
    fn image_format_invalid() {
        let source = r#"image("nginx 1.25")"#;
        let diags = check(source, &empty_ws());
        assert!(diags.iter().any(|d| d.code.as_ref().is_some_and(|c| {
            matches!(c, tower_lsp::lsp_types::NumberOrString::String(s) if s == "husako/image-format")
        })));
    }
}
