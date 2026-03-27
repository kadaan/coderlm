use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::git::FileDiff;
use crate::git::diff::FileStatus as GitFileStatus;
use crate::index::file_entry::Language;
use crate::server::state::Project;
use crate::symbols::symbol::Symbol;

use super::{DiffStats, FileChangeStatus, FileDiffEntry, SymbolChange, SymbolDiffEntry};
use crate::symbols::symbol::SymbolKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewDiff {
    pub file_diffs: Vec<FileDiffEntry>,
    pub symbol_diffs: Vec<SymbolDiffEntry>,
    pub stats: DiffStats,
}

/// Incrementally update a review diff after the head has advanced.
///
/// `full_base_git_diffs` is the full diff from `base_ref` to the new head ref.
/// `changed_paths` is the set of file paths that changed between the old head
/// and the new head — only these files are re-processed; all others reuse their
/// existing symbol diffs.
pub fn compute_incremental_diff(
    old_diff: &ReviewDiff,
    base_project: &Project,
    head_project: &Project,
    full_base_git_diffs: &[FileDiff],
    changed_paths: &HashSet<String>,
) -> ReviewDiff {
    // Index old symbol diffs and file entries by path for fast lookup.
    let mut old_syms_by_file: HashMap<String, Vec<SymbolDiffEntry>> = HashMap::new();
    for sym in &old_diff.symbol_diffs {
        old_syms_by_file.entry(sym.file.clone()).or_default().push(sym.clone());
    }
    let old_file_entries: HashMap<&str, &FileDiffEntry> = old_diff
        .file_diffs
        .iter()
        .map(|e| (e.path.as_str(), e))
        .collect();

    let mut file_diffs: Vec<FileDiffEntry> = Vec::new();
    let mut symbol_diffs: Vec<SymbolDiffEntry> = Vec::new();

    for git_diff in full_base_git_diffs {
        let path = &git_diff.path;
        let needs_recompute = changed_paths.contains(path.as_str())
            || !old_file_entries.contains_key(path.as_str());

        if needs_recompute {
            let (entry, sym_changes) = process_file_diff(git_diff, base_project, head_project);
            file_diffs.push(entry);
            symbol_diffs.extend(sym_changes);
        } else {
            // File unchanged since last update: reuse the existing entries.
            let entry = (*old_file_entries[path.as_str()]).clone();
            file_diffs.push(entry);
            if let Some(old_syms) = old_syms_by_file.remove(path) {
                symbol_diffs.extend(old_syms);
            }
        }
    }

    detect_moves(&mut symbol_diffs, base_project, head_project);

    let mut stats = DiffStats::default();
    for fd in &file_diffs {
        match fd.status {
            FileChangeStatus::Added => stats.files_added += 1,
            FileChangeStatus::Deleted => stats.files_deleted += 1,
            FileChangeStatus::Modified => stats.files_modified += 1,
            FileChangeStatus::Renamed => stats.files_renamed += 1,
        }
    }
    for sym in &symbol_diffs {
        match &sym.change {
            SymbolChange::Added => stats.symbols_added += 1,
            SymbolChange::Deleted => stats.symbols_deleted += 1,
            SymbolChange::Modified { .. } => stats.symbols_modified += 1,
            SymbolChange::Moved { .. } => stats.symbols_moved += 1,
        }
    }

    ReviewDiff { file_diffs, symbol_diffs, stats }
}

/// Compute the full semantic diff between `base_project` and `head_project`
/// using the file-level `git_diffs` to scope which files to examine.
pub fn compute_review_diff(
    base_project: &Project,
    head_project: &Project,
    git_diffs: &[FileDiff],
) -> ReviewDiff {
    let mut file_diffs: Vec<FileDiffEntry> = Vec::new();
    let mut symbol_diffs: Vec<SymbolDiffEntry> = Vec::new();

    for git_diff in git_diffs {
        let (entry, sym_changes) = process_file_diff(git_diff, base_project, head_project);
        file_diffs.push(entry);
        symbol_diffs.extend(sym_changes);
    }

    // Cross-file move/rename detection: converts Deleted+Added pairs into Moved.
    detect_moves(&mut symbol_diffs, base_project, head_project);

    // Compute stats from the final (post-move-detection) symbol list.
    let mut stats = DiffStats::default();
    for fd in &file_diffs {
        match fd.status {
            FileChangeStatus::Added => stats.files_added += 1,
            FileChangeStatus::Deleted => stats.files_deleted += 1,
            FileChangeStatus::Modified => stats.files_modified += 1,
            FileChangeStatus::Renamed => stats.files_renamed += 1,
        }
    }
    for sym in &symbol_diffs {
        match &sym.change {
            SymbolChange::Added => stats.symbols_added += 1,
            SymbolChange::Deleted => stats.symbols_deleted += 1,
            SymbolChange::Modified { .. } => stats.symbols_modified += 1,
            SymbolChange::Moved { .. } => stats.symbols_moved += 1,
        }
    }

    ReviewDiff { file_diffs, symbol_diffs, stats }
}

