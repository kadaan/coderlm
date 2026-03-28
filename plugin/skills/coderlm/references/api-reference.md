# CoderLM Server API Reference

All endpoints prefixed with `/api/v1`. Session-scoped endpoints require `X-Session-Id` header.
The CLI wrapper (`coderlm_cli.py`) handles headers and session management automatically.

## CLI Command Reference

All commands below assume the CLI is invoked as:
```bash
${CLAUDE_SKILL_DIR}/scripts/coderlm <command> [args]
```
Abbreviated as `{cli}` in the examples below.

Sessions are created automatically on first use — `init` is optional.

### Session Management

```bash
# Create session explicitly (optional — any command will auto-create on first use)
{cli} init [--cwd /path/to/project] [--port 3000]

# Server + session status
{cli} status

# Delete session
{cli} cleanup
```

### Codebase Structure

```bash
# File tree (depth 0 = unlimited)
{cli} structure [--depth 2]

# Annotate a file
{cli} define-file src/main.rs "CLI entrypoint, parses args and starts server"
{cli} redefine-file src/main.rs "Updated description"

# Tag file type: documentation, ignore, test, config, generated, custom
{cli} mark tests/integration.rs test
```

### Symbol Operations

```bash
# List symbols (filter by kind, file, or both)
{cli} symbols [--kind function] [--file src/main.rs] [--limit 50]

# Search symbols by name substring
{cli} search "handler" [--limit 20]

# Get full source code of a symbol
{cli} impl run_server --file src/main.rs

# Find call sites
{cli} callers scan_directory --file src/index/walker.rs [--limit 50]

# Find tests referencing a symbol
{cli} tests scan_directory --file src/index/walker.rs [--limit 20]

# List local variables in a function
{cli} variables scan_directory --file src/index/walker.rs

# Get implementations for multiple symbols in one call
{cli} batch-impl --symbols '[{"symbol":"fn_a","file":"src/a.rs"},{"symbol":"fn_b","file":"src/b.rs"}]' [--branch base]

# Annotate a symbol
{cli} define-symbol scan_directory --file src/index/walker.rs "Walks codebase respecting gitignore"
{cli} redefine-symbol scan_directory --file src/index/walker.rs "Updated description"
```

### Content Operations

```bash
# Read lines from a file (0-indexed, end exclusive)
{cli} peek src/main.rs [--start 0] [--end 50]

# Regex search across all indexed files
{cli} grep "DashMap" [--max-matches 50] [--context-lines 2]

# Scope-aware grep: only match in code (skip comments and strings)
{cli} grep "DashMap" --scope code

# Compute byte-range chunks for a file
{cli} chunks src/main.rs [--size 5000] [--overlap 200]
```

### Annotations

```bash
# Save annotations (definitions + marks) to .coderlm/annotations.json
{cli} save-annotations

# Load annotations from disk (auto-loaded on session creation)
{cli} load-annotations
```

### History

```bash
# Session command history
{cli} history [--limit 50]
```

### Base Branch Queries (with review attached)

```bash
# Query base branch snapshot for comparison (requires attached review)
{cli} impl run_server --file src/main.rs --branch base
{cli} callers scan_directory --file src/index/walker.rs --branch base
{cli} symbols --file src/routes.rs --branch base
{cli} tests scan_directory --file src/index/walker.rs --branch base
```

---

## Output Examples

The CLI outputs markdown by default. Every command includes TOON data blocks (where applicable) wrapped in `<data id="name">` tags, and a **Recommended Next Steps** section with `{field}` placeholders referencing TOON column names. Use `--output json` or `CODERLM_FORMAT=json` to get raw JSON instead.

### structure

````markdown
## Project Structure — 42 files

The tree below shows the indexed project layout.

```
├── src/
│   ├── main.rs
│   ├── server/
│   │   ├── routes.rs
│   │   └── state.rs
│   └── ops/
│       └── symbol_ops.rs
└── tests/
    └── integration.rs
```

### Recommended Next Steps

#### 1. List Symbols
```
{cli} symbols
```
...
````

### symbols / search

````markdown
## Symbols — 3 found

...

**Symbols:**
<data id="symbols">
```toon
symbols[3]{name,kind,file,line_range}:
  run_server,function,src/main.rs,"[69, 143]"
  handle_request,function,src/server/routes.rs,"[12, 45]"
  AppState,struct,src/server/state.rs,"[8, 32]"
```
</data>

### Recommended Next Steps

#### 1. Read Symbol Implementations
```
{cli} impl "{name}" --file "{file}"
```
````

`search` returns the same shape, with the heading `## Search Results for "QUERY" — N matches` and data block id `results`.

### impl

````markdown
## scan_directory in src/index/walker.rs (function, lines [10, 58])

The source below is the full body of `scan_directory` as it exists in the indexed snapshot.

