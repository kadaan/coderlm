use serde::Serialize;

use crate::index::file_entry::Language;
use crate::review::{FileDiffEntry, FileChangeStatus, Review};
use crate::symbols::symbol::{Symbol, SymbolKind};

#[derive(Debug, Clone, Serialize)]
pub struct ImportEntry {
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileImportDiff {
    pub file: String,
    pub language: Language,
    pub added_imports: Vec<ImportEntry>,
    pub removed_imports: Vec<ImportEntry>,
}

/// Compute import diff for a single file.
pub fn compute_import_diff_for_file(review: &Review, file: &str) -> Option<FileImportDiff> {
    let diff_guard = review.diff.read();
    let diff = diff_guard.as_ref()?;

    let file_entry = diff.file_diffs.iter().find(|e| e.path == file)?;
    Some(compute_diff_for_entry(review, file_entry))
}

/// Compute import diff across all changed files in the PR.
pub fn compute_import_diff_all(review: &Review) -> Vec<FileImportDiff> {
    let diff_guard = review.diff.read();
    let diff = match diff_guard.as_ref() {
        Some(d) => d,
        None => return vec![],
    };

    let entries: Vec<FileDiffEntry> = diff.file_diffs.clone();
    drop(diff_guard);

    entries
        .iter()
        .filter_map(|entry| {
            let result = {
                let diff_guard = review.diff.read();
                let diff = diff_guard.as_ref()?;
                let e = diff.file_diffs.iter().find(|e| e.path == entry.path)?;
                Some(compute_diff_for_entry(review, e))
            };
            result.filter(|r| !r.added_imports.is_empty() || !r.removed_imports.is_empty())
        })
        .collect()
}

fn compute_diff_for_entry(review: &Review, file_entry: &FileDiffEntry) -> FileImportDiff {
    let file = &file_entry.path;
    let language = file_entry.language;

    // Get imports from base (use old_path for renames).
    let base_file = file_entry.old_path.as_deref().unwrap_or(file.as_str());
    let base_imports = match file_entry.status {
        FileChangeStatus::Added => vec![],
        _ => imports_from_project(&review.base_project.symbol_table, base_file),
    };

    // Get imports from head.
    let head_imports = match file_entry.status {
        FileChangeStatus::Deleted => vec![],
        _ => imports_from_project(&review.head_project.symbol_table, file),
    };

    // Set-diff by import text (name field).
    let base_texts: std::collections::HashSet<&str> =
        base_imports.iter().map(|s| s.name.as_str()).collect();
    let head_texts: std::collections::HashSet<&str> =
        head_imports.iter().map(|s| s.name.as_str()).collect();

    let added_imports: Vec<ImportEntry> = head_imports
        .iter()
        .filter(|s| !base_texts.contains(s.name.as_str()))
        .map(|s| ImportEntry { line: s.line_range.0, text: s.name.clone() })
        .collect();

    let removed_imports: Vec<ImportEntry> = base_imports
        .iter()
        .filter(|s| !head_texts.contains(s.name.as_str()))
        .map(|s| ImportEntry { line: s.line_range.0, text: s.name.clone() })
        .collect();

    FileImportDiff { file: file.clone(), language, added_imports, removed_imports }
}

fn imports_from_project(symbol_table: &crate::symbols::SymbolTable, file: &str) -> Vec<Symbol> {
    symbol_table
        .list_by_file(file)
        .into_iter()
        .filter(|s| s.kind == SymbolKind::Import)
        .collect()
}
