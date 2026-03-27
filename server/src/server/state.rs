use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::Mutex;
use tracing::{info, warn};
use uuid::Uuid;

use crate::git;
use crate::index::file_tree::FileTree;
use crate::index::{walker, watcher};
use crate::review::diff::compute_review_diff;
use crate::review::{Review, ReviewStatus};
use crate::server::errors::AppError;
use crate::server::session::Session;
use crate::symbols::{parser, SymbolTable};

/// Cached indexed data for a base-branch commit. Kept after a review is deleted
/// so that creating a new review against the same base commit skips re-indexing.
struct BaseCacheEntry {
    file_tree: Arc<FileTree>,
    symbol_table: Arc<SymbolTable>,
    last_used: Mutex<DateTime<Utc>>,
}

const BASE_CACHE_CAPACITY: usize = 3;

/// A single indexed project with its own file tree, symbol table, and watcher.
pub struct Project {
    pub root: PathBuf,
    pub file_tree: Arc<FileTree>,
    pub symbol_table: Arc<SymbolTable>,
    // Held alive to keep the filesystem watcher running; dropped on eviction.
    #[allow(dead_code)]
    pub watcher: Option<watcher::WatcherHandle>,
    pub last_active: Mutex<DateTime<Utc>>,
}

/// Shared application state, wrapped in Arc for axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<AppStateInner>,
}

pub struct AppStateInner {
    pub projects: DashMap<PathBuf, Arc<Project>>,
    pub sessions: DashMap<String, Session>,
    pub reviews: DashMap<String, Arc<Review>>,
    /// Cached file tree + symbol table keyed by base commit SHA. Avoids
    /// re-indexing when the same base commit is reused across reviews.
    base_cache: DashMap<String, BaseCacheEntry>,
    pub max_projects: usize,
    pub max_file_size: u64,
}

impl AppState {
    pub fn new(max_projects: usize, max_file_size: u64) -> Self {
        Self {
            inner: Arc::new(AppStateInner {
                projects: DashMap::new(),
                sessions: DashMap::new(),
                reviews: DashMap::new(),
                base_cache: DashMap::new(),
                max_projects,
                max_file_size,
            }),
        }
    }

    /// Look up an existing project or index a new one. Evicts LRU if at capacity.
    pub fn get_or_create_project(&self, cwd: &Path) -> Result<Arc<Project>, AppError> {
        let canonical = cwd.canonicalize().map_err(|e| {
            AppError::BadRequest(format!("Path not accessible: {}", e))
        })?;

        if !canonical.is_dir() {
            return Err(AppError::BadRequest(format!(
                "'{}' is not a directory",
                canonical.display()
            )));
        }

        // Return existing project if found
        if let Some(project) = self.inner.projects.get(&canonical) {
            *project.last_active.lock() = Utc::now();
            return Ok(project.clone());
        }

        // Check capacity, evict if needed
        if self.inner.projects.len() >= self.inner.max_projects {
            self.evict_lru()?;
        }

        // Scan directory
        let file_tree = Arc::new(FileTree::new());
        let symbol_table = Arc::new(SymbolTable::new());
        let max_file_size = self.inner.max_file_size;

        info!("Indexing new project: {}", canonical.display());
        let file_count =
            walker::scan_directory(&canonical, &file_tree, max_file_size)
                .map_err(|e| AppError::Internal(e.to_string()))?;
        info!("Indexed {} files for {}", file_count, canonical.display());

        // Start watcher
        let watcher_handle = watcher::start_watcher(
            &canonical,
            file_tree.clone(),
            symbol_table.clone(),
            max_file_size,
        )
        .ok();

        let project = Arc::new(Project {
            root: canonical.clone(),
            file_tree: file_tree.clone(),
            symbol_table: symbol_table.clone(),
            watcher: watcher_handle,
            last_active: Mutex::new(Utc::now()),
        });

        self.inner.projects.insert(canonical, project.clone());

        // Spawn symbol extraction in background
        let ft = file_tree;
        let st = symbol_table;
        let root = project.root.clone();
        tokio::spawn(async move {
            info!("Starting symbol extraction for {}...", root.display());
            match parser::extract_all_symbols(&root, &ft, &st).await {
                Ok(count) => info!("Extracted {} symbols for {}", count, root.display()),
                Err(e) => tracing::error!("Symbol extraction failed for {}: {}", root.display(), e),
            }
        });

        Ok(project)
    }

