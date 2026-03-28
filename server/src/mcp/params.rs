use schemars::JsonSchema;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Structure & Navigation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetStructureParams {
    #[schemars(description = "Maximum depth of the file tree (0 = unlimited)")]
    pub depth: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DefineFileParams {
    #[schemars(description = "Relative path of the file to annotate")]
    pub file: String,
    #[schemars(description = "Human-readable description of the file's purpose")]
    pub definition: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MarkFileParams {
    #[schemars(description = "Relative path of the file to mark")]
    pub file: String,
    #[schemars(description = "Semantic tag: documentation, ignore, test, config, generated, or custom:<name>")]
    pub mark: String,
}

// ---------------------------------------------------------------------------
// Symbols
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListSymbolsParams {
    #[schemars(description = "Filter by symbol kind: function, method, class, struct, enum, trait, interface, constant, variable, type, module, import, other")]
    pub kind: Option<String>,
    #[schemars(description = "Filter to symbols defined in this file (relative path)")]
    pub file: Option<String>,
    #[schemars(description = "Maximum number of results (default 100)")]
    pub limit: Option<usize>,
    #[schemars(description = "Use 'base' to query the base branch of the active review instead of head")]
    pub branch: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchSymbolsParams {
    #[schemars(description = "Case-insensitive substring to search for in symbol names")]
    pub query: String,
    #[schemars(description = "Maximum number of results (default 20)")]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetImplementationParams {
    #[schemars(description = "Symbol name")]
    pub symbol: String,
    #[schemars(description = "File containing the symbol (relative path)")]
    pub file: String,
    #[schemars(description = "Use 'base' to get the implementation from the base branch of the active review")]
    pub branch: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SymbolRef {
    #[schemars(description = "Symbol name")]
    pub symbol: String,
    #[schemars(description = "File containing the symbol (relative path)")]
    pub file: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BatchImplementationsParams {
    #[schemars(description = "List of symbol+file pairs to fetch implementations for")]
    pub symbols: Vec<SymbolRef>,
    #[schemars(description = "Use 'base' to query the base branch of the active review")]
    pub branch: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindTestsParams {
    #[schemars(description = "Symbol name to find tests for")]
    pub symbol: String,
    #[schemars(description = "File containing the symbol (relative path)")]
    pub file: String,
    #[schemars(description = "Maximum number of results (default 20)")]
    pub limit: Option<usize>,
    #[schemars(description = "Use 'base' to query the base branch of the active review")]
    pub branch: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindCallersParams {
    #[schemars(description = "Symbol name to find call sites for")]
    pub symbol: String,
    #[schemars(description = "File containing the symbol (relative path)")]
    pub file: String,
    #[schemars(description = "Maximum number of results (default 50)")]
    pub limit: Option<usize>,
    #[schemars(description = "Use 'base' to query the base branch of the active review")]
    pub branch: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListVariablesParams {
    #[schemars(description = "Function name to list local variables for")]
    pub function: String,
    #[schemars(description = "File containing the function (relative path)")]
    pub file: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DefineSymbolParams {
    #[schemars(description = "Symbol name")]
    pub symbol: String,
    #[schemars(description = "File containing the symbol (relative path)")]
    pub file: String,
    #[schemars(description = "Human-readable description of the symbol's purpose")]
    pub definition: String,
}

// ---------------------------------------------------------------------------
// Content
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PeekParams {
    #[schemars(description = "File to read (relative path)")]
    pub file: String,
    #[schemars(description = "First line to read, 0-indexed (default 0)")]
    pub start: Option<usize>,
    #[schemars(description = "Last line to read, exclusive (default 100)")]
    pub end: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GrepParams {
    #[schemars(description = "Regex pattern to search for")]
    pub pattern: String,
    #[schemars(description = "Maximum number of matches to return (default 50)")]
    pub max_matches: Option<usize>,
    #[schemars(description = "Lines of context around each match (default 2)")]
    pub context_lines: Option<usize>,
    #[schemars(description = "Search scope: 'all' (default) or 'code' to skip comments and string literals")]
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ChunkIndicesParams {
    #[schemars(description = "File to chunk (relative path)")]
    pub file: String,
    #[schemars(description = "Target chunk size in bytes (default 5000)")]
    pub size: Option<usize>,
    #[schemars(description = "Overlap between consecutive chunks in bytes (default 200)")]
    pub overlap: Option<usize>,
}

// ---------------------------------------------------------------------------
// History & Annotations
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetHistoryParams {
    #[schemars(description = "Maximum number of recent entries to return (default 50)")]
    pub limit: Option<usize>,
}

// ---------------------------------------------------------------------------
// Review Lifecycle
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateReviewParams {
    #[schemars(description = "Git ref for the base branch (e.g. 'main', 'HEAD~1', a commit SHA)")]
    pub base_ref: String,
    #[schemars(description = "Git ref for the head branch (default 'HEAD')")]
    pub head_ref: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReviewIdParams {
    #[schemars(description = "Review ID returned by create_review or list_reviews")]
    pub review_id: String,
}

// ---------------------------------------------------------------------------
// Review Diff Queries
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReviewSymbolsParams {
    #[schemars(description = "Filter by change type: added, deleted, modified")]
    pub change: Option<String>,
    #[schemars(description = "Filter to symbols in this file (relative path)")]
    pub file: Option<String>,
    #[schemars(description = "Filter by symbol kind: function, method, class, struct, etc.")]
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReviewFileDiffParams {
    #[schemars(description = "File to get the unified diff for (relative path)")]
    pub file: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReviewFileContextParams {
    #[schemars(description = "File to get context for (relative path)")]
    pub file: String,
    #[schemars(description = "Lines of context around changed hunks (default 50)")]
    pub context_lines: Option<usize>,
    #[schemars(description = "Maximum lines before switching to windowed excerpts (default 500)")]
    pub max_full_file_lines: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReviewSymbolDiffParams {
    #[schemars(description = "Symbol name")]
    pub symbol: String,
    #[schemars(description = "File containing the symbol (relative path)")]
    pub file: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReviewSafetyParams {
    #[schemars(description = "Caller chain depth to traverse (1-3, default 1)")]
    pub depth: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReviewImpactParams {
    #[schemars(description = "Symbol name to analyze impact for")]
    pub symbol: String,
    #[schemars(description = "File containing the symbol (relative path)")]
    pub file: String,
    #[schemars(description = "Caller chain depth to traverse (1-3, default 1)")]
    pub depth: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReviewReferenceCheckParams {
    #[schemars(description = "Symbol name to check for stale references")]
    pub symbol: String,
    #[schemars(description = "File containing the symbol (relative path)")]
    pub file: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReviewImportDiffParams {
    #[schemars(description = "Limit to a specific file (relative path). Omit for all changed files.")]
    pub file: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReviewComplexityParams {
    #[schemars(description = "Limit to a specific file (relative path). Omit for all changed files.")]
    pub file: Option<String>,
}
