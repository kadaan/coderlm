use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use tempfile::TempDir;
use tracing::{info, warn};

/// An active git worktree. Cleans up the worktree (both the git registration
/// and the temp directory on disk) when dropped.
pub struct WorktreeInfo {
    pub path: PathBuf,
    #[allow(dead_code)]
    pub ref_name: String,
    #[allow(dead_code)]
    pub commit_sha: String,
    repo_root: PathBuf,
    // Held to keep the temp directory alive. Dropped after git worktree remove.
    _temp_dir: TempDir,
}

impl Drop for WorktreeInfo {
    fn drop(&mut self) {
        // Best-effort: deregister from git before the temp dir is deleted.
        let _ = Command::new("git")
            .args([
                "-C",
                &self.repo_root.to_string_lossy(),
                "worktree",
                "remove",
                "--force",
                &self.path.to_string_lossy(),
            ])
            .output();
    }
}

/// Create a git worktree for `commit_sha` in a new temp directory.
pub fn create_worktree(
    repo_root: &Path,
    ref_name: &str,
    commit_sha: &str,
) -> Result<WorktreeInfo> {
    let temp_dir = tempfile::Builder::new()
        .prefix(&format!("coderlm-review-{}-", &commit_sha[..8]))
        .tempdir()
        .context("failed to create temp directory for worktree")?;

    let worktree_path = temp_dir.path().to_path_buf();

    info!(
        "Creating git worktree for '{}' ({}) at '{}'",
        ref_name,
        &commit_sha[..8],
        worktree_path.display()
    );

    let output = Command::new("git")
        .args([
            "-C",
            &repo_root.to_string_lossy(),
            "worktree",
            "add",
            "--detach",
            &worktree_path.to_string_lossy(),
            commit_sha,
        ])
        .output()
        .context("failed to run git worktree add")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "git worktree add failed for '{}': {}",
            ref_name,
            stderr.trim()
        ));
    }

    info!("Worktree ready at '{}'", worktree_path.display());

    Ok(WorktreeInfo {
        path: worktree_path,
        ref_name: ref_name.to_string(),
        commit_sha: commit_sha.to_string(),
        repo_root: repo_root.to_path_buf(),
        _temp_dir: temp_dir,
    })
}

/// Scan git worktrees and prune references whose directories no longer exist.
/// Called on server startup to clean up after crashes.
#[allow(dead_code)]
pub fn cleanup_orphaned_worktrees(repo_root: &Path) {
    let _ = Command::new("git")
        .args(["-C", &repo_root.to_string_lossy(), "worktree", "prune"])
        .output();
    warn!("Pruned orphaned git worktrees for '{}'", repo_root.display());
}
