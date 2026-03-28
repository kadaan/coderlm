use std::path::PathBuf;
use std::sync::Arc;

use rmcp::{
    ErrorData, ServerHandler, tool, tool_handler, tool_router,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo},
};

use crate::ops::{annotations, content, history, structure, symbol_ops};
use crate::review::{self, ReviewStatus};
use crate::server::errors::AppError;
use crate::server::session::Session;
use crate::server::state::{AppState, Project};
use crate::symbols::symbol::SymbolKind;

use super::error;
use super::params::*;

// ---------------------------------------------------------------------------
// McpServer struct
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct McpServer {
    state: AppState,
    session_id: String,
    project_root: PathBuf,
    /// Active review for diff query tools. Shared via Arc so clones stay in sync.
    active_review_id: Arc<parking_lot::RwLock<Option<String>>>,
    tool_router: ToolRouter<McpServer>,
}

// ---------------------------------------------------------------------------
// Constructor and helper methods
// ---------------------------------------------------------------------------

impl McpServer {
    pub async fn new(path: PathBuf, max_file_size: u64) -> anyhow::Result<Self> {
        let state = AppState::new(5, max_file_size);

        // Index the project directory.
        let project = state.get_or_create_project(&path)
            .map_err(|e| anyhow::anyhow!("Failed to index '{}': {}", path.display(), e))?;

        // Restore any reviews persisted from a previous run.
        state.restore_reviews(&path).await;

        // Auto-create a session.
        let session_id = uuid::Uuid::new_v4().to_string();
        let session = Session::new(session_id.clone(), project.root.clone());
        state.inner.sessions.insert(session_id.clone(), session);

        // Load annotations after symbol extraction has had a moment to start.
        let ft = project.file_tree.clone();
        let st = project.symbol_table.clone();
        let root = project.root.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let _ = annotations::load_annotations(&root, &ft, &st);
        });

        let project_root = project.root.clone();
        let tool_router = Self::tool_router();

        Ok(Self {
            state,
            session_id,
            project_root,
            active_review_id: Arc::new(parking_lot::RwLock::new(None)),
            tool_router,
        })
    }

    // --- Internal helpers ---

    fn get_project(&self) -> Result<Arc<Project>, AppError> {
        self.state.get_project_for_session(&self.session_id)
    }

    fn get_review(&self) -> Result<Arc<review::Review>, AppError> {
        let review_id = self.active_review_id.read().clone().ok_or_else(|| {
            AppError::BadRequest(
                "No active review. Use create_review or set_active_review first.".into(),
            )
        })?;

        let review = self
            .state
            .inner
            .reviews
            .get(&review_id)
            .ok_or_else(|| AppError::Gone("Active review was deleted.".into()))?
            .clone();

        let status = review.status.read();
        match &*status {
            ReviewStatus::Ready | ReviewStatus::Updating => {}
            ReviewStatus::Indexing => {
                return Err(AppError::BadRequest(
                    "Review is not ready yet (status: indexing). Call get_review to check status.".into(),
                ))
            }
            ReviewStatus::Computing => {
                return Err(AppError::BadRequest(
                    "Review is not ready yet (status: computing). Call get_review to check status.".into(),
                ))
            }
            ReviewStatus::Error(msg) => return Err(AppError::Internal(msg.clone())),
        }
        drop(status);

        Ok(review)
    }

    fn resolve_project_for_branch(&self, branch: Option<&str>) -> Result<Arc<Project>, AppError> {
        if branch == Some("base") {
            let review = self.get_review()?;
            Ok(review.base_project.clone())
        } else {
            self.get_project()
        }
    }

    fn record_history(&self, method: &str, path: &str, preview: &str) {
        if let Some(mut session) = self.state.inner.sessions.get_mut(&self.session_id) {
            session.record(method, path, preview);
        }
    }

    fn json_result<T: serde::Serialize>(&self, value: &T) -> Result<CallToolResult, ErrorData> {
        Ok(match serde_json::to_string_pretty(value) {
            Ok(s) => CallToolResult::success(vec![Content::text(s)]),
            Err(e) => CallToolResult::error(vec![Content::text(format!("Serialization error: {e}"))]),
        })
    }

    fn error_result(&self, err: AppError) -> Result<CallToolResult, ErrorData> {
        error::app_error_result(err)
    }
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

#[tool_router]
impl McpServer {
    // -----------------------------------------------------------------------
    // Structure & Navigation
    // -----------------------------------------------------------------------

    #[tool(description = "Get the project file tree. Use this to orient yourself to the codebase layout before searching for symbols.")]
    fn get_structure(&self, Parameters(p): Parameters<GetStructureParams>) -> Result<CallToolResult, ErrorData> {
        let project = match self.get_project() {
            Ok(p) => p,
            Err(e) => return self.error_result(e),
        };
        let depth = p.depth.unwrap_or(0);
        let result = structure::get_structure(&project.file_tree, depth);
        self.record_history("TOOL", "get_structure", &format!("{} files", result.file_count));
        self.json_result(&result)
    }

