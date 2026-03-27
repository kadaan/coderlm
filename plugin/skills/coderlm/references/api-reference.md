# CoderLM Server API Reference

All endpoints prefixed with `/api/v1`. Session-scoped endpoints require `X-Session-Id` header.
The CLI wrapper (`coderlm_cli.py`) handles headers and session management automatically.

## CLI Command Reference

All commands below assume the CLI is at `skills/coderlm/scripts/coderlm_cli.py`.
Abbreviated as `cli` below.

### Session Management

```bash
# Create session (indexes project, caches session ID)
python3 cli init [--cwd /path/to/project] [--port 3000]

# Server + session status
python3 cli status

# Delete session
python3 cli cleanup
```

### Codebase Structure

```bash
# File tree (depth 0 = unlimited)
python3 cli structure [--depth 2]

# Annotate a file
python3 cli define-file src/main.rs "CLI entrypoint, parses args and starts server"
python3 cli redefine-file src/main.rs "Updated description"

# Tag file type: documentation, ignore, test, config, generated, custom
python3 cli mark tests/integration.rs test
```

### Symbol Operations

```bash
# List symbols (filter by kind, file, or both)
python3 cli symbols [--kind function] [--file src/main.rs] [--limit 50]

# Search symbols by name substring
python3 cli search "handler" [--limit 20]

# Get full source code of a symbol
python3 cli impl run_server --file src/main.rs

# Find call sites
python3 cli callers scan_directory --file src/index/walker.rs [--limit 50]

# Find tests referencing a symbol
python3 cli tests scan_directory --file src/index/walker.rs [--limit 20]

# List local variables in a function
python3 cli variables scan_directory --file src/index/walker.rs

# Annotate a symbol
python3 cli define-symbol scan_directory --file src/index/walker.rs "Walks codebase respecting gitignore"
python3 cli redefine-symbol scan_directory --file src/index/walker.rs "Updated description"
```

### Content Operations

```bash
# Read lines from a file (0-indexed, end exclusive)
python3 cli peek src/main.rs [--start 0] [--end 50]

# Regex search across all indexed files
python3 cli grep "DashMap" [--max-matches 50] [--context-lines 2]

# Scope-aware grep: only match in code (skip comments and strings)
python3 cli grep "DashMap" --scope code

# Compute byte-range chunks for a file
python3 cli chunks src/main.rs [--size 5000] [--overlap 200]
```

### Annotations

```bash
# Save annotations (definitions + marks) to .coderlm/annotations.json
python3 cli save-annotations

# Load annotations from disk (auto-loaded on session creation)
python3 cli load-annotations
```

### History

```bash
# Session command history
python3 cli history [--limit 50]
```

### Base Branch Queries (with review attached)

```bash
# Query base branch snapshot for comparison (requires attached review)
python3 cli impl run_server --file src/main.rs --branch base
python3 cli callers scan_directory --file src/index/walker.rs --branch base
python3 cli symbols --file src/routes.rs --branch base
python3 cli tests scan_directory --file src/index/walker.rs --branch base
```

## Response Shapes

### structure
```json
{
  "tree": "├── src/\n│   ├── main.rs\n...",
  "file_count": 42,
  "language_breakdown": [{"language": "rust", "count": 38}]
}
```

### symbols
```json
{
  "count": 3,
  "symbols": [
    {
      "name": "run_server",
      "kind": "function",
      "file": "src/main.rs",
      "line_range": [69, 143],
      "signature": "async fn run_server(",
      "definition": null,
      "parent": null
    }
  ]
}
```

### search
Same shape as symbols response.

### impl
```json
{
  "symbol": "scan_directory",
  "file": "src/index/walker.rs",
  "source": "pub fn scan_directory(root: &Path) -> Result<usize> {\n    ...\n}"
}
```

### callers
```json
{
  "count": 2,
  "callers": [
    {"file": "src/main.rs", "line": 95, "text": "walker::scan_directory("}
  ]
}
```

### tests
```json
{
  "count": 1,
  "tests": [
    {"name": "test_scan_directory", "file": "tests/walker_test.rs", "line": 12, "signature": "fn test_scan_directory() {"}
  ]
}
```

### variables
```json
{
  "count": 3,
  "variables": [
    {"name": "walker", "function": "scan_directory"},
    {"name": "count", "function": "scan_directory"}
  ]
}
```

### peek
```json
{
  "file": "src/main.rs",
  "start_line": 1,
  "end_line": 10,
  "total_lines": 143,
  "content": "     1 │ mod config;\n     2 │ mod index;\n..."
}
```

### grep
```json
{
  "pattern": "DashMap",
  "total_matches": 8,
  "truncated": false,
  "matches": [
    {
      "file": "src/index/file_tree.rs",
      "line": 1,
      "text": "use dashmap::DashMap;",
      "context_before": [],
      "context_after": ["use serde::Serialize;"]
    }
  ]
}
```

### chunks
```json
{
  "file": "src/main.rs",
  "total_bytes": 3521,
  "chunk_size": 5000,
  "overlap": 200,
  "chunks": [{"index": 0, "start": 0, "end": 3521}]
}
```

### health
```json
{
  "status": "ok",
  "projects": 2,
  "active_sessions": 3,
  "max_projects": 5
}
```

## Symbol Kinds

`function`, `method`, `class`, `struct`, `enum`, `trait`, `interface`, `constant`, `variable`, `type`, `module`

## Supported Languages (tree-sitter)

