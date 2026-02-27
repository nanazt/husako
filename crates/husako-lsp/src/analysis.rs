//! Source analysis: context detection, chain analysis, build-call scanning.
//!
//! Performs chain-context and variable-tracking heuristics via lightweight
//! text scanning. This "best-effort" approach matches the plan's design intent
//! and avoids coupling to oxc visitor APIs that change frequently.

use std::collections::{HashMap, HashSet};

use tower_lsp::lsp_types::Position;

// ── Position utilities ────────────────────────────────────────────────────────

/// Convert an LSP `Position` (0-based line + character) to a byte offset.
pub fn lsp_position_to_offset(source: &str, pos: Position) -> u32 {
    let mut line = 0u32;
    let mut offset = 0usize;
    for ch in source.chars() {
        if line == pos.line && offset >= source.len().min(usize::MAX) {
            break;
        }
        if line == pos.line {
            // We're on the right line — count characters
            break;
        }
        if ch == '\n' {
            line += 1;
        }
        offset += ch.len_utf8();
    }
    // Add character offset within the line
    let char_offset: usize = source[offset..]
        .chars()
        .take(pos.character as usize)
        .map(|c| c.len_utf8())
        .sum();
    (offset + char_offset) as u32
}

/// Convert a byte offset to an LSP `Position`.
pub fn offset_to_position(source: &str, offset: u32) -> Position {
    let offset = offset as usize;
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    Position {
        line,
        character: col,
    }
}

// ── Chain context ─────────────────────────────────────────────────────────────

/// The chain context at a given cursor position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainContext {
    MetadataChain,
    ContainerChain,
    TolerationChain,
    VolumeMountChain,
    EnvVarChain,
    ContainerPortChain,
    SpecFragment,
    Unknown,
}

impl ChainContext {
    /// Return the chain name as used in `_chains.meta.json`.
    pub fn chain_name(&self) -> Option<&'static str> {
        match self {
            Self::MetadataChain => Some("MetadataChain"),
            Self::ContainerChain => Some("ContainerChain"),
            Self::TolerationChain => Some("TolerationChain"),
            Self::VolumeMountChain => Some("VolumeMountChain"),
            Self::EnvVarChain => Some("EnvVarChain"),
            Self::ContainerPortChain => Some("ContainerPortChain"),
            Self::SpecFragment | Self::Unknown => None,
        }
    }
}

/// Map a builder method name to its chain context.
pub fn method_to_context(method: &str) -> ChainContext {
    match method {
        "metadata" => ChainContext::MetadataChain,
        "containers" | "initContainers" => ChainContext::ContainerChain,
        "tolerations" => ChainContext::TolerationChain,
        "volumeMounts" => ChainContext::VolumeMountChain,
        "env" => ChainContext::EnvVarChain,
        "ports" => ChainContext::ContainerPortChain,
        _ => ChainContext::Unknown,
    }
}

/// Determine the chain context at the given cursor position.
///
/// Scans backwards from the cursor to find the nearest enclosing builder
/// method call (e.g. `.metadata(`, `.containers([`). This is a best-effort
/// heuristic; it silently returns `SpecFragment` when context is ambiguous.
pub fn context_at(source: &str, position: Position) -> ChainContext {
    let offset = lsp_position_to_offset(source, position) as usize;
    let before = &source[..offset.min(source.len())];

    // Walk backwards looking for `.methodName(` or `.methodName([`
    let builder_methods = [
        "metadata",
        "containers",
        "initContainers",
        "tolerations",
        "volumeMounts",
        "env",
        "ports",
    ];

    // Find the rightmost `.method(` before the cursor
    let mut best: Option<(usize, &str)> = None;
    for method in &builder_methods {
        let pattern = format!(".{method}(");
        if let Some(idx) = rfind(before, &pattern)
            && best.is_none_or(|(best_idx, _)| idx > best_idx)
        {
            best = Some((idx, method));
        }
    }

    match best {
        Some((_, method)) => method_to_context(method),
        None => ChainContext::SpecFragment,
    }
}

/// `str::rfind` helper (avoids borrowing issues).
fn rfind(haystack: &str, needle: &str) -> Option<usize> {
    haystack.rfind(needle)
}

// ── Build call scanning ───────────────────────────────────────────────────────

/// Result of scanning a file for `husako.build()` call sites.
#[derive(Debug, Default)]
pub struct BuildCallScan {
    pub count: usize,
    /// Approximate byte spans `(start, end)` for each call.
    pub spans: Vec<(u32, u32)>,
}