    #[tool(description = "Attach a human-readable definition to a file. Fails if a definition already exists; use redefine_file to overwrite.")]
    fn define_file(&self, Parameters(p): Parameters<DefineFileParams>) -> Result<CallToolResult, ErrorData> {
        let project = match self.get_project() {
            Ok(p) => p,
            Err(e) => return self.error_result(e),
        };
        match structure::define_file(&project.file_tree, &p.file, &p.definition) {
            Ok(()) => {
                self.record_history("TOOL", "define_file", &p.file);
                self.json_result(&serde_json::json!({ "ok": true }))
            }
            Err(msg) => error::app_error_result(AppError::BadRequest(msg)),
        }
    }

    #[tool(description = "Replace a file's existing definition with a new one.")]
    fn redefine_file(&self, Parameters(p): Parameters<DefineFileParams>) -> Result<CallToolResult, ErrorData> {
        let project = match self.get_project() {
            Ok(p) => p,
            Err(e) => return self.error_result(e),
        };
        match structure::redefine_file(&project.file_tree, &p.file, &p.definition) {
            Ok(()) => {
                self.record_history("TOOL", "redefine_file", &p.file);
                self.json_result(&serde_json::json!({ "ok": true }))
            }
            Err(msg) => error::app_error_result(AppError::BadRequest(msg)),
        }
    }

    #[tool(description = "Tag a file with a semantic mark. Valid marks: documentation, ignore, test, config, generated, custom:<name>.")]
    fn mark_file(&self, Parameters(p): Parameters<MarkFileParams>) -> Result<CallToolResult, ErrorData> {
        let project = match self.get_project() {
            Ok(p) => p,
            Err(e) => return self.error_result(e),
        };
        match structure::mark_file(&project.file_tree, &p.file, &p.mark) {
            Ok(()) => {
                self.record_history("TOOL", "mark_file", &p.file);
                self.json_result(&serde_json::json!({ "ok": true }))
            }
            Err(msg) => error::app_error_result(AppError::BadRequest(msg)),
        }
    }

    #[tool(description = "List all currently indexed project roots with file and symbol counts.")]
    fn list_roots(&self) -> Result<CallToolResult, ErrorData> {
        let roots: Vec<serde_json::Value> = self
            .state
            .inner
            .projects
            .iter()
            .map(|entry| {
                let project = entry.value();
                let session_count = self
                    .state
                    .inner
                    .sessions
                    .iter()
                    .filter(|s| s.value().project_path == *entry.key())
                    .count();
                serde_json::json!({
                    "path": project.root.display().to_string(),
                    "file_count": project.file_tree.len(),
                    "symbol_count": project.symbol_table.len(),
                    "last_active": (*project.last_active.lock()).to_rfc3339(),
                    "session_count": session_count,
                })
            })
            .collect();
        self.json_result(&serde_json::json!({ "roots": roots, "count": roots.len() }))
    }

    // -----------------------------------------------------------------------
    // Symbols
    // -----------------------------------------------------------------------

    #[tool(description = "List symbols in the project, optionally filtered by kind, file, or limit. Use branch='base' to query the base branch of the active review.")]
    fn list_symbols(&self, Parameters(p): Parameters<ListSymbolsParams>) -> Result<CallToolResult, ErrorData> {
        let project = match self.resolve_project_for_branch(p.branch.as_deref()) {
            Ok(p) => p,
            Err(e) => return self.error_result(e),
        };
        let kind_filter = p.kind.as_deref().and_then(SymbolKind::from_str);
        let limit = p.limit.unwrap_or(100);
        let results =
            symbol_ops::list_symbols(&project.symbol_table, kind_filter, p.file.as_deref(), limit);
        self.record_history("TOOL", "list_symbols", &format!("{} symbols", results.len()));
        self.json_result(&serde_json::json!({ "symbols": results, "count": results.len() }))
    }

    #[tool(description = "Search for symbols by name substring (case-insensitive).")]
    fn search_symbols(
        &self,
        Parameters(p): Parameters<SearchSymbolsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let project = match self.get_project() {
            Ok(p) => p,
            Err(e) => return self.error_result(e),
        };
        let limit = p.limit.unwrap_or(20);
        let results = symbol_ops::search_symbols(&project.symbol_table, &p.query, limit);
        self.record_history(
            "TOOL",
            "search_symbols",
            &format!("{} matches for '{}'", results.len(), p.query),
        );
        self.json_result(&serde_json::json!({ "symbols": results, "count": results.len() }))
    }

    #[tool(description = "Get the full source code of a symbol. Use branch='base' to read from the base branch of the active review.")]
    fn get_implementation(
        &self,
        Parameters(p): Parameters<GetImplementationParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let project = match self.resolve_project_for_branch(p.branch.as_deref()) {
            Ok(p) => p,
            Err(e) => return self.error_result(e),
        };
        match symbol_ops::get_implementation(
            &project.root,
            &project.symbol_table,
            &p.symbol,
            &p.file,
        ) {
            Ok(source) => {
                self.record_history(
                    "TOOL",
                    "get_implementation",
                    &format!("{}::{}", p.file, p.symbol),
                );
                self.json_result(
                    &serde_json::json!({ "symbol": p.symbol, "file": p.file, "source": source }),
                )
            }
            Err(msg) => error::app_error_result(AppError::NotFound(msg)),
        }
    }