/// Helper data for a detected cross-file symbol move.
struct MoveInfo {
    del_idx: usize,
    new_file: String,
    kind: SymbolKind,
    old_file: String,
    signature_changed: bool,
    body_changed: bool,
}

/// Post-processing pass: find cross-file moves by matching Deleted+Added pairs
/// with the same symbol name in different files. Replaces the matched pair with
/// a single `Moved` entry.
fn detect_moves(
    symbol_diffs: &mut Vec<SymbolDiffEntry>,
    base_project: &Project,
    head_project: &Project,
) {
    // Build name → first-occurrence index for Deleted and Added entries.
    let mut deleted_by_name: HashMap<String, usize> = HashMap::new();
    let mut added_by_name: HashMap<String, usize> = HashMap::new();

    for (i, diff) in symbol_diffs.iter().enumerate() {
        match &diff.change {
            SymbolChange::Deleted => {
                deleted_by_name.entry(diff.name.clone()).or_insert(i);
            }
            SymbolChange::Added => {
                added_by_name.entry(diff.name.clone()).or_insert(i);
            }
            _ => {}
        }
    }

    let mut move_infos: Vec<MoveInfo> = Vec::new();
    let mut matched_added: HashSet<usize> = HashSet::new();

    for (name, &del_idx) in &deleted_by_name {
        if let Some(&add_idx) = added_by_name.get(name.as_str()) {
            let old_file = symbol_diffs[del_idx].file.clone();
            let new_file = symbol_diffs[add_idx].file.clone();
            let kind = symbol_diffs[del_idx].kind;

            // Only treat as a move if the files differ.
            if old_file == new_file {
                continue;
            }

            let (signature_changed, body_changed) = compare_symbol_bodies(
                name, &old_file, &new_file, base_project, head_project,
            );

            move_infos.push(MoveInfo {
                del_idx,
                new_file,
                kind,
                old_file,
                signature_changed,
                body_changed,
            });
            matched_added.insert(add_idx);
        }
    }

    // Apply moves: update the Deleted entries to Moved, pointing to the new file.
    for m in move_infos {
        let name = symbol_diffs[m.del_idx].name.clone();
        symbol_diffs[m.del_idx] = SymbolDiffEntry {
            name,
            file: m.new_file,
            kind: m.kind,
            change: SymbolChange::Moved {
                old_file: m.old_file,
                old_name: None,
                signature_changed: m.signature_changed,
                body_changed: m.body_changed,
            },
        };
    }

    // Remove the now-redundant Added entries (highest index first).
    let mut indices_to_remove: Vec<usize> = matched_added.into_iter().collect();
    indices_to_remove.sort_unstable_by(|a, b| b.cmp(a));
    for i in indices_to_remove {
        symbol_diffs.remove(i);
    }
}

/// Compare signature and body of a symbol across base/head projects.
fn compare_symbol_bodies(
    name: &str,
    old_file: &str,
    new_file: &str,
    base_project: &Project,
    head_project: &Project,
) -> (bool, bool) {
    let base_sym = base_project.symbol_table.get(old_file, name);
    let head_sym = head_project.symbol_table.get(new_file, name);

    match (base_sym, head_sym) {
        (Some(b), Some(h)) => {
            let sig_changed = b.signature != h.signature;
            let body_changed =
                sig_changed || body_hash(&b, &base_project.root) != body_hash(&h, &head_project.root);
            (sig_changed, body_changed)
        }
        _ => (false, false),
    }
}