```
pub fn scan_directory(root: &Path) -> Result<usize> {
    let walker = WalkBuilder::new(root).build();
    let mut count = 0;
    for entry in walker { ... }
    Ok(count)
}
```

### Recommended Next Steps

#### 1. Find Callers
```
{cli} callers scan_directory --file src/index/walker.rs
```
...
````

### batch-impl

````markdown
## Implementations — 2 found

Each section below contains the full source body of one requested symbol.

### fn_a in src/a.rs (lines [10, 25])
```
pub fn fn_a() { ... }
```

### fn_b in src/b.rs (lines [30, 48])
```
pub fn fn_b(x: usize) -> String { ... }
```
````

### callers

````markdown
## Callers of scan_directory — 2 call sites

...

**Call Sites:**
<data id="callers">
```toon
callers[2]{caller,file,line}:
  run_server,src/main.rs,95
  index_project,src/server/state.rs,42
```
</data>

### Recommended Next Steps

#### 1. Read Caller Implementations
```
{cli} impl "{caller}" --file "{file}"
```
````

### tests

````markdown
## Tests for scan_directory — 1 found

...

**Tests:**
<data id="tests">
```toon
tests[1]{name,file,line}:
  test_scan_directory,tests/integration.rs,12
```
</data>

### Recommended Next Steps

#### 1. Read Test Implementations
```
{cli} impl "{name}" --file "{file}"
```
````

### variables

````markdown
## Variables in scan_directory — 3 bindings

...

**Variables:**
<data id="variables">
```toon
variables[3]{name,function}:
  walker,scan_directory
  count,scan_directory
  entry,scan_directory
```
</data>
````

### peek

````markdown
## src/main.rs (lines 1–10)

The excerpt below shows lines 1–10 of `src/main.rs` from the indexed snapshot.

```
     1 │ mod config;
     2 │ mod index;
     3 │ mod ops;
     4 │ mod server;
     5 │ mod symbols;
```
````

### grep

````markdown
## Grep: "DashMap" — 3 matches

**src/index/file_tree.rs:1**
```
use dashmap::DashMap;
```

**src/server/state.rs:8**
```
    projects: DashMap<PathBuf, Arc<Project>>,
```

### Recommended Next Steps

#### 1. List Symbols in Matched Files
```
{cli} symbols --file "{file}"
```
````

### chunks

````markdown
## Chunks for src/main.rs — 1 chunk

...

**Chunks:**
<data id="chunks">
```toon
chunks[1]{index,start,end}:
  0,0,3521
```
</data>

### Recommended Next Steps

#### 1. Read Each Chunk
```
{cli} peek "src/main.rs" --start {start} --end {end}
```
````

### status

````markdown
## Server Status

**Server:** ok | **Projects:** 2 | **Sessions:** 3
**Session:** sess_abc123def
**Project:** /path/to/myproject
````

---

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

---

## Code Review API

Review endpoints compare two indexed snapshots of a repo (base branch vs PR branch). A review must be created and attached to a session before using the diff/safety/impact endpoints.

### Review Lifecycle

```bash
# Create a review (indexes base branch in a git worktree, computes semantic diff)
{cli} review-init --base main [--head HEAD]
# → polls until status=ready

# Show status of attached review
{cli} review-status

# Delete review and clean up worktree
{cli} review-cleanup
```

### Orientation Queries

```bash
# High-level stats
{cli} review-summary

# Changed files (with status and language)
{cli} review-files

# Changed symbols (filter by change type, file, or kind)
{cli} review-symbols [--change added|deleted|modified] [--file path] [--kind function]
```

### File Context Bundle

Gets patch + changed symbols + source excerpts for a file in a single call.

```bash
{cli} review-file-context --file src/server/routes.rs [--context-lines 50] [--max-full-file-lines 500]
```

`change_type`: `new_file` | `modified` | `modified_deletions_only` | `deleted_file`
`reading_strategy`: `full_file` | `changed_regions_with_context` | `none`

### Symbol Before/After Diff

Gets base and head source for a changed symbol, plus a unified diff.

```bash
{cli} review-symbol-diff --symbol find_callers --file src/ops/symbol_ops.rs
```

### Safety Analysis

```bash
{cli} review-safety                  # Deleted/signature-changed symbols with unmodified callers
{cli} review-body-safety [--depth N] # Body-only behavioral changes with unmodified callers
```

### Cross-Reference Completeness Check

```bash
{cli} review-reference-check --symbol find_callers --file src/ops/symbol_ops.rs
```

`risk`: `high` | `medium` | `low` | `none`

### Import Diff

```bash
{cli} review-import-diff                            # Whole PR
{cli} review-import-diff --file src/review/mod.rs  # One file
```

### Semantic Change Classification

```bash
{cli} review-change-classification
```