    #[tool(description = "Get source code for multiple symbols at once. Use branch='base' to read from the base branch of the active review.")]
    fn batch_implementations(
        &self,
        Parameters(p): Parameters<BatchImplementationsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let project = match self.resolve_project_for_branch(p.branch.as_deref()) {
            Ok(p) => p,
            Err(e) => return self.error_result(e),
        };
        let mut results: Vec<serde_json::Value> = Vec::new();
        let mut errors: Vec<serde_json::Value> = Vec::new();
        for item in &p.symbols {
            match symbol_ops::get_implementation(
                &project.root,
                &project.symbol_table,
                &item.symbol,
                &item.file,
            ) {
                Ok(source) => {
                    let line_range = project
                        .symbol_table
                        .get(&item.file, &item.symbol)
                        .map(|s| s.line_range);
                    results.push(serde_json::json!({
                        "symbol": item.symbol,
                        "file": item.file,
                        "source": source,
                        "line_range": line_range,
                    }));
                }
                Err(msg) => {
                    errors.push(
                        serde_json::json!({ "symbol": item.symbol, "file": item.file, "error": msg }),
                    );
                }
            }
        }
        self.record_history(
            "TOOL",
            "batch_implementations",
            &format!("{} symbols", p.symbols.len()),
        );
        self.json_result(&serde_json::json!({ "results": results, "errors": errors }))
    }

    #[tool(description = "Find test functions that reference a given symbol. Use branch='base' to search the base branch of the active review.")]
    fn find_tests(&self, Parameters(p): Parameters<FindTestsParams>) -> Result<CallToolResult, ErrorData> {
        let project = match self.resolve_project_for_branch(p.branch.as_deref()) {
            Ok(p) => p,
            Err(e) => return self.error_result(e),
        };
        let limit = p.limit.unwrap_or(20);
        match symbol_ops::find_tests(
            &project.root,
            &project.file_tree,
            &project.symbol_table,
            &p.symbol,
            &p.file,
            limit,
        ) {
            Ok(tests) => {
                self.record_history(
                    "TOOL",
                    "find_tests",
                    &format!("{} tests for {}", tests.len(), p.symbol),
                );
                self.json_result(&serde_json::json!({ "tests": tests, "count": tests.len() }))
            }
            Err(msg) => error::app_error_result(AppError::NotFound(msg)),
        }
    }

    #[tool(description = "Find call sites of a symbol across the codebase. Use branch='base' to search the base branch of the active review.")]
    fn find_callers(&self, Parameters(p): Parameters<FindCallersParams>) -> Result<CallToolResult, ErrorData> {
        let project = match self.resolve_project_for_branch(p.branch.as_deref()) {
            Ok(p) => p,
            Err(e) => return self.error_result(e),
        };
        let limit = p.limit.unwrap_or(50);
        match symbol_ops::find_callers(
            &project.root,
            &project.file_tree,
            &project.symbol_table,
            &p.symbol,
            &p.file,
            limit,
        ) {
            Ok(callers) => {
                self.record_history(
                    "TOOL",
                    "find_callers",
                    &format!("{} callers of {}", callers.len(), p.symbol),
                );
                self.json_result(&serde_json::json!({ "callers": callers, "count": callers.len() }))
            }
            Err(msg) => error::app_error_result(AppError::NotFound(msg)),
        }
    }

    #[tool(description = "List local variables within a function (AST-aware).")]
    fn list_variables(
        &self,
        Parameters(p): Parameters<ListVariablesParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let project = match self.get_project() {
            Ok(p) => p,
            Err(e) => return self.error_result(e),
        };
        match symbol_ops::list_variables(
            &project.root,
            &project.symbol_table,
            &p.function,
            &p.file,
        ) {
            Ok(vars) => {
                self.record_history(
                    "TOOL",
                    "list_variables",
                    &format!("{} variables in {}", vars.len(), p.function),
                );
                self.json_result(&serde_json::json!({ "variables": vars, "count": vars.len() }))
            }
            Err(msg) => error::app_error_result(AppError::NotFound(msg)),
        }
    }

    #[tool(description = "Attach a human-readable definition to a symbol. Fails if one already exists; use redefine_symbol to overwrite.")]
    fn define_symbol(&self, Parameters(p): Parameters<DefineSymbolParams>) -> Result<CallToolResult, ErrorData> {
        let project = match self.get_project() {
            Ok(p) => p,
            Err(e) => return self.error_result(e),
        };
        match symbol_ops::define_symbol(
            &project.symbol_table,
            &p.symbol,
            &p.file,
            &p.definition,
        ) {
            Ok(()) => {
                self.record_history("TOOL", "define_symbol", &p.symbol);
                self.json_result(&serde_json::json!({ "ok": true }))
            }
            Err(msg) => error::app_error_result(AppError::BadRequest(msg)),
        }
    }