    /// Create a review by indexing both the head (current working directory) and
    /// base (git worktree) snapshots, then computing a semantic diff.
    ///
    /// This function performs blocking I/O (git commands, directory scanning) and
    /// should be called via `tokio::task::spawn_blocking` from async handlers.
    pub fn create_review(
        &self,
        cwd: &Path,
        base_ref: &str,
        head_ref: &str,
    ) -> Result<Arc<Review>, AppError> {
        let canonical_cwd = cwd.canonicalize().map_err(|e| {
            AppError::BadRequest(format!("Path not accessible: {}", e))
        })?;

        // Locate the git repo containing `cwd`
        let repo_root = git::find_repo_root(&canonical_cwd)
            .map_err(|e| AppError::BadRequest(e.to_string()))?;

        // Resolve both refs to commit SHAs
        let base_commit = git::resolve_ref(&repo_root, base_ref)
            .map_err(|e| AppError::BadRequest(format!("Cannot resolve base ref '{}': {}", base_ref, e)))?;
        let head_commit = git::resolve_ref(&repo_root, head_ref)
            .map_err(|e| AppError::BadRequest(format!("Cannot resolve head ref '{}': {}", head_ref, e)))?;

        info!(
            "Creating review: {}..{} ({:.8}..{:.8})",
            base_ref, head_ref, base_commit, head_commit
        );

        // Index the head project (current working directory)
        let head_project = self.get_or_create_project(&canonical_cwd)?;

        // Create the base worktree
        let worktree = git::create_worktree(&repo_root, base_ref, &base_commit)
            .map_err(|e| AppError::Internal(format!("Failed to create worktree: {}", e)))?;

        let worktree_path = worktree.path.clone();

        // Index the base project — reuse cached data if available (skips
        // re-indexing and symbol extraction for the same base commit).
        let base_project = if let Some(cached) = self.inner.base_cache.get(&base_commit) {
            *cached.last_used.lock() = Utc::now();
            let project = Arc::new(Project {
                root: worktree_path.clone(),
                file_tree: cached.file_tree.clone(),
                symbol_table: cached.symbol_table.clone(),
                watcher: None, // base is a fixed commit — no need to watch
                last_active: Mutex::new(Utc::now()),
            });
            self.inner.projects.insert(worktree_path.clone(), project.clone());
            info!("Base project {} restored from cache (skipping indexing)", &base_commit[..8]);
            project
        } else {
            self.get_or_create_project(&worktree_path)?
        };

        // Build and store the review
        let id = Uuid::new_v4().to_string();
        let review = Arc::new(Review {
            id: id.clone(),
            repo_root: repo_root.clone(),
            base_ref: base_ref.to_string(),
            head_ref: head_ref.to_string(),
            base_commit,
            head_commit: parking_lot::RwLock::new(head_commit),
            base_project: base_project.clone(),
            head_project: head_project.clone(),
            worktree,
            diff: parking_lot::RwLock::new(None),
            status: parking_lot::RwLock::new(ReviewStatus::Indexing),
            created_at: Utc::now(),
            cache: crate::review::ReviewCache::new(),
        });

        self.inner.reviews.insert(id.clone(), review.clone());

        info!("Review {} created; waiting for symbol extraction to complete", id);

        // Spawn background task: wait for symbols, then compute diff.
        // Use the owned strings stored on the review rather than the borrowed params.
        let review_weak = Arc::downgrade(&review);
        tokio::spawn(async move {
            wait_for_symbol_extraction(&base_project, &head_project).await;

            let Some(review) = review_weak.upgrade() else { return };
            *review.status.write() = ReviewStatus::Computing;

            let file_diffs = match git::diff_files(&repo_root, &review.base_ref, &review.head_ref) {
                Ok(d) => d,
                Err(e) => {
                    tracing::error!("git diff failed for review {}: {}", review.id, e);
                    *review.status.write() = ReviewStatus::Error(e.to_string());
                    return;
                }
            };

            let diff = compute_review_diff(&base_project, &head_project, &file_diffs);
            *review.diff.write() = Some(Arc::new(diff));
            *review.status.write() = ReviewStatus::Ready;

            info!("Review {} is ready", review.id);
            crate::review::persistence::save_review(&review);
        });

        Ok(review)
    }