`classification` values: `behavioral` | `structural` | `documentation_only` | `type_change` | `error_handling_change` | `api_surface_change` | `import_change`

### Complexity Delta

```bash
{cli} review-complexity                            # Whole PR
{cli} review-complexity --file src/routes.rs       # One file
```

`regression: true` means the head version is more complex than the base (any metric increased).

### Per-Symbol Blast Radius

```bash
{cli} review-impact process_request --file src/server/routes.rs
```

### Test Coverage

```bash
{cli} review-test-coverage
```

---

## Review Output Examples

### review-summary

````markdown
## PR Summary: main..feature/review-primitives

**Files changed:** +3 added, ~7 modified, -1 deleted, →0 renamed
**Symbols changed:** +12 added, ~8 modified, -4 deleted, →0 moved

### Recommended Next Steps

#### 1. Enumerate Changed Files
```
{cli} review-files
```
...
````

### review-files

````markdown
## Changed Files (4 files)

...

**Files:**
<data id="files">
```toon
files[4]{path,status,language}:
  src/review/context.rs,added,rust
  src/review/impact.rs,modified,rust
  src/server/routes.rs,modified,rust
  src/old_module.rs,deleted,rust
```
</data>

### Recommended Next Steps

#### 4. Review Individual Files
```
{cli} review-file-context --file "{path}"
```
````

### review-symbols

````markdown
## Changed Symbols (5 total)

...

**Symbols:**
<data id="symbols">
```toon
symbols[5]{name,kind,change,file}:
  FileContextResult,struct,added,src/review/context.rs
  get_file_context,function,added,src/review/context.rs
  compute_impact,function,modified,src/review/impact.rs
  collect_callers_bfs,function,modified,src/review/impact.rs
  old_handler,function,deleted,src/server/routes.rs
```
</data>

### Recommended Next Steps

#### 1. Inspect Symbol Diffs
```
{cli} review-symbol-diff --symbol "{name}" --file "{file}"
```
````

### review-safety / review-body-safety

````markdown
## Safety Report — 1 issue, 4 safe changes

...

**Issues:**
<data id="issues">
```toon
issues[1]{symbol,file,callers}:
  old_handler,src/server/routes.rs,2
```
</data>

### Recommended Next Steps

#### 1. Inspect What Changed
```
{cli} review-symbol-diff --symbol "{symbol}" --file "{file}"
```

#### 2. Measure the Blast Radius
```
{cli} review-impact "{symbol}" --file "{file}"
```
````

`review-body-safety` uses the same shape with heading `## Body-Change Safety — N issues, N safe changes`.

### review-file-context

````markdown
## File Context: src/review/impact.rs (modified, rust)

**Lines:** 245 | **Change type:** modified | **Reading strategy:** changed_regions_with_context

...

### Changed Ranges

**Ranges:**
<data id="ranges">
```toon
ranges[1]{start,end,symbols}:
  88,134,"['compute_impact']"
```
</data>

### Changed Symbols

**Symbols:**
<data id="symbols">
```toon
symbols[2]{name,kind,change,signature_changed,body_changed}:
  compute_impact,function,modified,true,true
  collect_callers_bfs,function,modified,false,true
```
</data>

### Patch
```diff
--- a/src/review/impact.rs
+++ b/src/review/impact.rs
@@ -88,10 +88,15 @@ pub fn compute_impact(
+    depth: usize,
```

### Source Excerpts

Lines 38–184:
```
// ... source context around changed ranges ...
```

### Recommended Next Steps

#### 1. Inspect Modified Symbols
```
{cli} review-symbol-diff --symbol "{name}" --file "src/review/impact.rs"
```
````

### review-symbol-diff

````markdown
## Symbol Diff: compute_impact in src/review/impact.rs (function, modified)

**Signature changed** — the public interface of this symbol was modified.

| | Signature |
|---|---|
| **Before** | `pub fn compute_impact(sym: &str, file: &str) -> ImpactResult` |
| **After** | `pub fn compute_impact(sym: &str, file: &str, depth: usize) -> ImpactResult` |

### Unified Diff
```diff
--- base
+++ head
@@ -1,12 +1,13 @@
 pub fn compute_impact(
     sym: &str,
     file: &str,
+    depth: usize,
 ) -> ImpactResult {
```

### Base Source (lines [88, 128])
```
pub fn compute_impact(sym: &str, file: &str) -> ImpactResult { ... }
```

### Head Source (lines [88, 134])
```
pub fn compute_impact(sym: &str, file: &str, depth: usize) -> ImpactResult { ... }
```

### Recommended Next Steps

#### 1. Assess Caller Risk
```
{cli} review-impact --symbol compute_impact --file src/review/impact.rs
```
...
````

### review-reference-check

````markdown
## Reference Check: compute_impact in src/review/impact.rs