    #[tool(description = "Replace a symbol's existing definition with a new one.")]
    fn redefine_symbol(
        &self,
        Parameters(p): Parameters<DefineSymbolParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let project = match self.get_project() {
            Ok(p) => p,
            Err(e) => return self.error_result(e),
        };
        match symbol_ops::redefine_symbol(
            &project.symbol_table,
            &p.symbol,
            &p.file,
            &p.definition,
        ) {
            Ok(()) => {
                self.record_history("TOOL", "redefine_symbol", &p.symbol);
                self.json_result(&serde_json::json!({ "ok": true }))
            }
            Err(msg) => error::app_error_result(AppError::BadRequest(msg)),
        }
    }

    // -----------------------------------------------------------------------
    // Content
    // -----------------------------------------------------------------------

    #[tool(description = "Read lines from a file with line numbers.")]
    fn peek(&self, Parameters(p): Parameters<PeekParams>) -> Result<CallToolResult, ErrorData> {
        let project = match self.get_project() {
            Ok(p) => p,
            Err(e) => return self.error_result(e),
        };
        let start = p.start.unwrap_or(0);
        let end = p.end.unwrap_or(100);
        match content::peek(&project.root, &project.file_tree, &p.file, start, end) {
            Ok(result) => {
                self.record_history("TOOL", "peek", &format!("{}:{}-{}", p.file, start, end));
                self.json_result(&result)
            }
            Err(msg) => error::app_error_result(AppError::NotFound(msg)),
        }
    }

    #[tool(description = "Search file contents by regex pattern. Use scope='code' to skip comments and string literals.")]
    fn grep(&self, Parameters(p): Parameters<GrepParams>) -> Result<CallToolResult, ErrorData> {
        let project = match self.get_project() {
            Ok(pr) => pr,
            Err(e) => return self.error_result(e),
        };
        let max_matches = p.max_matches.unwrap_or(50);
        let context_lines = p.context_lines.unwrap_or(2);
        let scope = p
            .scope
            .as_deref()
            .and_then(content::GrepScope::from_str)
            .unwrap_or(content::GrepScope::All);

        let root = project.root.clone();
        let file_tree = project.file_tree.clone();
        let pattern = p.pattern.clone();

        let result = tokio::task::block_in_place(|| {
            content::grep_with_scope(&root, &file_tree, &pattern, max_matches, context_lines, scope)
        });

        match result {
            Ok(r) => {
                self.record_history(
                    "TOOL",
                    "grep",
                    &format!("{} matches for '{}'", r.total_matches, p.pattern),
                );
                self.json_result(&r)
            }
            Err(msg) => error::app_error_result(AppError::BadRequest(msg)),
        }
    }

    #[tool(description = "Get byte-range chunk boundaries for a file. Useful for splitting large files into overlapping windows.")]
    fn chunk_indices(
        &self,
        Parameters(p): Parameters<ChunkIndicesParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let project = match self.get_project() {
            Ok(p) => p,
            Err(e) => return self.error_result(e),
        };
        let size = p.size.unwrap_or(5000);
        let overlap = p.overlap.unwrap_or(200);
        match content::chunk_indices(&project.root, &project.file_tree, &p.file, size, overlap) {
            Ok(result) => {
                self.record_history(
                    "TOOL",
                    "chunk_indices",
                    &format!("{} chunks for {}", result.chunks.len(), p.file),
                );
                self.json_result(&result)
            }
            Err(msg) => error::app_error_result(AppError::BadRequest(msg)),
        }
    }

    // -----------------------------------------------------------------------
    // History & Annotations
    // -----------------------------------------------------------------------

    #[tool(description = "Get the tool call history for this session.")]
    fn get_history(&self, Parameters(p): Parameters<GetHistoryParams>) -> Result<CallToolResult, ErrorData> {
        let limit = p.limit.unwrap_or(50);
        match history::get_history(&self.state, &self.session_id, limit) {
            Ok(entries) => {
                self.json_result(&serde_json::json!({ "history": entries, "count": entries.len() }))
            }
            Err(msg) => error::app_error_result(AppError::NotFound(msg)),
        }
    }

    #[tool(description = "Persist all file and symbol annotations to disk (.coderlm/annotations.json in the project root).")]
    fn save_annotations(&self) -> Result<CallToolResult, ErrorData> {
        let project = match self.get_project() {
            Ok(p) => p,
            Err(e) => return self.error_result(e),
        };
        match annotations::save_annotations(
            &project.root,
            &project.file_tree,
            &project.symbol_table,
        ) {
            Ok(()) => {
                self.record_history("TOOL", "save_annotations", "saved");
                self.json_result(&serde_json::json!({ "ok": true }))
            }
            Err(msg) => error::app_error_result(AppError::Internal(msg)),
        }
    }

    #[tool(description = "Load annotations from disk and apply them to the in-memory index.")]
    fn load_annotations(&self) -> Result<CallToolResult, ErrorData> {
        let project = match self.get_project() {
            Ok(p) => p,
            Err(e) => return self.error_result(e),
        };
        match annotations::load_annotations(
            &project.root,
            &project.file_tree,
            &project.symbol_table,
        ) {
            Ok(data) => {
                self.record_history("TOOL", "load_annotations", "loaded");
                self.json_result(&serde_json::json!({
                    "ok": true,
                    "loaded": {
                        "file_definitions": data.file_definitions.len(),
                        "file_marks": data.file_marks.len(),
                        "symbol_definitions": data.symbol_definitions.len(),
                    }
                }))
            }
            Err(msg) => error::app_error_result(AppError::Internal(msg)),
        }
    }