    /// Delete a review and clean up its base project. The worktree is cleaned up
    /// automatically when the Review is dropped.
    pub fn delete_review(&self, review_id: &str) -> Result<(), AppError> {
        let (_, review) = self.inner.reviews.remove(review_id)
            .ok_or_else(|| AppError::NotFound(format!("Review '{}' not found", review_id)))?;

        // Delete the persisted review file.
        crate::review::persistence::delete_persisted_review(&review.repo_root, &review.id);

        // Cache the base project's indexed data so future reviews against
        // the same base commit can skip re-indexing.
        let base_commit = review.base_commit.clone();
        if !self.inner.base_cache.contains_key(&base_commit) {
            // Evict LRU entry if at capacity.
            if self.inner.base_cache.len() >= BASE_CACHE_CAPACITY {
                let oldest = self
                    .inner
                    .base_cache
                    .iter()
                    .min_by_key(|e| *e.value().last_used.lock())
                    .map(|e| e.key().clone());
                if let Some(key) = oldest {
                    self.inner.base_cache.remove(&key);
                }
            }
            self.inner.base_cache.insert(
                base_commit.clone(),
                BaseCacheEntry {
                    file_tree: review.base_project.file_tree.clone(),
                    symbol_table: review.base_project.symbol_table.clone(),
                    last_used: Mutex::new(Utc::now()),
                },
            );
            info!("Base project {} saved to cache", &base_commit[..8]);
        }

        // Remove the base project (worktree) from the projects map.
        // The head project is kept as it may be used by other sessions.
        let base_root = review.base_project.root.clone();
        self.inner.projects.remove(&base_root);
        self.inner.sessions.retain(|_, s| s.project_path != base_root);

        // `review` is dropped here, which drops `worktree`, which:
        //   1. Runs `git worktree remove --force`
        //   2. Drops the TempDir, deleting the directory

        Ok(())
    }

    /// Incrementally update a review to a new head commit.
    ///
    /// Compares old head against new head to find changed files, recomputes
    /// symbol diffs only for those files, and invalidates cached results.
    ///
    /// Blocking: runs git commands and reads symbol tables. Call via
    /// `tokio::task::spawn_blocking` from async handlers.
    pub fn update_review(&self, review_id: &str, new_head_ref: &str) -> Result<(), AppError> {
        let review = self
            .inner
            .reviews
            .get(review_id)
            .ok_or_else(|| AppError::NotFound(format!("Review '{}' not found", review_id)))?
            .clone();

        let new_head_commit = git::resolve_ref(&review.repo_root, new_head_ref).map_err(|e| {
            AppError::BadRequest(format!("Cannot resolve ref '{}': {}", new_head_ref, e))
        })?;

        let old_head_commit = review.head_commit.read().clone();

        // Nothing to do if already up to date.
        if new_head_commit == old_head_commit {
            return Ok(());
        }

        *review.status.write() = ReviewStatus::Updating;

        // Files changed between old and new head (to know what to recompute).
        let changed_since =
            git::diff_files(&review.repo_root, &old_head_commit, &new_head_commit).map_err(
                |e| {
                    *review.status.write() = ReviewStatus::Ready;
                    AppError::Internal(format!("git diff failed: {}", e))
                },
            )?;
        let changed_paths: HashSet<String> =
            changed_since.iter().map(|f| f.path.clone()).collect();

        // Full diff from base_ref to new head (authoritative file list).
        let full_base_diffs =
            git::diff_files(&review.repo_root, &review.base_ref, new_head_ref).map_err(|e| {
                *review.status.write() = ReviewStatus::Ready;
                AppError::Internal(format!("git diff failed: {}", e))
            })?;

        let old_diff = review
            .diff
            .read()
            .clone()
            .expect("review in Updating state must have a diff");

        let new_diff = crate::review::diff::compute_incremental_diff(
            &old_diff,
            &review.base_project,
            &review.head_project,
            &full_base_diffs,
            &changed_paths,
        );

        // Invalidate cached results for affected files.
        let changed_path_vec: Vec<String> = changed_paths.into_iter().collect();
        review.cache.invalidate_files(&changed_path_vec);

        // Atomically update head_commit, diff, and status.
        *review.head_commit.write() = new_head_commit;
        *review.diff.write() = Some(Arc::new(new_diff));
        *review.status.write() = ReviewStatus::Ready;

        info!("Review {} updated to {}", review_id, new_head_ref);
        crate::review::persistence::save_review(&review);

        Ok(())
    }