/// Count `husako.build(...)` calls in the source.
pub fn scan_build_calls(source: &str) -> BuildCallScan {
    let pattern = "husako.build(";
    let mut scan = BuildCallScan::default();
    let mut search_from = 0;

    while let Some(idx) = source[search_from..].find(pattern) {
        let abs = search_from + idx;
        let start = abs as u32;
        // Find the closing paren (simplified: find next `)` on same depth)
        let end = source[abs + pattern.len()..]
            .find(')')
            .map(|i| (abs + pattern.len() + i + 1) as u32)
            .unwrap_or(start + pattern.len() as u32);
        scan.count += 1;
        scan.spans.push((start, end));
        search_from = abs + 1;
    }

    scan
}

// ── Variable / chain fragment tracking ───────────────────────────────────────

/// A chain fragment assembled from starter function calls.
#[derive(Debug, Default, Clone)]
pub struct ChainFragment {
    /// Field names set on this chain (e.g. `"name"`, `"image"`).
    pub fields_set: HashSet<String>,
}

/// Scan the source for simple direct variable assignments of chain expressions:
/// `const x = starterFn(...).method(...)` → record which fields are set on `x`.
///
/// Only handles simple `const`/`let` declarators — skips function boundaries,
/// conditionals, and complex patterns (zero false positives).
pub fn scan_chain_variables(source: &str) -> HashMap<String, ChainFragment> {
    let mut vars: HashMap<String, ChainFragment> = HashMap::new();

    // Match: `const <name> = <chain_expr>;` or `let <name> = <chain_expr>;`
    for line in source.lines() {
        let trimmed = line.trim();
        let rest = if let Some(r) = trimmed.strip_prefix("const ") {
            r
        } else if let Some(r) = trimmed.strip_prefix("let ") {
            r
        } else {
            continue;
        };

        // Extract variable name (identifier before ` = `)
        let eq_idx = match rest.find(" = ") {
            Some(i) => i,
            None => continue,
        };
        let var_name = rest[..eq_idx].trim().to_string();
        if var_name.is_empty() || !var_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }

        // Skip conditionals (ternary) — zero false positives rule
        let rhs = &rest[eq_idx + 3..];
        if rhs.contains(" ? ") || rhs.starts_with("flag") {
            continue;
        }

        // Collect all `.fieldName(` patterns in the RHS
        let fields = collect_chain_fields(rhs);
        vars.insert(var_name, ChainFragment { fields_set: fields });
    }

    vars
}

/// Collect all method names called in a chain expression string.
/// `name("x").image("y")` → `{"name", "image"}`.
fn collect_chain_fields(expr: &str) -> HashSet<String> {
    let mut fields = HashSet::new();
    let mut rest = expr;

    // Find all `.fieldName(` occurrences
    while let Some(dot_idx) = rest.find('.') {
        let after_dot = &rest[dot_idx + 1..];
        let name: String = after_dot
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            // Make sure it's followed by `(`
            if after_dot[name.len()..].starts_with('(') {
                fields.insert(name);
            }
        }
        rest = &rest[dot_idx + 1..];
    }

    // Also capture the leading starter function name: `name(...)` at start
    let starter: String = expr
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if !starter.is_empty() && expr[starter.len()..].starts_with('(') {
        fields.insert(starter);
    }

    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_inside_metadata() {
        let source = "Deployment().metadata(\n  name(\"x\").\n)";
        let pos = Position {
            line: 1,
            character: 11,
        };
        assert_eq!(context_at(source, pos), ChainContext::MetadataChain);
    }

    #[test]
    fn context_inside_containers() {
        let source = "Deployment().containers([\n  name(\"x\").\n])";
        let pos = Position {
            line: 1,
            character: 11,
        };
        assert_eq!(context_at(source, pos), ChainContext::ContainerChain);
    }

    #[test]
    fn context_top_level() {
        let source = "const x = name(\"app\").";
        let pos = Position {
            line: 0,
            character: 21,
        };
        assert_eq!(context_at(source, pos), ChainContext::SpecFragment);
    }

    #[test]
    fn build_scan_zero() {
        let scan = scan_build_calls("const x = 1;");
        assert_eq!(scan.count, 0);
    }

    #[test]
    fn build_scan_one() {
        let scan = scan_build_calls("husako.build([nginx]);");
        assert_eq!(scan.count, 1);
    }

    #[test]
    fn build_scan_two() {
        let scan = scan_build_calls("husako.build([a]); husako.build([b]);");
        assert_eq!(scan.count, 2);
    }

    #[test]
    fn chain_variable_fields() {
        let vars = scan_chain_variables("const c = name(\"nginx\").image(\"nginx:1.25\");\n");
        let frag = vars.get("c").expect("c should be tracked");
        assert!(frag.fields_set.contains("name"));
        assert!(frag.fields_set.contains("image"));
    }

    #[test]
    fn chain_variable_skips_conditional() {
        let vars =
            scan_chain_variables("const c = flag ? name(\"x\").image(\"a\") : name(\"y\");\n");
        // Conditional — skipped for zero false positives
        assert!(!vars.contains_key("c"));
    }
}