    // -----------------------------------------------------------------------
    // Review Lifecycle
    // -----------------------------------------------------------------------

    #[tool(description = "Create a review comparing two git refs. Blocks until indexing and diff computation are complete (up to 120 seconds). The new review becomes the active review for diff query tools.")]
    fn create_review(&self, Parameters(p): Parameters<CreateReviewParams>) -> Result<CallToolResult, ErrorData> {
        let head_ref = p.head_ref.unwrap_or_else(|| "HEAD".to_string());

        // Wrap the entire create + poll loop in block_in_place so that both
        // the blocking git I/O and std::thread::sleep don't stall the executor.
        tokio::task::block_in_place(|| {
            let review = match self.state.create_review(&self.project_root, &p.base_ref, &head_ref) {
                Ok(r) => r,
                Err(e) => return self.error_result(e),
            };

            let review_id = review.id.clone();

            // Poll until Ready or Error (up to 120 seconds, checking every 500ms).
            for _ in 0..240_u32 {
                std::thread::sleep(std::time::Duration::from_millis(500));

                let Some(r) = self.state.inner.reviews.get(&review_id) else {
                    return error::str_error_result("Review was removed during creation");
                };

                match &*r.status.read() {
                    ReviewStatus::Ready => {
                        // Set active review and attach to session.
                        *self.active_review_id.write() = Some(review_id.clone());
                        if let Some(mut s) = self.state.inner.sessions.get_mut(&self.session_id) {
                            s.review_id = Some(review_id.clone());
                        }

                        let stats = r.diff.read().as_ref().map(|d| {
                            serde_json::json!({
                                "files_added": d.stats.files_added,
                                "files_deleted": d.stats.files_deleted,
                                "files_modified": d.stats.files_modified,
                                "symbols_added": d.stats.symbols_added,
                                "symbols_deleted": d.stats.symbols_deleted,
                                "symbols_modified": d.stats.symbols_modified,
                                "symbols_moved": d.stats.symbols_moved,
                            })
                        });

                        self.record_history(
                            "TOOL",
                            "create_review",
                            &format!("{}..{}", r.base_ref, r.head_ref),
                        );
                        return self.json_result(&serde_json::json!({
                            "review_id": review_id,
                            "status": "ready",
                            "base_ref": r.base_ref,
                            "head_ref": r.head_ref,
                            "base_commit": &r.base_commit[..8],
                            "head_commit": &r.head_commit.read()[..8],
                            "stats": stats,
                        }));
                    }
                    ReviewStatus::Error(msg) => {
                        return error::str_error_result(
                            format!("Review indexing failed: {msg}"),
                        );
                    }
                    _ => {}
                }
            }

            error::str_error_result(
                "Review indexing timed out after 120 seconds. Call get_review to check status.",
            )
        })
    }

    #[tool(description = "List all active reviews with their status.")]
    fn list_reviews(&self) -> Result<CallToolResult, ErrorData> {
        let reviews: Vec<serde_json::Value> = self
            .state
            .inner
            .reviews
            .iter()
            .map(|entry| {
                let r = entry.value();
                let status = match &*r.status.read() {
                    ReviewStatus::Indexing => "indexing",
                    ReviewStatus::Computing => "computing",
                    ReviewStatus::Updating => "updating",
                    ReviewStatus::Ready => "ready",
                    ReviewStatus::Error(_) => "error",
                };
                serde_json::json!({
                    "review_id": r.id,
                    "base_ref": r.base_ref,
                    "head_ref": r.head_ref,
                    "status": status,
                    "created_at": r.created_at.to_rfc3339(),
                })
            })
            .collect();
        self.json_result(&serde_json::json!({ "reviews": reviews, "count": reviews.len() }))
    }

    #[tool(description = "Get the status and diff stats for a review.")]
    fn get_review_info(&self, Parameters(p): Parameters<ReviewIdParams>) -> Result<CallToolResult, ErrorData> {
        let review = match self.state.inner.reviews.get(&p.review_id) {
            Some(r) => r.clone(),
            None => {
                return error::app_error_result(AppError::NotFound(format!(
                    "Review '{}' not found",
                    p.review_id
                )))
            }
        };

        let status_str = match &*review.status.read() {
            ReviewStatus::Indexing => "indexing",
            ReviewStatus::Computing => "computing",
            ReviewStatus::Updating => "updating",
            ReviewStatus::Ready => "ready",
            ReviewStatus::Error(_) => "error",
        };

        let stats = review.diff.read().as_ref().map(|d| {
            serde_json::json!({
                "files_added": d.stats.files_added,
                "files_deleted": d.stats.files_deleted,
                "files_modified": d.stats.files_modified,
                "symbols_added": d.stats.symbols_added,
                "symbols_deleted": d.stats.symbols_deleted,
                "symbols_modified": d.stats.symbols_modified,
                "symbols_moved": d.stats.symbols_moved,
            })
        });

        let repo_root = review.repo_root.clone();
        let head_ref = review.head_ref.clone();
        let head_commit = review.head_commit.read().clone();
        let stale = tokio::task::block_in_place(|| {
            crate::git::resolve_ref(&repo_root, &head_ref)
                .map(|current| current != head_commit)
                .unwrap_or(false)
        });

        self.json_result(&serde_json::json!({
            "review_id": review.id,
            "base_ref": review.base_ref,
            "head_ref": review.head_ref,
            "base_commit": &review.base_commit[..8],
            "head_commit": &review.head_commit.read()[..8],
            "status": status_str,
            "created_at": review.created_at.to_rfc3339(),
            "stats": stats,
            "stale": stale,
        }))
    }

