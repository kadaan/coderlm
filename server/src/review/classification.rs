use serde::Serialize;

use crate::review::{Review, SymbolChange};
use crate::symbols::symbol::SymbolKind;

#[derive(Debug, Clone, Serialize)]
pub struct SymbolClassification {
    pub file: String,
    pub symbol: String,
    pub kind: SymbolKind,
    pub classification: String,
    pub sub_classification: String,
}

/// Classify every changed symbol in the review diff by the semantic nature
/// of its change.
pub fn compute_change_classification(review: &Review) -> Vec<SymbolClassification> {
    let diff_guard = review.diff.read();
    let diff = match diff_guard.as_ref() {
        Some(d) => d,
        None => return vec![],
    };

    let mut classifications = Vec::new();

    for sym in &diff.symbol_diffs {
        let (classification, sub_classification) = classify_symbol(review, sym);
        classifications.push(SymbolClassification {
            file: sym.file.clone(),
            symbol: sym.name.clone(),
            kind: sym.kind,
            classification,
            sub_classification,
        });
    }

    classifications
}

fn classify_symbol(
    review: &Review,
    sym: &crate::review::SymbolDiffEntry,
) -> (String, String) {
    // Imports get their own category regardless of change type.
    if sym.kind == SymbolKind::Import {
        return ("import_change".to_string(), String::new());
    }

    match &sym.change {
        SymbolChange::Added => ("structural".to_string(), "added".to_string()),
        SymbolChange::Deleted => ("structural".to_string(), "deleted".to_string()),
        SymbolChange::Moved { .. } => ("structural".to_string(), "moved".to_string()),

        SymbolChange::Modified { signature_changed, body_changed, .. } => {
            let is_type_symbol = matches!(
                sym.kind,
                SymbolKind::Struct | SymbolKind::Enum | SymbolKind::Type | SymbolKind::Interface
            );

            if *signature_changed {
                if is_type_symbol {
                    ("type_change".to_string(), "signature_changed".to_string())
                } else {
                    ("api_surface_change".to_string(), "signature_changed".to_string())
                }
            } else if *body_changed {
                if is_type_symbol {
                    ("type_change".to_string(), "body_change".to_string())
                } else if has_error_handling_change(review, &sym.name, &sym.file) {
                    ("error_handling_change".to_string(), "error_handling_change".to_string())
                } else {
                    ("behavioral".to_string(), "body_logic_change".to_string())
                }
            } else {
                // signature_changed=false, body_changed=false: no visible change (edge case).
                ("behavioral".to_string(), String::new())
            }
        }
    }
}

/// Heuristic: check if the head version of the symbol body contains
/// significantly more error-handling patterns than the base version.
fn has_error_handling_change(review: &Review, symbol: &str, file: &str) -> bool {
    let base_sym = review.base_project.symbol_table.get(file, symbol);
    let head_sym = review.head_project.symbol_table.get(file, symbol);

    let (base_count, head_count) = match (base_sym, head_sym) {
        (Some(b), Some(h)) => {
            let base_src = read_symbol_body(&review.base_project.root, &b);
            let head_src = read_symbol_body(&review.head_project.root, &h);
            (error_pattern_count(&base_src), error_pattern_count(&head_src))
        }
        (None, Some(h)) => {
            let head_src = read_symbol_body(&review.head_project.root, &h);
            (0, error_pattern_count(&head_src))
        }
        _ => return false,
    };

    // Consider it an error-handling change if error patterns increased.
    head_count > base_count
}

fn read_symbol_body(root: &std::path::Path, sym: &crate::symbols::symbol::Symbol) -> String {
    let abs = root.join(&sym.file);
    std::fs::read(&abs)
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
        .unwrap_or_default()
}

/// Count occurrences of error-handling patterns in source text.
fn error_pattern_count(source: &str) -> usize {
    // Language-agnostic keywords/tokens that indicate error handling.
    const PATTERNS: &[&str] = &[
        "Result<", "Err(", "unwrap(", "unwrap_or", "expect(",
        "?",
        "try {", "catch", "throw ", "throws ",
        "error", "Error",
        "panic!(", "bail!(",
        "raise ", "except ",
        "if err != nil",
    ];
    PATTERNS.iter().map(|p| source.matches(p).count()).sum()
}
