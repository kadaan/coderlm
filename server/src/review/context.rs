use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::index::file_entry::Language;
use crate::ops::symbol_ops;
use crate::review::{FileChangeStatus, Review, SymbolChange, SymbolDiffEntry};
use crate::symbols::symbol::SymbolKind;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    NewFile,
    Modified,
    ModifiedDeletionsOnly,
    DeletedFile,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadingStrategy {
    FullFile,
    ChangedRegionsWithContext,
    #[serde(rename = "none")]
    None,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChangedRange {
    pub start: usize,
    pub end: usize,
    pub symbols: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextSymbolEntry {
    pub name: String,
    pub kind: SymbolKind,
    pub line_range: (usize, usize),
    pub change: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_changed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_changed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceExcerpt {
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileContextResult {
    pub file: String,
    pub status: FileChangeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    pub language: Language,
    pub total_lines: usize,
    pub change_type: ChangeType,
    pub patch: String,
    pub changed_ranges: Vec<ChangedRange>,
    pub symbols: Vec<ContextSymbolEntry>,
    pub source_excerpts: Vec<SourceExcerpt>,
    pub reading_strategy: ReadingStrategy,
}

// ---------------------------------------------------------------------------
// Main computation
// ---------------------------------------------------------------------------

pub fn compute_file_context(
    review: &Review,
    file: &str,
    context_lines: usize,
    max_full_file_lines: usize,
) -> Result<Arc<FileContextResult>> {
    // Check cache first.
    if let Some(cached) = review.cache.file_contexts.get(file) {
        return Ok(cached.clone());
    }

    let diff_guard = review.diff.read();
    let diff = diff_guard.as_ref().expect("compute_file_context called only when review is Ready");

    let file_entry = diff
        .file_diffs
        .iter()
        .find(|e| e.path == file)
        .ok_or_else(|| anyhow!("File '{}' not found in review diff", file))?;

    let old_path = file_entry.old_path.clone();
    let language = file_entry.language;
    let status = file_entry.status.clone();

    // Get or compute patch; cache it for reuse by review-file-diff.
    let patch = match review.cache.file_diffs.get(file) {
        Some(cached) => cached.clone(),
        None => {
            let p = crate::git::get_file_diff(
                &review.repo_root,
                &review.base_ref,
                &review.head_ref,
                file,
            )?;
            review.cache.file_diffs.insert(file.to_string(), p.clone());
            p
        }
    };

    let change_type = determine_change_type(&status, &patch);

    // Read source lines (only needed for non-deleted files).
    let (total_lines, source_lines_opt) = match &change_type {
        ChangeType::DeletedFile | ChangeType::ModifiedDeletionsOnly => {
            let base_file = old_path.as_deref().unwrap_or(file);
            let abs = review.base_project.root.join(base_file);
            let count = std::fs::read_to_string(&abs)
                .map(|c| c.lines().count())
                .unwrap_or(0);
            (count, None::<Vec<String>>)
        }
        _ => {
            let abs = review.head_project.root.join(file);
            let content = std::fs::read_to_string(&abs)?;
            let lines: Vec<String> = content.lines().map(str::to_string).collect();
            let count = lines.len();
            (count, Some(lines))
        }
    };

    let hunk_ranges = parse_hunk_ranges(&patch);

    // Collect diff symbol entries for this file and build ContextSymbolEntry list.
    let file_symbols: Vec<&SymbolDiffEntry> =
        diff.symbol_diffs.iter().filter(|s| s.file == file).collect();

    let context_symbols: Vec<ContextSymbolEntry> = file_symbols
        .iter()
        .map(|sym| {
            let line_range = get_symbol_line_range(review, sym, file);
            sym_to_context_entry(sym, line_range)
        })
        .collect();

    let changed_ranges = build_changed_ranges(&hunk_ranges, &context_symbols);

    let reading_strategy = match &change_type {
        ChangeType::DeletedFile | ChangeType::ModifiedDeletionsOnly => ReadingStrategy::None,
        ChangeType::NewFile => ReadingStrategy::FullFile,
        ChangeType::Modified => {
            if total_lines <= max_full_file_lines || hunk_ranges.is_empty() {
                ReadingStrategy::FullFile
            } else {
                ReadingStrategy::ChangedRegionsWithContext
            }
        }
    };

    let source_excerpts = match (&reading_strategy, &source_lines_opt) {
        (ReadingStrategy::FullFile, Some(lines)) => {
            vec![build_excerpt(lines, 1, total_lines)]
        }
        (ReadingStrategy::ChangedRegionsWithContext, Some(lines)) => {
            let windows = expand_and_merge_ranges(&hunk_ranges, context_lines, total_lines);
            windows.iter().map(|(s, e)| build_excerpt(lines, *s, *e)).collect()
        }
        _ => vec![],
    };

    let result = Arc::new(FileContextResult {
        file: file.to_string(),
        status,
        old_path,
        language,
        total_lines,
        change_type,
        patch,
        changed_ranges,
        symbols: context_symbols,
        source_excerpts,
        reading_strategy,
    });

    review.cache.file_contexts.insert(file.to_string(), result.clone());

    Ok(result)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Parse `@@ -old_start,old_count +new_start,new_count @@` hunk headers.
/// Returns head-side line ranges (1-indexed, inclusive).
fn parse_hunk_ranges(patch: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for line in patch.lines() {
        if !line.starts_with("@@") {
            continue;
        }
        // Find the '+' introducing the head-side range.
        if let Some(plus_pos) = line.find('+') {
            let rest = &line[plus_pos + 1..];
            let end_pos = rest
                .find(|c: char| c == ' ' || c == '@')
                .unwrap_or(rest.len());
            let part = &rest[..end_pos];
            if let Some(comma) = part.find(',') {
                let start: usize = part[..comma].parse().unwrap_or(0);
                let count: usize = part[comma + 1..].parse().unwrap_or(0);
                if start > 0 && count > 0 {
                    ranges.push((start, start + count - 1));
                }
                // count == 0 means a pure deletion — no head lines, skip.
            } else {
                // "+N" with no comma — single line hunk.
                let start: usize = part.parse().unwrap_or(0);
                if start > 0 {
                    ranges.push((start, start));
                }
            }
        }
    }
    ranges
}

fn determine_change_type(status: &FileChangeStatus, patch: &str) -> ChangeType {
    match status {
        FileChangeStatus::Added => ChangeType::NewFile,
        FileChangeStatus::Deleted => ChangeType::DeletedFile,
        FileChangeStatus::Modified | FileChangeStatus::Renamed => {
            let has_additions =
                patch.lines().any(|l| l.starts_with('+') && !l.starts_with("+++"));
            if has_additions {
                ChangeType::Modified
            } else {
                ChangeType::ModifiedDeletionsOnly
            }
        }
    }
}

/// Expand each hunk range by `context_lines` on both sides, clamp to
/// `[1, total_lines]`, then merge overlapping/adjacent windows.
fn expand_and_merge_ranges(
    hunk_ranges: &[(usize, usize)],
    context_lines: usize,
    total_lines: usize,
) -> Vec<(usize, usize)> {
    if hunk_ranges.is_empty() || total_lines == 0 {
        return vec![];
    }

    let mut expanded: Vec<(usize, usize)> = hunk_ranges
        .iter()
        .map(|(start, end)| {
            let new_start = start.saturating_sub(context_lines).max(1);
            let new_end = (end + context_lines).min(total_lines);
            (new_start, new_end)
        })
        .collect();

    expanded.sort_by_key(|(s, _)| *s);

    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in expanded {
        if let Some(last) = merged.last_mut() {
            if start <= last.1 + 1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        merged.push((start, end));
    }
    merged
}

fn get_symbol_line_range(review: &Review, sym: &SymbolDiffEntry, file: &str) -> (usize, usize) {
    match &sym.change {
        SymbolChange::Deleted => review
            .base_project
            .symbol_table
            .get(file, &sym.name)
            .map(|s| s.line_range)
            .unwrap_or((0, 0)),
        _ => review
            .head_project
            .symbol_table
            .get(file, &sym.name)
            .map(|s| s.line_range)
            .unwrap_or((0, 0)),
    }
}

fn sym_to_context_entry(sym: &SymbolDiffEntry, line_range: (usize, usize)) -> ContextSymbolEntry {
    let (change, signature_changed, old_signature, new_signature, body_changed, old_file) =
        match &sym.change {
            SymbolChange::Added => (
                "added",
                None,
                None,
                None,
                None,
                None,
            ),
            SymbolChange::Deleted => (
                "deleted",
                None,
                None,
                None,
                None,
                None,
            ),
            SymbolChange::Modified {
                signature_changed,
                old_signature,
                new_signature,
                body_changed,
            } => (
                "modified",
                Some(*signature_changed),
                Some(old_signature.clone()),
                Some(new_signature.clone()),
                Some(*body_changed),
                None,
            ),
            SymbolChange::Moved {
                old_file,
                signature_changed,
                body_changed,
                ..
            } => (
                "moved",
                Some(*signature_changed),
                None,
                None,
                Some(*body_changed),
                Some(old_file.clone()),
            ),
        };

    ContextSymbolEntry {
        name: sym.name.clone(),
        kind: sym.kind,
        line_range,
        change: change.to_string(),
        signature_changed,
        old_signature,
        new_signature,
        body_changed,
        old_file,
    }
}

fn build_changed_ranges(
    hunk_ranges: &[(usize, usize)],
    symbols: &[ContextSymbolEntry],
) -> Vec<ChangedRange> {
    hunk_ranges
        .iter()
        .map(|(hunk_start, hunk_end)| {
            let overlapping: Vec<String> = symbols
                .iter()
                .filter(|s| {
                    let (sym_start, sym_end) = s.line_range;
                    sym_start > 0
                        && sym_end > 0
                        && sym_start <= *hunk_end
                        && sym_end >= *hunk_start
                })
                .map(|s| s.name.clone())
                .collect();
            ChangedRange {
                start: *hunk_start,
                end: *hunk_end,
                symbols: overlapping,
            }
        })
        .collect()
}

/// Build a source excerpt with line numbers matching the peek format.
/// `start` and `end` are 1-indexed, inclusive.
fn build_excerpt(lines: &[String], start: usize, end: usize) -> SourceExcerpt {
    let actual_start = start.max(1);
    let actual_end = end.min(lines.len());
    let content = if actual_start <= actual_end {
        lines[actual_start - 1..actual_end]
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:>6} │ {}", actual_start + i, line))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        String::new()
    };
    SourceExcerpt {
        start_line: actual_start,
        end_line: actual_end,
        content,
    }
}

// ---------------------------------------------------------------------------
// Symbol Before/After Diff
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct SymbolDiffResult {
    pub symbol: String,
    pub file: String,
    pub kind: SymbolKind,
    pub change: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_changed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_line_range: Option<(usize, usize)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_line_range: Option<(usize, usize)>,
}

/// Return a before/after source diff for a single changed symbol.
pub fn compute_symbol_diff(review: &Review, symbol: &str, file: &str) -> Result<SymbolDiffResult> {
    let diff_guard = review.diff.read();
    let diff = diff_guard.as_ref().expect("compute_symbol_diff called only when review is Ready");

    let entry = diff
        .symbol_diffs
        .iter()
        .find(|s| s.name == symbol && s.file == file)
        .ok_or_else(|| {
            anyhow!("Symbol '{}' in '{}' not found in review diff", symbol, file)
        })?;

    let kind = entry.kind;

    let (change, signature_changed, old_signature, new_signature, base_source, head_source, base_line_range, head_line_range) =
        match &entry.change {
            SymbolChange::Added => {
                let (src, lr) = get_symbol_source_and_range(
                    &review.head_project.root,
                    &review.head_project.symbol_table,
                    symbol,
                    file,
                );
                ("added", None, None, None, None, src, None, lr)
            }
            SymbolChange::Deleted => {
                let (src, lr) = get_symbol_source_and_range(
                    &review.base_project.root,
                    &review.base_project.symbol_table,
                    symbol,
                    file,
                );
                ("deleted", None, None, None, src, None, lr, None)
            }
            SymbolChange::Modified {
                signature_changed,
                old_signature,
                new_signature,
                ..
            } => {
                let (base_src, base_lr) = get_symbol_source_and_range(
                    &review.base_project.root,
                    &review.base_project.symbol_table,
                    symbol,
                    file,
                );
                let (head_src, head_lr) = get_symbol_source_and_range(
                    &review.head_project.root,
                    &review.head_project.symbol_table,
                    symbol,
                    file,
                );
                (
                    "modified",
                    Some(*signature_changed),
                    Some(old_signature.clone()),
                    Some(new_signature.clone()),
                    base_src,
                    head_src,
                    base_lr,
                    head_lr,
                )
            }
            SymbolChange::Moved {
                old_file,
                signature_changed,
                ..
            } => {
                // Base is in old_file; head is in the new file (entry.file).
                let old_file_str = old_file.as_str();
                let (base_src, base_lr) = get_symbol_source_and_range(
                    &review.base_project.root,
                    &review.base_project.symbol_table,
                    symbol,
                    old_file_str,
                );
                let (head_src, head_lr) = get_symbol_source_and_range(
                    &review.head_project.root,
                    &review.head_project.symbol_table,
                    symbol,
                    file,
                );
                (
                    "moved",
                    Some(*signature_changed),
                    None,
                    None,
                    base_src,
                    head_src,
                    base_lr,
                    head_lr,
                )
            }
        };

    // Generate unified diff for Modified/Moved symbols (where both sides exist).
    let diff_text = match (&base_source, &head_source) {
        (Some(base), Some(head)) => {
            let text_diff = similar::TextDiff::from_lines(base.as_str(), head.as_str());
            let unified = text_diff.unified_diff().header("base", "head").to_string();
            if unified.is_empty() { None } else { Some(unified) }
        }
        _ => None,
    };

    Ok(SymbolDiffResult {
        symbol: symbol.to_string(),
        file: file.to_string(),
        kind,
        change: change.to_string(),
        signature_changed,
        old_signature,
        new_signature,
        base_source,
        head_source,
        diff: diff_text,
        base_line_range,
        head_line_range,
    })
}

fn get_symbol_source_and_range(
    root: &std::path::Path,
    symbol_table: &Arc<crate::symbols::SymbolTable>,
    symbol: &str,
    file: &str,
) -> (Option<String>, Option<(usize, usize)>) {
    let line_range = symbol_table.get(file, symbol).map(|s| s.line_range);
    let source = symbol_ops::get_implementation(root, symbol_table, symbol, file).ok();
    (source, line_range)
}