| Field | Value | Description |
|-------|-------|-------------|
| **signature_changed** | `true` | Whether the symbol's public interface was modified |
| **total_references** | `5` | Total callers found in the codebase |
| **updated in this PR** | `3` | Callers whose files are included in this diff |
| **not updated** | `2` | Callers in files NOT changed by this PR |
| **risk** | `high` | Derived from the `not_updated` count |

### Un-Updated Call Sites

<data id="not_updated">
```toon
not_updated[2]{file,line}:
  src/server/routes.rs,71
  src/ops/analysis.rs,33
```
</data>

### Recommended Next Steps

#### 2. Review Each Un-Updated Call Site
```
{cli} review-file-context --file "{file}"
```
````

### review-import-diff

````markdown
## Import Changes — 2 files affected

...

**Files:**
<data id="files">
```toon
files[2]{file,added,removed}:
  src/review/impact.rs,1,1
  src/review/mod.rs,2,0
```
</data>

### Import Detail

**src/review/impact.rs** (+1 added, -1 removed)
  + `use crate::ops::content;` (line 3)
  - `use crate::ops::history;` (line 5)

**src/review/mod.rs** (+2 added, -0 removed)
  + `use crate::review::context;` (line 4)
  + `use crate::review::references;` (line 5)

### Recommended Next Steps

#### 1. Review Each Affected File
```
{cli} review-file-context --file "{file}"
```
````

### review-change-classification

````markdown
## Change Classification — 4 symbols

...

**Classifications:**
<data id="classifications">
```toon
classifications[4]{symbol,file,classification,sub_classification}:
  compute_impact,src/review/impact.rs,api_surface_change,signature_changed
  FileContextResult,src/review/context.rs,structural,added
  get_file_context,src/review/context.rs,behavioral,body_logic_change
  old_handler,src/server/routes.rs,structural,deleted
```
</data>

### Recommended Next Steps

#### 1. Verify API Surface Changes
```
{cli} review-reference-check --symbol "{symbol}" --file "{file}"
```

#### 2. Inspect Behavioral Changes
```
{cli} review-symbol-diff --symbol "{symbol}" --file "{file}"
```
````

### review-complexity

````markdown
## Complexity Delta — 3 symbols analyzed, 1 regression

...

**Complexity Deltas:**
<data id="deltas">
```toon
deltas[3]{symbol,file,base_lines,head_lines,base_nesting,head_nesting,base_params,head_params,regression}:
  compute_impact,src/review/impact.rs,40,52,3,5,3,4,true
  get_file_context,src/review/context.rs,0,85,0,4,0,3,false
  build_excerpt,src/review/context.rs,0,28,0,2,0,2,false
```
</data>

### Regressions

- **compute_impact** (`src/review/impact.rs`): lines 40→52, nesting 3→5, params 3→4

### Recommended Next Steps

#### 1. Inspect Regressed Symbols
```
{cli} review-symbol-diff --symbol "{symbol}" --file "{file}"
```
````

### review-impact

````markdown
## Impact: compute_impact in src/review/impact.rs — high risk

| Field | Value | Description |
|-------|-------|-------------|
| **change** | `modified` | What happened to this symbol |
| **unmodified callers** | `2` | Call sites in files NOT part of this PR's diff |
| **risk** | `high` | Derived from unmodified caller count |

### Unmodified Call Sites

**Callers:**
<data id="callers">
```toon
callers[2]{symbol,file,line}:
  handle_review,src/server/routes.rs,71
  run_analysis,src/ops/analysis.rs,33
```
</data>

### Test References

**Tests:**
<data id="tests">
```toon
tests[1]{name,file,line}:
  test_compute_impact,tests/integration.rs,88
```
</data>

### Recommended Next Steps

#### 1. Inspect What Changed
```
{cli} review-symbol-diff --symbol compute_impact --file src/review/impact.rs
```
````

### review-test-coverage

````markdown
## Test Coverage

### Covered (1 symbols with tests)

**Covered Symbols:**
<data id="covered">
```toon
covered[1]{symbol,file,kind,test_count}:
  new_handler,src/server/routes.rs,function,1
```
</data>

### Uncovered (2 symbols without tests)

**Uncovered Symbols:**
<data id="uncovered">
```toon
uncovered[2]{symbol,file,kind}:
  get_file_context,src/review/context.rs,function
  FileContextResult,src/review/context.rs,struct
```
</data>

### Recommended Next Steps

#### 1. Prioritize by Impact
```
{cli} review-impact "{symbol}" --file "{file}"
```
````

---

## Error Codes

| Status | Meaning |
|--------|---------|
| 400    | Bad request (missing/invalid parameters) |
| 404    | Resource not found (or expired session — re-run command to auto-reinitialize) |
| 410    | Project evicted — re-run command to auto-reinitialize |
| 500    | Server error |