    #[tool(description = "Delete a review and clean up its git worktrees and cached data.")]
    fn delete_review(&self, Parameters(p): Parameters<ReviewIdParams>) -> Result<CallToolResult, ErrorData> {
        // Clear active review if it's the one being deleted.
        {
            let mut active = self.active_review_id.write();
            if active.as_deref() == Some(&p.review_id) {
                *active = None;
                if let Some(mut s) = self.state.inner.sessions.get_mut(&self.session_id) {
                    s.review_id = None;
                }
            }
        }
        match self.state.delete_review(&p.review_id) {
            Ok(()) => self.json_result(&serde_json::json!({ "deleted": true })),
            Err(e) => self.error_result(e),
        }
    }

    #[tool(description = "Set which review is active for diff query tools (review_summary, review_files, etc.). The review must exist and be in 'ready' status.")]
    fn set_active_review(
        &self,
        Parameters(p): Parameters<ReviewIdParams>,
    ) -> Result<CallToolResult, ErrorData> {
        if !self.state.inner.reviews.contains_key(&p.review_id) {
            return error::app_error_result(AppError::NotFound(format!(
                "Review '{}' not found",
                p.review_id
            )));
        }
        *self.active_review_id.write() = Some(p.review_id.clone());
        if let Some(mut s) = self.state.inner.sessions.get_mut(&self.session_id) {
            s.review_id = Some(p.review_id.clone());
        }
        self.json_result(&serde_json::json!({ "ok": true, "active_review_id": p.review_id }))
    }

    // -----------------------------------------------------------------------
    // Review Diff Queries
    // -----------------------------------------------------------------------

    #[tool(description = "Get a statistical summary of the active review diff (file and symbol counts by change type). Requires an active review.")]
    fn review_summary(&self) -> Result<CallToolResult, ErrorData> {
        let review = match self.get_review() {
            Ok(r) => r,
            Err(e) => return self.error_result(e),
        };
        let diff = review.diff.read();
        let diff = diff.as_ref().expect("status=Ready implies diff is Some");
        self.json_result(&serde_json::json!({
            "base_ref": review.base_ref,
            "head_ref": review.head_ref,
            "base_commit": &review.base_commit[..8],
            "head_commit": &review.head_commit.read()[..8],
            "files_added": diff.stats.files_added,
            "files_deleted": diff.stats.files_deleted,
            "files_modified": diff.stats.files_modified,
            "files_renamed": diff.stats.files_renamed,
            "symbols_added": diff.stats.symbols_added,
            "symbols_deleted": diff.stats.symbols_deleted,
            "symbols_modified": diff.stats.symbols_modified,
            "symbols_moved": diff.stats.symbols_moved,
        }))
    }

    #[tool(description = "List all files changed in the active review with their change status and language. Requires an active review.")]
    fn review_files(&self) -> Result<CallToolResult, ErrorData> {
        let review = match self.get_review() {
            Ok(r) => r,
            Err(e) => return self.error_result(e),
        };
        let diff = review.diff.read();
        let diff = diff.as_ref().expect("status=Ready implies diff is Some");
        self.json_result(
            &serde_json::json!({ "files": diff.file_diffs, "count": diff.file_diffs.len() }),
        )
    }

    #[tool(description = "List all changed symbols in the active review, optionally filtered by change type, file, or kind. Requires an active review.")]
    fn review_symbols(
        &self,
        Parameters(p): Parameters<ReviewSymbolsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let review = match self.get_review() {
            Ok(r) => r,
            Err(e) => return self.error_result(e),
        };
        let diff = review.diff.read();
        let diff = diff.as_ref().expect("status=Ready implies diff is Some");

        let kind_filter = p.kind.as_deref().and_then(SymbolKind::from_str);

        let symbols: Vec<&review::SymbolDiffEntry> = diff
            .symbol_diffs
            .iter()
            .filter(|s| {
                if let Some(change) = p.change.as_deref() {
                    let matches = match change {
                        "added" => matches!(s.change, review::SymbolChange::Added),
                        "deleted" => matches!(s.change, review::SymbolChange::Deleted),
                        "modified" => matches!(s.change, review::SymbolChange::Modified { .. }),
                        _ => true,
                    };
                    if !matches {
                        return false;
                    }
                }
                if let Some(file) = p.file.as_deref() {
                    if s.file != file {
                        return false;
                    }
                }
                if let Some(kind) = kind_filter {
                    if s.kind != kind {
                        return false;
                    }
                }
                true
            })
            .collect();

        self.json_result(&serde_json::json!({ "symbols": symbols, "count": symbols.len() }))
    }

