use std::sync::Arc;

use dashmap::DashMap;
use parking_lot::RwLock;

use super::context::FileContextResult;
use super::{ImpactResult, SafetyReport, TestCoverageReport};

/// Per-review result cache. Reviews are immutable snapshots (base_commit +
/// head_commit are pinned), so cached values never need invalidation during
/// a review's lifetime.
pub struct ReviewCache {
    /// Unified diff text per file: file_path -> raw diff string.
    pub file_diffs: DashMap<String, String>,
    /// File context bundle per file: file_path -> FileContextResult.
    pub file_contexts: DashMap<String, Arc<FileContextResult>>,
    /// Safety report (expensive: calls find_callers for every risky symbol).
    pub safety_report: RwLock<Option<Arc<SafetyReport>>>,
    /// Test coverage report (expensive: calls find_tests for every changed symbol).
    pub test_coverage: RwLock<Option<Arc<TestCoverageReport>>>,
    /// Per-symbol impact results: "file::symbol" -> ImpactResult.
    pub impact_results: DashMap<String, Arc<ImpactResult>>,
}

impl ReviewCache {
    pub fn new() -> Self {
        Self {
            file_diffs: DashMap::new(),
            file_contexts: DashMap::new(),
            safety_report: RwLock::new(None),
            test_coverage: RwLock::new(None),
            impact_results: DashMap::new(),
        }
    }

    /// Key format used for impact_results.
    pub fn impact_key(file: &str, symbol: &str) -> String {
        format!("{}::{}", file, symbol)
    }

    /// Clears cached safety, test coverage, and any impact results for symbols
    /// in the given files. Used by incremental update to invalidate stale entries.
    pub fn invalidate_files(&self, files: &[String]) {
        let file_set: std::collections::HashSet<&str> =
            files.iter().map(String::as_str).collect();

        // Clear whole-review reports since they aggregate across all files.
        *self.safety_report.write() = None;
        *self.test_coverage.write() = None;

        // Remove file-diff and file-context cache entries for affected files.
        for f in files {
            self.file_diffs.remove(f);
            self.file_contexts.remove(f);
        }

        // Remove per-symbol impact results for symbols in affected files.
        // Impact key format is "file::symbol", so we check the prefix.
        self.impact_results.retain(|key, _| {
            let file_part = key.split("::").next().unwrap_or("");
            !file_set.contains(file_part)
        });
    }
}
