use serde::Serialize;

use crate::review::{Review, SymbolChange};
use crate::symbols::symbol::{Symbol, SymbolKind};

#[derive(Debug, Clone, Serialize)]
pub struct ComplexityMetrics {
    pub lines: usize,
    pub max_nesting: usize,
    pub parameters: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComplexityDelta {
    pub symbol: String,
    pub file: String,
    pub kind: SymbolKind,
    pub base: ComplexityMetrics,
    pub head: ComplexityMetrics,
    pub delta: ComplexityMetrics,
    /// True when any head metric is larger than the corresponding base metric.
    pub regression: bool,
}

/// Compute complexity deltas for all modified symbols in the PR.
/// Only Modified symbols have a meaningful before/after comparison.
pub fn compute_complexity(review: &Review, file_filter: Option<&str>) -> Vec<ComplexityDelta> {
    let diff_guard = review.diff.read();
    let diff = match diff_guard.as_ref() {
        Some(d) => d,
        None => return vec![],
    };

    let mut deltas = Vec::new();

    for sym in &diff.symbol_diffs {
        // Only Modified symbols have a meaningful before/after.
        let is_modified = matches!(&sym.change, SymbolChange::Modified { .. });
        if !is_modified {
            continue;
        }
        if let Some(file) = file_filter {
            if sym.file != file {
                continue;
            }
        }
        // Skip imports and variables — complexity metrics don't apply.
        if matches!(sym.kind, SymbolKind::Import | SymbolKind::Variable) {
            continue;
        }

        let base_sym = review.base_project.symbol_table.get(&sym.file, &sym.name);
        let head_sym = review.head_project.symbol_table.get(&sym.file, &sym.name);

        if let (Some(base_sym), Some(head_sym)) = (base_sym, head_sym) {
            let base_metrics = measure_symbol(&review.base_project.root, &base_sym);
            let head_metrics = measure_symbol(&review.head_project.root, &head_sym);

            let delta = ComplexityMetrics {
                lines: head_metrics.lines.saturating_sub(base_metrics.lines),
                max_nesting: head_metrics.max_nesting.saturating_sub(base_metrics.max_nesting),
                parameters: head_metrics.parameters.saturating_sub(base_metrics.parameters),
            };

            let regression = head_metrics.lines > base_metrics.lines
                || head_metrics.max_nesting > base_metrics.max_nesting
                || head_metrics.parameters > base_metrics.parameters;

            deltas.push(ComplexityDelta {
                symbol: sym.name.clone(),
                file: sym.file.clone(),
                kind: sym.kind,
                base: base_metrics,
                head: head_metrics,
                delta,
                regression,
            });
        }
    }

    deltas
}

fn measure_symbol(root: &std::path::Path, sym: &Symbol) -> ComplexityMetrics {
    let abs = root.join(&sym.file);
    let source = std::fs::read(&abs)
        .ok()
        .and_then(|bytes| {
            let (start, end) = sym.byte_range;
            let end = end.min(bytes.len());
            if start < end {
                String::from_utf8(bytes[start..end].to_vec()).ok()
            } else {
                None
            }
        })
        .unwrap_or_default();

    let lines = source.lines().count();
    let max_nesting = compute_max_nesting(&source);
    let parameters = count_parameters(&sym.signature);

    ComplexityMetrics { lines, max_nesting, parameters }
}

/// Compute max brace-nesting depth. Ignores string/comment context for
/// simplicity; gives a reasonable proxy for control-flow nesting.
fn compute_max_nesting(source: &str) -> usize {
    let mut depth: usize = 0;
    let mut max_depth: usize = 0;
    let mut in_line_comment = false;
    let mut in_string = false;
    let mut string_char = ' ';
    let mut prev = ' ';

    for ch in source.chars() {
        // Track line comments.
        if ch == '\n' {
            in_line_comment = false;
        }
        if in_line_comment {
            prev = ch;
            continue;
        }
        // Detect `//` line comments.
        if ch == '/' && prev == '/' && !in_string {
            in_line_comment = true;
            prev = ch;
            continue;
        }

        // Track string literals (single and double quoted, simplified).
        if !in_string && (ch == '"' || ch == '\'') {
            in_string = true;
            string_char = ch;
        } else if in_string && ch == string_char && prev != '\\' {
            in_string = false;
        }

        if !in_string {
            match ch {
                '{' => {
                    depth += 1;
                    if depth > max_depth {
                        max_depth = depth;
                    }
                }
                '}' => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
        }

        prev = ch;
    }

    // Subtract 1 to discount the outermost function body brace.
    max_depth.saturating_sub(1)
}

/// Count parameters from a signature string by counting commas in the
/// outermost parentheses.
fn count_parameters(signature: &str) -> usize {
    let start = match signature.find('(') {
        Some(i) => i + 1,
        None => return 0,
    };
    let chars: Vec<char> = signature[start..].chars().collect();
    let mut depth = 0usize;
    let mut commas = 0usize;
    let mut has_content = false;

    for &ch in &chars {
        match ch {
            '(' | '[' | '<' => depth += 1,
            ')' | ']' | '>' => {
                if depth == 0 {
                    break; // Reached closing paren of parameter list.
                }
                depth -= 1;
            }
            ',' if depth == 0 => commas += 1,
            ' ' | '\t' | '\n' => {}
            _ => has_content = true,
        }
    }

    if !has_content { 0 } else { commas + 1 }
}
