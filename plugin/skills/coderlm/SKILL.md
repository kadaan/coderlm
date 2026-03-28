---
name: coderlm
description: "Primary tool for all code navigation, reading, and pull request review in supported languages (Rust, Python, TypeScript, JavaScript, Go, Java, Scala, SQL). Use instead of Read, Grep, and Glob for finding symbols, reading function implementations, tracing callers, discovering tests, understanding execution paths, and reviewing PRs. For code review: provides semantic diff of base vs PR branch — changed files, changed symbols, safety analysis (deleted/signature-changed symbols with unmodified callers), body-change safety, cross-reference completeness, import diffs, change classification, complexity regression detection, and per-file context bundles. Use for: finding functions by name or pattern, reading specific implementations, answering 'what calls X', 'where does this error come from', 'how does X work', tracing from entrypoint to outcome, any codebase exploration, and systematic PR review. Use Read only for config files, markdown, and unsupported languages."
allowed-tools:
  - Bash
  - Read
---

# CodeRLM — Structural Codebase Exploration & PR Review

You have access to a tree-sitter-backed index server that knows the structure of this codebase: every function, every caller, every symbol, every test reference. Use it instead of guessing with grep.

The tree-sitter is monitoring the directory and will stay up-to-date as you make changes in the codebase.

## How to Explore

Do not scan files looking for relevant code. Work the way an engineer traces through a codebase:

**Start from an entrypoint.** Every exploration begins somewhere concrete — an error message, a function name, an API endpoint, a log line. Use `search` or `grep` to locate that entrypoint in the index.

**Trace the path.** Once you've found an entrypoint, use `callers` to understand what invokes it and `impl` to read what it does. Follow the chain: what calls this? What does that caller do? What state does it pass in? Build a model of the execution path, not a list of files.

**Understand the sequence of events.** The goal is to reconstruct the causal chain — what had to happen to produce the state you're looking at. Trace upstream (what called this, with what arguments?) and sometimes downstream (what happens after, does it matter?).

**Stop when you have the narrative.** You're done exploring when you can explain the path from trigger to outcome — not when you've read every related file.

## What This Replaces

Without the index, you explore by globbing for filenames, grepping for strings, and reading entire files hoping to find relevant sections. That works, but it's wasteful and produces false confidence — you see code near your search term but miss the actual execution path.

With the index:
- **Symbol search** instead of string matching — find the function, not every comment mentioning it
- **Caller chains** instead of grep-and-hope — know exactly what invokes a function
- **Exact implementations** instead of full-file reads — get the 20-line function body, not the 500-line file
- **Test discovery** by symbol reference — find what tests cover a function, not by guessing test filenames
- **Semantic PR diff** instead of raw git diff — see which symbols changed, what their callers are, and where coverage is missing

## Prerequisites

The `coderlm-server` must be running. Start it separately:

```bash
coderlm-server serve                     # indexes projects on-demand
coderlm-server serve /path/to/project    # pre-index a specific project
```

If the server is not running, all CLI commands will fail with a connection error.

Sessions are created automatically on first use — no explicit `init` is required.

## CLI Reference

All commands go through the wrapper script:

```bash
${CLAUDE_SKILL_DIR}/scripts/coderlm <command> [args]
```

### Setup (Optional)

```bash
${CLAUDE_SKILL_DIR}/scripts/coderlm init                      # Explicit session init (optional)
${CLAUDE_SKILL_DIR}/scripts/coderlm structure --depth 2       # File tree with language breakdown
```

`init` is optional. Any command that needs a session will create one automatically against the current working directory.

### Finding Code

```bash
${CLAUDE_SKILL_DIR}/scripts/coderlm search "symbol_name" --limit 20       # Find symbols by name (index lookup)
${CLAUDE_SKILL_DIR}/scripts/coderlm symbols --kind function --file path   # List all functions in a file
${CLAUDE_SKILL_DIR}/scripts/coderlm grep "pattern" --max-matches 20       # Scope-aware pattern search
```

### Retrieving Exact Code

```bash
${CLAUDE_SKILL_DIR}/scripts/coderlm impl function_name --file path        # Full function body (tree-sitter extracted)
${CLAUDE_SKILL_DIR}/scripts/coderlm peek path --start N --end M           # Exact line range
${CLAUDE_SKILL_DIR}/scripts/coderlm variables function_name --file path   # Local variables inside a function
```

**Prefer `impl` and `peek` over the Read tool.** They return exactly the code you need — a single function from a 1000-line file, a specific line range — without loading irrelevant code into context. Fall back to Read only when you need an entire small file.

### Tracing Connections

```bash
${CLAUDE_SKILL_DIR}/scripts/coderlm callers function_name --file path     # Every call site: file, line, calling code
${CLAUDE_SKILL_DIR}/scripts/coderlm tests function_name --file path       # Tests referencing this symbol
```

