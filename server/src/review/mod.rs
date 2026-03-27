pub mod cache;
pub mod diff;
pub mod impact;
pub mod persistence;

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::git::WorktreeInfo;
use crate::index::file_entry::Language;
use crate::server::state::Project;
use crate::symbols::symbol::SymbolKind;

pub use cache::ReviewCache;
pub use diff::ReviewDiff;

// ---------------------------------------------------------------------------
// Review entity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Indexing,
    Computing,
    /// Review is usable but an incremental update is in progress.
    Updating,
    Ready,
    Error(String),
}

pub struct Review {
    pub id: String,
    pub repo_root: PathBuf,
    pub base_ref: String,
    pub head_ref: String,
    pub base_commit: String,
    /// The most recently confirmed head commit SHA. Updated by `update_review`.
    pub head_commit: RwLock<String>,
    pub base_project: Arc<Project>,
    pub head_project: Arc<Project>,
    /// Owns the worktree; dropped (cleaned up) when the Review is dropped.
    #[allow(dead_code)]
    pub worktree: WorktreeInfo,
    pub diff: RwLock<Option<Arc<ReviewDiff>>>,
    pub status: RwLock<ReviewStatus>,
    pub created_at: DateTime<Utc>,
    pub cache: ReviewCache,
}

// ---------------------------------------------------------------------------
// Diff structures
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiffEntry {
    pub path: String,
    pub status: FileChangeStatus,
    /// Previous path for renames.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    pub language: Language,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum SymbolChange {
    Added,
    Deleted,
    Modified {
        signature_changed: bool,
        old_signature: String,
        new_signature: String,
        body_changed: bool,
    },
    Moved {
        old_file: String,
        /// Set when the symbol was renamed in addition to being moved.
        #[serde(skip_serializing_if = "Option::is_none")]
        old_name: Option<String>,
        signature_changed: bool,
        body_changed: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolDiffEntry {
    pub name: String,
    pub file: String,
    pub kind: SymbolKind,
    #[serde(flatten)]
    pub change: SymbolChange,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiffStats {
    pub files_added: usize,
    pub files_deleted: usize,
    pub files_modified: usize,
    pub files_renamed: usize,
    pub symbols_added: usize,
    pub symbols_deleted: usize,
    pub symbols_modified: usize,
    pub symbols_moved: usize,
}

// ---------------------------------------------------------------------------
// Impact / cross-reference structures
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct CallerWithRisk {
    pub file: String,
    pub line: usize,
    pub text: String,
    /// True if this caller's file was also modified in the PR (likely adapted).
    pub also_modified_in_pr: bool,
    /// How many hops away from the changed symbol (1 = direct caller).
    pub depth: usize,
    /// Intermediate symbol name for depth > 1 (the function that calls the changed symbol).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactResult {
    pub symbol: String,
    pub file: String,
    #[serde(flatten)]
    pub change: SymbolChange,
    pub base_callers: Vec<CallerWithRisk>,
    pub unmodified_callers_count: usize,
    pub tests: Vec<TestWithStatus>,
    pub risk: RiskLevel,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestWithStatus {
    pub name: String,
    pub file: String,
    pub line: usize,
    pub also_modified_in_pr: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SafetyIssue {
    pub symbol: String,
    pub file: String,
    #[serde(flatten)]
    pub change: SymbolChange,
    pub unmodified_callers: Vec<CallerWithRisk>,
    pub risk: RiskLevel,
}

#[derive(Debug, Clone, Serialize)]
pub struct SafetyReport {
    pub issues: Vec<SafetyIssue>,
    pub safe_changes_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoveredSymbol {
    pub symbol: String,
    pub file: String,
    pub kind: SymbolKind,
    pub tests: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UncoveredSymbol {
    pub symbol: String,
    pub file: String,
    pub kind: SymbolKind,
    pub change: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestCoverageReport {
    pub covered: Vec<CoveredSymbol>,
    pub uncovered: Vec<UncoveredSymbol>,
    pub coverage_ratio: String,
}