    /// Restore reviews that were persisted to disk in a previous server run.
    /// Should be called once at startup before serving traffic.
    pub async fn restore_reviews(&self, search_path: &Path) {
        let repo_root = match git::find_repo_root(search_path) {
            Ok(r) => r,
            Err(_) => return,
        };

        let persisted = crate::review::persistence::load_reviews(&repo_root);
        if persisted.is_empty() {
            return;
        }

        info!("Restoring {} persisted review(s) for {}", persisted.len(), repo_root.display());

        let state = self.clone();
        tokio::task::spawn_blocking(move || {
            for pr in persisted {
                let id = pr.id.clone();
                if let Err(e) = state.restore_one_review(pr) {
                    warn!("Failed to restore review {}: {}", id, e);
                }
            }
        })
        .await
        .ok();
    }

    /// Synchronously restore a single persisted review. Called from spawn_blocking.
    fn restore_one_review(
        &self,
        pr: crate::review::persistence::PersistedReview,
    ) -> Result<(), AppError> {
        // Verify both commits still exist in the repo.
        if !git_commit_exists(&pr.repo_root, &pr.base_commit)
            || !git_commit_exists(&pr.repo_root, &pr.head_commit)
        {
            warn!("Review {}: commits no longer exist, removing persisted file", pr.id);
            crate::review::persistence::delete_persisted_review(&pr.repo_root, &pr.id);
            return Ok(());
        }

        // Re-create the worktree for the base commit.
        let worktree = git::create_worktree(&pr.repo_root, &pr.base_ref, &pr.base_commit)
            .map_err(|e| AppError::Internal(format!("Failed to re-create worktree: {}", e)))?;

        let worktree_path = worktree.path.clone();

        // Index both projects — reuse cached base data if available.
        let head_project = self.get_or_create_project(&pr.repo_root)?;
        let base_project = if let Some(cached) = self.inner.base_cache.get(&pr.base_commit) {
            *cached.last_used.lock() = Utc::now();
            let project = Arc::new(Project {
                root: worktree_path.clone(),
                file_tree: cached.file_tree.clone(),
                symbol_table: cached.symbol_table.clone(),
                watcher: None,
                last_active: Mutex::new(Utc::now()),
            });
            self.inner.projects.insert(worktree_path.clone(), project.clone());
            project
        } else {
            self.get_or_create_project(&worktree_path)?
        };

        let id = pr.id.clone();
        let review = Arc::new(Review {
            id: pr.id,
            repo_root: pr.repo_root,
            base_ref: pr.base_ref,
            head_ref: pr.head_ref,
            base_commit: pr.base_commit,
            head_commit: parking_lot::RwLock::new(pr.head_commit),
            base_project: base_project.clone(),
            head_project: head_project.clone(),
            worktree,
            diff: parking_lot::RwLock::new(Some(Arc::new(pr.diff))),
            status: parking_lot::RwLock::new(ReviewStatus::Indexing),
            created_at: pr.created_at,
            cache: crate::review::ReviewCache::new(),
        });

        self.inner.reviews.insert(id.clone(), review.clone());
        info!("Review {} restored; waiting for symbol extraction", id);

        // Spawn a background task: wait for symbol extraction then set Ready.
        let review_weak = Arc::downgrade(&review);
        tokio::spawn(async move {
            wait_for_symbol_extraction(&base_project, &head_project).await;
            if let Some(review) = review_weak.upgrade() {
                *review.status.write() = ReviewStatus::Ready;
                info!("Restored review {} is ready", review.id);
            }
        });

        Ok(())
    }