These search the entire indexed codebase, not just files you've already seen.

### Annotating

```bash
${CLAUDE_SKILL_DIR}/scripts/coderlm define-file src/server/mod.rs "HTTP routing and handler dispatch"
${CLAUDE_SKILL_DIR}/scripts/coderlm define-symbol handle_request --file src/server/mod.rs "Routes requests by method+path"
${CLAUDE_SKILL_DIR}/scripts/coderlm mark tests/integration.rs test
```

Annotations persist across queries within a session — build shared understanding as you go.

### Cleanup

```bash
${CLAUDE_SKILL_DIR}/scripts/coderlm cleanup                               # End session (also cleans up any review)
```

### Code Review

Code review mode indexes two snapshots of the repo — the base branch and the PR branch — and provides semantic diff queries.

```bash
# Setup: attach a review to the current session (no separate init needed)
${CLAUDE_SKILL_DIR}/scripts/coderlm review-init --base main              # Indexes base + computes diff (polls until ready)

# Orientation
${CLAUDE_SKILL_DIR}/scripts/coderlm review-summary                       # Stats: files/symbols added/deleted/modified
${CLAUDE_SKILL_DIR}/scripts/coderlm review-files                         # List of changed files with status + language
${CLAUDE_SKILL_DIR}/scripts/coderlm review-symbols                       # All changed symbols
${CLAUDE_SKILL_DIR}/scripts/coderlm review-symbols --change modified     # Only modified symbols
${CLAUDE_SKILL_DIR}/scripts/coderlm review-symbols --file src/routes.rs  # Changes in a specific file

# File-level context bundle (patch + changed symbols + source excerpts in one call)
${CLAUDE_SKILL_DIR}/scripts/coderlm review-file-context --file src/routes.rs

# Symbol before/after diff
${CLAUDE_SKILL_DIR}/scripts/coderlm review-symbol-diff --symbol process_request --file src/routes.rs

# Safety analysis (highest value)
${CLAUDE_SKILL_DIR}/scripts/coderlm review-safety                        # Deleted/signature-changed symbols with unmodified callers
${CLAUDE_SKILL_DIR}/scripts/coderlm review-body-safety                   # Body-only changes with unmodified callers

# Cross-reference completeness (catch incomplete refactoring)
${CLAUDE_SKILL_DIR}/scripts/coderlm review-reference-check --symbol process_request --file src/routes.rs

# Import changes
${CLAUDE_SKILL_DIR}/scripts/coderlm review-import-diff                   # All import changes across PR
${CLAUDE_SKILL_DIR}/scripts/coderlm review-import-diff --file src/routes.rs  # One file

# Semantic change classification
${CLAUDE_SKILL_DIR}/scripts/coderlm review-change-classification         # Classify each change: behavioral, type_change, api_surface_change, etc.

# Complexity regression detection
${CLAUDE_SKILL_DIR}/scripts/coderlm review-complexity                    # Symbols with increased nesting/length/parameters

# Per-symbol blast radius
${CLAUDE_SKILL_DIR}/scripts/coderlm review-impact process_request --file src/routes.rs

# Test coverage
${CLAUDE_SKILL_DIR}/scripts/coderlm review-test-coverage                 # Which added/modified symbols have no tests?

# Batch implementations (N symbols in one call)
${CLAUDE_SKILL_DIR}/scripts/coderlm batch-impl --symbols '[{"symbol":"fn_a","file":"src/a.rs"},{"symbol":"fn_b","file":"src/b.rs"}]'

# Per-symbol old vs new comparison
${CLAUDE_SKILL_DIR}/scripts/coderlm impl process_request --file src/routes.rs             # New version
${CLAUDE_SKILL_DIR}/scripts/coderlm impl process_request --file src/routes.rs --branch base  # Old version

# Cleanup
${CLAUDE_SKILL_DIR}/scripts/coderlm review-cleanup                       # Delete review, clean up worktree
```

## Inputs

This skill reads `$ARGUMENTS`. Accepted patterns:
- `query=<question>` (required): what to find or understand
- `cwd=<path>` (optional): project directory, defaults to cwd
- `port=<N>` (optional): server port, defaults to 3000

If no query is provided, ask what the user wants to find or understand about the codebase.

## Workflow

1. **Orient** — `${CLAUDE_SKILL_DIR}/scripts/coderlm structure` to see the project layout. A session is created automatically. Identify likely starting points.
2. **Find the entrypoint** — `search` or `grep` to locate the starting symbol or pattern.
3. **Retrieve** — `impl` to read the exact implementation. Not the file. The function.
4. **Trace** — `callers` to see what calls it. `impl` on those callers. Follow the chain.
5. **Widen** — `tests` to find test coverage. `grep` for related patterns discovered during tracing.
6. **Annotate** — `define-symbol` and `define-file` as understanding solidifies.
7. **Synthesize** — Compile findings into a coherent answer with specific file:line references.

