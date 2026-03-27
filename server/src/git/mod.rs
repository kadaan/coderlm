pub mod diff;
pub mod worktree;

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};

pub use diff::{diff_files, get_file_diff, FileDiff};
pub use worktree::{create_worktree, WorktreeInfo};

/// Find the root of the git repository containing `path`.
pub fn find_repo_root(path: &Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["-C", &path.to_string_lossy(), "rev-parse", "--show-toplevel"])
        .output()
        .context("failed to run git")?;

    if !output.status.success() {
        return Err(anyhow!(
            "'{}' is not inside a git repository",
            path.display()
        ));
    }

    let root = String::from_utf8(output.stdout)
        .context("git output is not valid UTF-8")?
        .trim()
        .to_string();

    Ok(PathBuf::from(root))
}

/// Resolve a ref name (branch, tag, SHA, "HEAD", etc.) to a full commit SHA.
pub fn resolve_ref(repo_root: &Path, ref_name: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["-C", &repo_root.to_string_lossy(), "rev-parse", ref_name])
        .output()
        .context("failed to run git rev-parse")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("cannot resolve ref '{}': {}", ref_name, stderr.trim()));
    }

    let sha = String::from_utf8(output.stdout)
        .context("git output is not valid UTF-8")?
        .trim()
        .to_string();

    Ok(sha)
}
