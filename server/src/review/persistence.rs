use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::{Review, ReviewDiff};

/// Serializable snapshot of a review stored on disk.
/// The computed diff is persisted so restores skip expensive recomputation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedReview {
    pub id: String,
    pub repo_root: PathBuf,
    pub base_ref: String,
    pub head_ref: String,
    pub base_commit: String,
    pub head_commit: String,
    pub created_at: DateTime<Utc>,
    pub diff: ReviewDiff,
}

fn reviews_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".coderlm").join("reviews")
}

fn review_path(repo_root: &Path, review_id: &str) -> PathBuf {
    reviews_dir(repo_root).join(format!("{}.json", review_id))
}

/// Persist a ready review to `.coderlm/reviews/{id}.json` under the repo root.
/// Called after diff computation completes.
pub fn save_review(review: &Review) {
    let diff = match review.diff.read().clone() {
        Some(d) => d,
        None => {
            warn!("Tried to save review {} but diff is not computed yet", review.id);
            return;
        }
    };

    let persisted = PersistedReview {
        id: review.id.clone(),
        repo_root: review.repo_root.clone(),
        base_ref: review.base_ref.clone(),
        head_ref: review.head_ref.clone(),
        base_commit: review.base_commit.clone(),
        head_commit: review.head_commit.read().clone(),
        created_at: review.created_at,
        diff: (*diff).clone(),
    };

    let dir = reviews_dir(&review.repo_root);
    if let Err(e) = fs::create_dir_all(&dir) {
        warn!("Failed to create reviews dir {}: {}", dir.display(), e);
        return;
    }

    let path = review_path(&review.repo_root, &review.id);
    match serde_json::to_string_pretty(&persisted) {
        Ok(json) => {
            if let Err(e) = fs::write(&path, json) {
                warn!("Failed to write review {}: {}", path.display(), e);
            } else {
                debug!("Persisted review {} to {}", review.id, path.display());
            }
        }
        Err(e) => warn!("Failed to serialize review {}: {}", review.id, e),
    }
}

/// Load all persisted reviews from `.coderlm/reviews/` under `repo_root`.
pub fn load_reviews(repo_root: &Path) -> Vec<PersistedReview> {
    let dir = reviews_dir(repo_root);

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut reviews = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        match fs::read_to_string(&path) {
            Ok(json) => match serde_json::from_str::<PersistedReview>(&json) {
                Ok(r) => {
                    info!("Loaded persisted review {} from {}", r.id, path.display());
                    reviews.push(r);
                }
                Err(e) => warn!("Failed to parse {}: {}", path.display(), e),
            },
            Err(e) => warn!("Failed to read {}: {}", path.display(), e),
        }
    }

    reviews
}

/// Delete the persisted review file for a given review ID.
pub fn delete_persisted_review(repo_root: &Path, review_id: &str) {
    let path = review_path(repo_root, review_id);
    if path.exists() {
        if let Err(e) = fs::remove_file(&path) {
            warn!("Failed to delete persisted review {}: {}", path.display(), e);
        } else {
            debug!("Deleted persisted review file {}", path.display());
        }
    }
}