    #[tool(description = "Get the unified diff text for a single changed file. Requires an active review.")]
    fn review_file_diff(
        &self,
        Parameters(p): Parameters<ReviewFileDiffParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let review = match self.get_review() {
            Ok(r) => r,
            Err(e) => return self.error_result(e),
        };

        if let Some(cached) = review.cache.file_diffs.get(&p.file) {
            return self
                .json_result(&serde_json::json!({ "file": p.file, "diff": cached.clone() }));
        }

        let repo_root = review.repo_root.clone();
        let base_ref = review.base_ref.clone();
        let head_ref = review.head_ref.clone();
        let file = p.file.clone();

        let diff_text = match tokio::task::block_in_place(|| {
            crate::git::get_file_diff(&repo_root, &base_ref, &head_ref, &file)
        }) {
            Ok(d) => d,
            Err(e) => return error::app_error_result(AppError::BadRequest(e.to_string())),
        };

        review.cache.file_diffs.insert(p.file.clone(), diff_text.clone());
        self.json_result(&serde_json::json!({ "file": p.file, "diff": diff_text }))
    }

    #[tool(description = "Get a rich context bundle for a changed file: patch, hunk ranges, affected symbols, and source excerpts. Requires an active review.")]
    fn review_file_context(
        &self,
        Parameters(p): Parameters<ReviewFileContextParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let review = match self.get_review() {
            Ok(r) => r,
            Err(e) => return self.error_result(e),
        };
        let context_lines = p.context_lines.unwrap_or(50);
        let max_full_file_lines = p.max_full_file_lines.unwrap_or(500);
        let file = p.file.clone();

        let result = tokio::task::block_in_place(|| {
            review::context::compute_file_context(&review, &file, context_lines, max_full_file_lines)
        });

        match result {
            Ok(r) => self.json_result(r.as_ref()),
            Err(e) => error::app_error_result(AppError::BadRequest(e.to_string())),
        }
    }

    #[tool(description = "Get before/after source code and a unified diff for a single changed symbol. Requires an active review.")]
    fn review_symbol_diff(
        &self,
        Parameters(p): Parameters<ReviewSymbolDiffParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let review = match self.get_review() {
            Ok(r) => r,
            Err(e) => return self.error_result(e),
        };
        let symbol = p.symbol.clone();
        let file = p.file.clone();

        let result = tokio::task::block_in_place(|| {
            review::context::compute_symbol_diff(&review, &symbol, &file)
        });

        match result {
            Ok(r) => self.json_result(&r),
            Err(e) => error::app_error_result(AppError::NotFound(e.to_string())),
        }
    }

    #[tool(description = "Find symbols whose signature changed or were deleted, but still have callers in unmodified files. High-risk breakage indicator. Requires an active review.")]
    fn review_safety(
        &self,
        Parameters(p): Parameters<ReviewSafetyParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let review = match self.get_review() {
            Ok(r) => r,
            Err(e) => return self.error_result(e),
        };
        let depth = p.depth.unwrap_or(1).clamp(1, 3);

        if depth == 1 {
            if let Some(cached) = review.cache.safety_report.read().clone() {
                return self.json_result(cached.as_ref());
            }
        }

        let base_project = review.base_project.clone();
        let diff = review
            .diff
            .read()
            .clone()
            .expect("status=Ready implies diff is Some");

        let report = tokio::task::block_in_place(|| {
            review::impact::compute_safety(&base_project, &diff, depth)
        });

        let report = std::sync::Arc::new(report);
        if depth == 1 {
            *review.cache.safety_report.write() = Some(report.clone());
        }
        self.json_result(report.as_ref())
    }

    #[tool(description = "Find symbols with body-only changes (signature unchanged) that still have callers in unmodified files. Silent behavioral change indicator. Requires an active review.")]
    fn review_body_safety(
        &self,
        Parameters(p): Parameters<ReviewSafetyParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let review = match self.get_review() {
            Ok(r) => r,
            Err(e) => return self.error_result(e),
        };
        let depth = p.depth.unwrap_or(1).clamp(1, 3);

        let base_project = review.base_project.clone();
        let diff = review
            .diff
            .read()
            .clone()
            .expect("status=Ready implies diff is Some");

        let report = tokio::task::block_in_place(|| {
            review::impact::compute_body_safety(&base_project, &diff, depth)
        });
        self.json_result(&report)
    }