fn process_file_diff(
    git_diff: &FileDiff,
    base_project: &Project,
    head_project: &Project,
) -> (FileDiffEntry, Vec<SymbolDiffEntry>) {
    match &git_diff.status {
        GitFileStatus::Added => {
            let language = language_for_file(&git_diff.path, head_project);
            let head_syms = symbols_for_file(&git_diff.path, head_project);
            let sym_diffs = head_syms
                .into_iter()
                .map(|s| SymbolDiffEntry {
                    name: s.name,
                    file: git_diff.path.clone(),
                    kind: s.kind,
                    change: SymbolChange::Added,
                })
                .collect();
            let entry = FileDiffEntry {
                path: git_diff.path.clone(),
                status: FileChangeStatus::Added,
                old_path: None,
                language,
            };
            (entry, sym_diffs)
        }

        GitFileStatus::Deleted => {
            let language = language_for_file(&git_diff.path, base_project);
            let base_syms = symbols_for_file(&git_diff.path, base_project);
            let sym_diffs = base_syms
                .into_iter()
                .map(|s| SymbolDiffEntry {
                    name: s.name,
                    file: git_diff.path.clone(),
                    kind: s.kind,
                    change: SymbolChange::Deleted,
                })
                .collect();
            let entry = FileDiffEntry {
                path: git_diff.path.clone(),
                status: FileChangeStatus::Deleted,
                old_path: None,
                language,
            };
            (entry, sym_diffs)
        }

        GitFileStatus::Modified => {
            let language = language_for_file(&git_diff.path, head_project);
            let base_syms = symbols_for_file(&git_diff.path, base_project);
            let head_syms = symbols_for_file(&git_diff.path, head_project);
            let sym_diffs =
                diff_file_symbols(&base_syms, &head_syms, base_project, head_project, &git_diff.path);
            let entry = FileDiffEntry {
                path: git_diff.path.clone(),
                status: FileChangeStatus::Modified,
                old_path: None,
                language,
            };
            (entry, sym_diffs)
        }

        GitFileStatus::Renamed { from } => {
            let language = language_for_file(&git_diff.path, head_project);
            // Treat as: old symbols deleted from `from`, new symbols added at `path`.
            // Simple approach — diff by name between the two files.
            let base_syms = symbols_for_file(from, base_project);
            let head_syms = symbols_for_file(&git_diff.path, head_project);
            let sym_diffs =
                diff_file_symbols(&base_syms, &head_syms, base_project, head_project, &git_diff.path);
            let entry = FileDiffEntry {
                path: git_diff.path.clone(),
                status: FileChangeStatus::Renamed,
                old_path: Some(from.clone()),
                language,
            };
            (entry, sym_diffs)
        }
    }
}

/// Compute symbol-level changes between the base and head versions of a file.
pub fn diff_file_symbols(
    base_syms: &[Symbol],
    head_syms: &[Symbol],
    base_project: &Project,
    head_project: &Project,
    file_path: &str,
) -> Vec<SymbolDiffEntry> {
    let base_map: HashMap<&str, &Symbol> = base_syms.iter().map(|s| (s.name.as_str(), s)).collect();
    let head_map: HashMap<&str, &Symbol> = head_syms.iter().map(|s| (s.name.as_str(), s)).collect();

    let mut diffs = Vec::new();

    // Symbols in head but not base → Added
    for sym in head_syms {
        if !base_map.contains_key(sym.name.as_str()) {
            diffs.push(SymbolDiffEntry {
                name: sym.name.clone(),
                file: file_path.to_string(),
                kind: sym.kind,
                change: SymbolChange::Added,
            });
        }
    }

    // Symbols in base but not head → Deleted
    for sym in base_syms {
        if !head_map.contains_key(sym.name.as_str()) {
            diffs.push(SymbolDiffEntry {
                name: sym.name.clone(),
                file: file_path.to_string(),
                kind: sym.kind,
                change: SymbolChange::Deleted,
            });
        }
    }

    // Symbols in both → compare
    for sym in head_syms {
        if let Some(base_sym) = base_map.get(sym.name.as_str()) {
            let signature_changed = base_sym.signature != sym.signature;
            let body_changed = signature_changed
                || body_hash(base_sym, &base_project.root) != body_hash(sym, &head_project.root);

            if signature_changed || body_changed {
                diffs.push(SymbolDiffEntry {
                    name: sym.name.clone(),
                    file: file_path.to_string(),
                    kind: sym.kind,
                    change: SymbolChange::Modified {
                        signature_changed,
                        old_signature: base_sym.signature.clone(),
                        new_signature: sym.signature.clone(),
                        body_changed,
                    },
                });
            }
        }
    }

    diffs
}

fn symbols_for_file(file: &str, project: &Project) -> Vec<Symbol> {
    project.symbol_table.list_by_file(file)
}

fn language_for_file(file: &str, project: &Project) -> Language {
    project
        .file_tree
        .get(file)
        .map(|e| e.language)
        .unwrap_or(Language::Other)
}

/// Compute a simple hash of the symbol body by reading its byte range.
/// Returns 0 on any read error so the body is treated as unchanged.
fn body_hash(sym: &Symbol, root: &Path) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let abs_path = root.join(&sym.file);
    let Ok(source) = std::fs::read(&abs_path) else { return 0 };
    let (start, end) = sym.byte_range;
    let end = end.min(source.len());
    if start >= end { return 0; }
    let body = &source[start..end];
    let mut hasher = DefaultHasher::new();
    body.hash(&mut hasher);
    hasher.finish()
}