Steps 2–6 repeat. A typical exploration is: find a symbol → read its implementation → trace its callers → read those implementations → discover related symbols → repeat until the causal chain is clear.

## Code Review Workflow

For reviewing a pull request (comparing a base branch against a PR branch):

1. **Start review** — `${CLAUDE_SKILL_DIR}/scripts/coderlm review-init --base main` to index the base branch in a worktree, compute the semantic diff, and attach to the session. A session is created automatically. Waits until ready.
2. **Orient** — `review-summary` to understand the size/scope.
3. **Changed files** — `review-files` to see what changed.
4. **File context bundles** — `review-file-context --file PATH` for each important file: gets patch + changed symbols + source excerpts in one call.
5. **Symbol diffs** — `review-symbol-diff --symbol NAME --file FILE` to see base vs head side-by-side for modified symbols.
6. **Safety check** — `review-safety` to find deleted/signature-changed symbols with callers that weren't updated. Then `review-body-safety` for body-only behavioral changes.
7. **Reference completeness** — `review-reference-check --symbol NAME --file FILE` for any symbol whose signature changed, to find call sites that weren't updated.
8. **Change classification** — `review-change-classification` to get a semantic label per symbol (behavioral, api_surface_change, type_change, etc.) for routing attention.
9. **Complexity regressions** — `review-complexity` to find symbols that got measurably more complex.
10. **Import changes** — `review-import-diff` to see new dependencies and removed ones.
11. **Drill down** — `review-impact SYMBOL --file FILE` for per-symbol blast radius on high-risk changes.
12. **Test coverage** — `review-test-coverage` to find new/modified symbols without test coverage.
13. **Synthesize** — Build a review with specific file:line references, risk assessments, and actionable findings.
14. **Cleanup** — `review-cleanup` (or `cleanup` which handles both).

## When to Use the Server vs Native Tools

### Codebase Exploration

| Task | Use | Why |
|------|-----|-----|
| Find a function by name | `search "name"` | Index lookup, exact match — no file scanning |
| Find code when name is unknown | `grep "pattern" --scope code` | Searches all indexed files, skips comments/strings |
| Read a function body | `impl name --file path` | Returns just that function, even from a 1000-line file |
| Read specific lines | `peek path --start N --end M` | Surgical extraction, not the whole file |
| Find what calls a function | `callers name --file path` | Cross-project BFS with exact call sites |
| Find tests for a function | `tests name --file path` | By symbol reference, not filename guessing |
| List all symbols in a file | `symbols --file path` | Full symbol table for the file |
| Get project overview | `structure` | Tree with file counts and language breakdown |
| Read an entire small config/markdown file | Read tool | When you genuinely need the whole file |
| File type not supported (YAML, TOML, etc.) | Read tool | No symbol table available |

### Code Review

| Task | Use | Why |
|------|-----|-----|
| Understand scope of PR | `review-summary` then `review-files` | Stats + file list without reading any code |
| Read a changed file's diff and context | `review-file-context --file PATH` | Patch + changed symbols + source excerpts in one call |
| Compare old vs new for a symbol | `review-symbol-diff --symbol NAME --file FILE` | Side-by-side with unified diff, no manual branch switching |
| Find breaking changes | `review-safety` | Deleted/signature-changed symbols with unmodified callers |
| Find silent behavioral changes | `review-body-safety` | Body-changed symbols (same signature) with unmodified callers |
| Check if a refactor was complete | `review-reference-check --symbol NAME --file FILE` | Lists call sites not updated in the PR |
| Classify changes for routing | `review-change-classification` | Labels each symbol: behavioral, api_surface_change, type_change, etc. |
| Spot complexity increases | `review-complexity` | Nesting depth, line count, parameter count — base vs head |
| Spot new/removed dependencies | `review-import-diff` | Import additions and removals per file |
| Find untested new code | `review-test-coverage` | New/modified symbols with no test references |
| Read multiple function bodies at once | `batch-impl --symbols '[...]'` | N implementations in one round-trip |
| Read raw git diff | Read tool on patch file | When you need the raw unified diff, not semantic analysis |

**Default to the server.** Use Read only when you need an entire file or the server is unavailable.

## Troubleshooting

- **"Cannot connect to coderlm-server"** — Server not running. Start with `coderlm-server serve`.
- **"Session expired (server was restarted)"** — Re-run the same command; a new session will be created automatically.
- **"Project was evicted"** — Server hit capacity (default 5 projects). Re-run your command; a new session will be created automatically.
- **Search returns nothing relevant** — Try broader grep patterns or list all symbols: `${CLAUDE_SKILL_DIR}/scripts/coderlm symbols --limit 200`.

For the full API endpoint reference, see [references/api-reference.md](references/api-reference.md).