    #[tool(description = "Analyze the blast radius of a specific changed symbol: all callers in the base branch annotated with whether they were also updated in this PR. Requires an active review.")]
    fn review_impact(
        &self,
        Parameters(p): Parameters<ReviewImpactParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let review = match self.get_review() {
            Ok(r) => r,
            Err(e) => return self.error_result(e),
        };
        let depth = p.depth.unwrap_or(1).clamp(1, 3);
        let cache_key = review::ReviewCache::impact_key(&p.file, &p.symbol);

        if depth == 1 {
            if let Some(cached) = review.cache.impact_results.get(&cache_key) {
                return self.json_result(cached.as_ref());
            }
        }

        let base_project = review.base_project.clone();
        let head_project = review.head_project.clone();
        let diff = review
            .diff
            .read()
            .clone()
            .expect("status=Ready implies diff is Some");
        let symbol = p.symbol.clone();
        let file = p.file.clone();

        let result = tokio::task::block_in_place(|| {
            review::impact::compute_impact(&base_project, &head_project, &diff, &symbol, &file, depth)
        });

        match result {
            Some(impact) => {
                let impact = std::sync::Arc::new(impact);
                if depth == 1 {
                    review.cache.impact_results.insert(cache_key, impact.clone());
                }
                self.json_result(impact.as_ref())
            }
            None => error::app_error_result(AppError::NotFound(format!(
                "Symbol '{}' in '{}' not found in the review diff",
                p.symbol, p.file
            ))),
        }
    }

    #[tool(description = "Check which added or modified symbols have test coverage and which do not. Requires an active review.")]
    fn review_test_coverage(&self) -> Result<CallToolResult, ErrorData> {
        let review = match self.get_review() {
            Ok(r) => r,
            Err(e) => return self.error_result(e),
        };

        if let Some(cached) = review.cache.test_coverage.read().clone() {
            return self.json_result(cached.as_ref());
        }

        let head_project = review.head_project.clone();
        let diff = review
            .diff
            .read()
            .clone()
            .expect("status=Ready implies diff is Some");

        let report = tokio::task::block_in_place(|| {
            review::impact::compute_test_coverage(&head_project, &diff)
        });

        let report = std::sync::Arc::new(report);
        *review.cache.test_coverage.write() = Some(report.clone());
        self.json_result(report.as_ref())
    }

    #[tool(description = "Find callers of a changed symbol that were NOT updated in this PR (stale references). Handles moved and renamed symbols. Requires an active review.")]
    fn review_reference_check(
        &self,
        Parameters(p): Parameters<ReviewReferenceCheckParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let review = match self.get_review() {
            Ok(r) => r,
            Err(e) => return self.error_result(e),
        };
        let symbol = p.symbol.clone();
        let file = p.file.clone();

        let result = tokio::task::block_in_place(|| {
            review::references::compute_reference_check(&review, &symbol, &file)
        });

        match result {
            Ok(r) => self.json_result(&r),
            Err(e) => error::app_error_result(AppError::NotFound(e.to_string())),
        }
    }

    #[tool(description = "Classify each changed symbol by semantic category: structural, api_surface_change, behavioral, type_change, error_handling_change, import_change. Requires an active review.")]
    fn review_change_classification(&self) -> Result<CallToolResult, ErrorData> {
        let review = match self.get_review() {
            Ok(r) => r,
            Err(e) => return self.error_result(e),
        };

        let classifications = tokio::task::block_in_place(|| {
            review::classification::compute_change_classification(&review)
        });

        self.json_result(&serde_json::json!({
            "classifications": classifications,
            "count": classifications.len(),
        }))
    }

    #[tool(description = "Show import additions and removals for changed files. Requires an active review.")]
    fn review_import_diff(
        &self,
        Parameters(p): Parameters<ReviewImportDiffParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let review = match self.get_review() {
            Ok(r) => r,
            Err(e) => return self.error_result(e),
        };

        if let Some(file) = p.file {
            let result = tokio::task::block_in_place(|| {
                review::imports::compute_import_diff_for_file(&review, &file)
            });

            match result {
                Some(r) => self.json_result(&r),
                None => error::app_error_result(AppError::NotFound(
                    "File not found in review diff".into(),
                )),
            }
        } else {
            let results = tokio::task::block_in_place(|| {
                review::imports::compute_import_diff_all(&review)
            });

            self.json_result(&serde_json::json!({ "files": results, "count": results.len() }))
        }
    }

    #[tool(description = "Compute complexity deltas (lines, nesting depth, parameter count) for modified symbols. Flags regressions. Requires an active review.")]
    fn review_complexity(
        &self,
        Parameters(p): Parameters<ReviewComplexityParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let review = match self.get_review() {
            Ok(r) => r,
            Err(e) => return self.error_result(e),
        };
        let file = p.file.clone();

        let deltas = tokio::task::block_in_place(|| {
            review::complexity::compute_complexity(&review, file.as_deref())
        });

        let regression_count = deltas.iter().filter(|d| d.regression).count();
        self.json_result(&serde_json::json!({
            "deltas": deltas,
            "count": deltas.len(),
            "regression_count": regression_count,
        }))
    }
}

// ---------------------------------------------------------------------------
// ServerHandler implementation
// ---------------------------------------------------------------------------

#[tool_handler]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "coderlm".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                ..Default::default()
            },
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some(
                "CodeRLM: tree-sitter-backed structural code exploration and PR diff review. \
                Use get_structure to orient yourself, search_symbols to find code, \
                get_implementation to read it, find_callers/find_tests to trace dependencies. \
                For PR review: create_review first, then use review_* tools to query the diff."
                    .into(),
            ),
            ..Default::default()
        }
    }
}
