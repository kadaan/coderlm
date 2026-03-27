use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum FileStatus {
    Added,
    Deleted,
    Modified,
    Renamed { from: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct FileDiff {
    pub path: String,
    pub status: FileStatus,
}

/// Return the list of files changed between `base_ref` and `head_ref`.
/// Uses rename detection (`-M`) so renames appear as Renamed rather than
/// Deleted + Added.
pub fn diff_files(repo_root: &Path, base_ref: &str, head_ref: &str) -> Result<Vec<FileDiff>> {
    let range = format!("{}...{}", base_ref, head_ref);

    let output = Command::new("git")
        .args([
            "-C",
            &repo_root.to_string_lossy(),
            "diff",
            "--name-status",
            "-M",   // detect renames
            &range,
        ])
        .output()
        .context("failed to run git diff")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "git diff failed for '{}': {}",
            range,
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8(output.stdout).context("git diff output is not valid UTF-8")?;
    parse_name_status(&stdout)
}

/// Return the unified diff text for a single file between `base_ref` and
/// `head_ref`.
pub fn get_file_diff(
    repo_root: &Path,
    base_ref: &str,
    head_ref: &str,
    file: &str,
) -> Result<String> {
    let range = format!("{}...{}", base_ref, head_ref);

    let output = Command::new("git")
        .args([
            "-C",
            &repo_root.to_string_lossy(),
            "diff",
            &range,
            "--",
            file,
        ])
        .output()
        .context("failed to run git diff for file")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "git diff failed for file '{}': {}",
            file,
            stderr.trim()
        ));
    }

    String::from_utf8(output.stdout).context("git diff output is not valid UTF-8")
}

/// Parse the output of `git diff --name-status -M` into a list of FileDiff.
///
/// Format per line:
///   A   path/added.rs
///   D   path/deleted.rs
///   M   path/modified.rs
///   R100  old/path.rs  new/path.rs   (rename with 100% similarity)
fn parse_name_status(output: &str) -> Result<Vec<FileDiff>> {
    let mut diffs = Vec::new();

    for line in output.lines() {
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        if parts.is_empty() {
            continue;
        }

        let status_code = parts[0];

        let diff = if status_code == "A" {
            let path = parts.get(1).copied().unwrap_or("").to_string();
            FileDiff { path, status: FileStatus::Added }
        } else if status_code == "D" {
            let path = parts.get(1).copied().unwrap_or("").to_string();
            FileDiff { path, status: FileStatus::Deleted }
        } else if status_code == "M" {
            let path = parts.get(1).copied().unwrap_or("").to_string();
            FileDiff { path, status: FileStatus::Modified }
        } else if status_code.starts_with('R') {
            // R100\told_path\tnew_path
            let from = parts.get(1).copied().unwrap_or("").to_string();
            let path = parts.get(2).copied().unwrap_or("").to_string();
            FileDiff {
                path,
                status: FileStatus::Renamed { from },
            }
        } else {
            // C (copy), T (type change), U (unmerged) — treat as modified
            let path = parts.get(1).copied().unwrap_or("").to_string();
            FileDiff { path, status: FileStatus::Modified }
        };

        if !diff.path.is_empty() {
            diffs.push(diff);
        }
    }

    Ok(diffs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_name_status() {
        let input = "A\tsrc/new.rs\nD\tsrc/old.rs\nM\tsrc/existing.rs\nR100\told_name.rs\tnew_name.rs\n";
        let diffs = parse_name_status(input).unwrap();
        assert_eq!(diffs.len(), 4);
        assert!(matches!(diffs[0].status, FileStatus::Added));
        assert!(matches!(diffs[1].status, FileStatus::Deleted));
        assert!(matches!(diffs[2].status, FileStatus::Modified));
        assert!(matches!(diffs[3].status, FileStatus::Renamed { .. }));
        assert_eq!(diffs[3].path, "new_name.rs");
    }
}