| Language   | Extensions                    | Support      |
|------------|-------------------------------|--------------|
| Rust       | `.rs`                         | tree-sitter  |
| Python     | `.py`, `.pyi`                 | tree-sitter  |
| TypeScript | `.ts`, `.tsx`                 | tree-sitter  |
| JavaScript | `.js`, `.jsx`, `.mjs`, `.cjs` | tree-sitter  |
| Go         | `.go`                         | tree-sitter  |
| Java       | `.java`                       | tree-sitter  |
| Scala      | `.scala`, `.sc`               | tree-sitter  |
| SQL        | `.sql`                        | regex        |

Languages with tree-sitter support produce full symbol tables (functions, classes, methods, callers, variables). SQL uses regex fallbacks for variable and definition detection. All other file types appear in the file tree and are searchable via peek/grep, but do not produce symbols.

## Mark Types

`documentation`, `ignore`, `test`, `config`, `generated`, `custom`

## Code Review API

Review endpoints compare two indexed snapshots of a repo (base branch vs PR branch). A review must be created and attached to a session before using the diff/safety/impact endpoints.

### Review Lifecycle

```bash
# Create a review (indexes base branch in a git worktree, computes semantic diff)
python3 cli review-init --base main [--head HEAD]
# → polls until status=ready

# Show status of attached review
python3 cli review-status

# Delete review and clean up worktree
python3 cli review-cleanup
```

### Diff Queries (require attached review)

```bash
# High-level stats
python3 cli review-summary

# Changed files
python3 cli review-files

# Changed symbols (filter by change type, file, or kind)
python3 cli review-symbols [--change added|deleted|modified] [--file path] [--kind function]

# Unified diff for a specific file
python3 cli review-file-diff --file src/server/routes.rs
```

### Cross-Reference Queries

```bash
# Find deleted/signature-changed symbols that have callers in unmodified files
python3 cli review-safety

# Per-symbol blast radius: all callers in base branch, annotated with PR change status
python3 cli review-impact process_request --file src/server/routes.rs

# Test coverage of added/modified symbols
python3 cli review-test-coverage
```

### Base Branch Queries (existing commands)

Add `--branch base` to query the base snapshot when a review is attached:

```bash
python3 cli impl process_request --file src/routes.rs --branch base  # old version
python3 cli callers process_request --file src/routes.rs --branch base  # callers in base
python3 cli symbols --file src/routes.rs --branch base  # symbols in base version
python3 cli tests process_request --file src/routes.rs --branch base  # tests in base
```

### Response Shapes

#### review-summary
```json
{
  "base_ref": "main",
  "head_ref": "feature-branch",
  "base_commit": "abc12345",
  "head_commit": "def67890",
  "status": "ready",
  "files_added": 3,
  "files_deleted": 1,
  "files_modified": 7,
  "files_renamed": 0,
  "symbols_added": 12,
  "symbols_deleted": 4,
  "symbols_modified": 8
}
```

#### review-files
```json
{
  "count": 11,
  "files": [
    {"path": "src/new_module.rs", "status": "added", "language": "rust"},
    {"path": "src/old.rs", "status": "deleted", "language": "rust"},
    {"path": "src/server/routes.rs", "status": "modified", "language": "rust"},
    {"path": "src/server/routes.rs", "status": "renamed", "old_path": "src/old_routes.rs", "language": "rust"}
  ]
}
```

#### review-symbols
```json
{
  "count": 3,
  "symbols": [
    {"name": "handle_review", "file": "src/routes.rs", "kind": "function", "change": "added"},
    {
      "name": "process_request",
      "file": "src/routes.rs",
      "kind": "function",
      "change": "modified",
      "signature_changed": true,
      "old_signature": "fn process_request(req: &Request) -> Response",
      "new_signature": "fn process_request(req: &Request, ctx: &Context) -> Response",
      "body_changed": true
    },
    {"name": "old_handler", "file": "src/routes.rs", "kind": "function", "change": "deleted"}
  ]
}
```

#### review-safety
```json
{
  "safe_changes_count": 2,
  "issues": [
    {
      "symbol": "old_handler",
      "file": "src/routes.rs",
      "change": "deleted",
      "unmodified_callers": [
        {"file": "src/main.rs", "line": 88, "text": "old_handler()", "also_modified_in_pr": false}
      ],
      "risk": "high"
    }
  ]
}
```

#### review-impact
```json
{
  "symbol": "process_request",
  "file": "src/routes.rs",
  "change": "modified",
  "signature_changed": true,
  "base_callers": [
    {"file": "src/main.rs", "line": 42, "text": "process_request(&req)", "also_modified_in_pr": false},
    {"file": "src/handlers.rs", "line": 18, "text": "process_request(&r)", "also_modified_in_pr": true}
  ],
  "unmodified_callers_count": 1,
  "tests": [
    {"name": "test_process_request", "file": "tests/test.rs", "line": 10, "also_modified_in_pr": false}
  ],
  "risk": "high"
}
```

#### review-test-coverage
```json
{
  "coverage_ratio": "3/5",
  "covered": [
    {"symbol": "new_handler", "file": "src/routes.rs", "kind": "function", "tests": ["test_new_handler"]}
  ],
  "uncovered": [
    {"symbol": "another_fn", "file": "src/routes.rs", "kind": "function", "change": "added"}
  ]
}
```

## Mark Types

`documentation`, `ignore`, `test`, `config`, `generated`, `custom`

## Error Codes

| Status | Meaning |
|--------|---------|
| 400    | Bad request (missing/invalid parameters) |
| 404    | Resource not found |
| 410    | Project evicted — create a new session |
| 500    | Server error |