    /// Look up the project for a given session. Returns a descriptive error if
    /// the project has been evicted.
    pub fn get_project_for_session(&self, session_id: &str) -> Result<Arc<Project>, AppError> {
        let session = self
            .inner
            .sessions
            .get(session_id)
            .ok_or_else(|| AppError::NotFound(format!("Session '{}' not found", session_id)))?;

        let project_path = &session.project_path;

        let project = self
            .inner
            .projects
            .get(project_path)
            .ok_or_else(|| {
                AppError::Gone(format!(
                    "Project at '{}' was evicted due to capacity limits. \
                     Start a new session to re-index, or increase --max-projects.",
                    project_path.display()
                ))
            })?;

        Ok(project.clone())
    }

    /// Look up the review attached to a session.
    pub fn get_review_for_session(&self, session_id: &str) -> Result<Arc<Review>, AppError> {
        let session = self
            .inner
            .sessions
            .get(session_id)
            .ok_or_else(|| AppError::NotFound(format!("Session '{}' not found", session_id)))?;

        let review_id = session
            .review_id
            .as_ref()
            .ok_or_else(|| AppError::BadRequest(
                "No review attached to this session. Use POST /reviews/{id}/attach first.".into(),
            ))?;

        let review = self
            .inner
            .reviews
            .get(review_id)
            .ok_or_else(|| AppError::Gone(
                "The review was deleted. Create a new one with POST /reviews.".into(),
            ))?;

        Ok(review.clone())
    }

    /// Update the last-active timestamp on a project.
    pub fn touch_project(&self, project_path: &Path) {
        if let Some(project) = self.inner.projects.get(project_path) {
            *project.last_active.lock() = Utc::now();
        }
    }

    /// Evict the least recently used project. Removes all sessions pointing to it.
    fn evict_lru(&self) -> Result<(), AppError> {
        // Find the project with the oldest last_active
        let oldest = self
            .inner
            .projects
            .iter()
            .min_by_key(|entry| *entry.value().last_active.lock())
            .map(|entry| entry.key().clone());

        let path = oldest.ok_or_else(|| {
            AppError::Internal("No projects to evict".into())
        })?;

        info!("Evicting project: {}", path.display());

        // Remove the project (drops watcher)
        self.inner.projects.remove(&path);

        // Remove all sessions attached to this project
        self.inner.sessions.retain(|_, session| session.project_path != path);

        Ok(())
    }
}

/// Poll both file trees until all supported-language files have had symbols
/// extracted, or until a timeout is reached (30 seconds).
async fn wait_for_symbol_extraction(base: &Arc<Project>, head: &Arc<Project>) {
    let timeout = Duration::from_secs(30);
    let poll_interval = Duration::from_millis(500);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() >= timeout {
            tracing::warn!("Timed out waiting for symbol extraction; computing diff with available symbols");
            break;
        }

        let base_done = symbols_extracted(&base.file_tree);
        let head_done = symbols_extracted(&head.file_tree);

        if base_done && head_done {
            break;
        }

        tokio::time::sleep(poll_interval).await;
    }
}

/// Returns true when all tree-sitter-supported files have been processed.
fn symbols_extracted(file_tree: &FileTree) -> bool {
    file_tree
        .files
        .iter()
        .filter(|e| e.value().language.has_tree_sitter_support())
        .all(|e| e.value().symbols_extracted)
}

/// Returns true if the given commit SHA exists in the repo.
fn git_commit_exists(repo_root: &Path, sha: &str) -> bool {
    std::process::Command::new("git")
        .args(["-C", &repo_root.to_string_lossy(), "cat-file", "-t", sha])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
