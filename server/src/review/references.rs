use std::collections::HashSet;

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::ops::symbol_ops;
use crate::review::{Review, SymbolChange};

#[derive(Debug, Clone, Serialize)]
pub struct StaleReference {
    pub file: String,
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReferenceCheckResult {
    pub symbol: String,
    pub file: String,
    pub change: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_changed: Option<bool>,
    pub total_references: usize,
    pub updated_in_pr: usize,
    pub not_updated: Vec<StaleReference>,
    pub risk: String,
}

/// Find callers of a changed symbol that are in files NOT modified by this PR.
/// These are the "stale" references that may need updating.
pub fn compute_reference_check(
    review: &Review,
    symbol: &str,
    file: &str,
) -> Result<ReferenceCheckResult> {
    let diff_guard = review.diff.read();
    let diff = diff_guard
        .as_ref()
        .expect("compute_reference_check called only when review is Ready");

    let entry = diff
        .symbol_diffs
        .iter()
        .find(|s| s.name == symbol && s.file == file)
        .ok_or_else(|| anyhow!("Symbol '{}' in '{}' not found in review diff", symbol, file))?;

    let (change, signature_changed) = match &entry.change {
        SymbolChange::Added => ("added", None),
        SymbolChange::Deleted => ("deleted", None),
        SymbolChange::Modified { signature_changed, .. } => {
            ("modified", Some(*signature_changed))
        }
        SymbolChange::Moved { signature_changed, .. } => ("moved", Some(*signature_changed)),
    };

    // Build the set of files modified in this PR (both old and new paths).
    let modified_files: HashSet<String> = diff
        .file_diffs
        .iter()
        .flat_map(|f| {
            let mut paths = vec![f.path.clone()];
            if let Some(old) = &f.old_path {
                paths.push(old.clone());
            }
            paths
        })
        .collect();

    // Choose which project + file to query callers from.
    // For Deleted: query base project (symbol no longer exists in head).
    // For Moved: query head project by new name in new file.
    // For Added/Modified: query head project.
    let (project, query_file) = match &entry.change {
        SymbolChange::Deleted => (&review.base_project, file.to_string()),
        _ => (&review.head_project, file.to_string()),
    };

    let callers = symbol_ops::find_callers(
        &project.root,
        &project.file_tree,
        &project.symbol_table,
        symbol,
        &query_file,
        500,
    )
    .unwrap_or_default();

    // For Moved symbols, also search for old-name references in head.
    let extra_callers = if let SymbolChange::Moved { old_file, old_name, .. } = &entry.change {
        let search_name = old_name.as_deref().unwrap_or(symbol);
        symbol_ops::find_callers(
            &review.head_project.root,
            &review.head_project.file_tree,
            &review.head_project.symbol_table,
            search_name,
            old_file,
            500,
        )
        .unwrap_or_default()
    } else {
        vec![]
    };

    let all_callers: Vec<_> = callers.into_iter().chain(extra_callers).collect();

    let mut not_updated: Vec<StaleReference> = Vec::new();
    let mut updated_in_pr: usize = 0;

    for caller in &all_callers {
        if modified_files.contains(&caller.file) {
            updated_in_pr += 1;
        } else {
            not_updated.push(StaleReference {
                file: caller.file.clone(),
                line: caller.line,
                text: caller.text.clone(),
            });
        }
    }

    let total_references = all_callers.len();

    let risk = if not_updated.is_empty() {
        "none".to_string()
    } else {
        match signature_changed {
            Some(true) => "high".to_string(),
            _ => match &entry.change {
                SymbolChange::Deleted => "high".to_string(),
                _ => "medium".to_string(),
            },
        }
    };

    Ok(ReferenceCheckResult {
        symbol: symbol.to_string(),
        file: file.to_string(),
        change: change.to_string(),
        signature_changed,
        total_references,
        updated_in_pr,
        not_updated,
        risk,
    })
}
