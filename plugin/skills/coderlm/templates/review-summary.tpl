## PR Summary: {{base_ref}}..{{head_ref}}

This is the scope of the diff indexed for review. Use these numbers to gauge effort and decide where to focus before running any deeper analysis.

**Files changed:** +{{files_added}} added, ~{{files_modified}} modified, -{{files_deleted}} deleted, →{{files_renamed}} renamed
**Symbols changed:** +{{symbols_added}} added, ~{{symbols_modified}} modified, -{{symbols_deleted}} deleted, →{{symbols_moved}} moved

### Recommended Next Steps

Run these in order to build a complete picture of the PR before reading individual files or diffs.

#### 1. Enumerate Changed Files
Lists every file touched by the diff with its change type and language. Use this to orient yourself to the PR's scope and decide which files to prioritize:
```
{{_cli}} review-files
```

#### 2. Classify Changes by Type
Classifies each changed symbol as an API surface change, behavioral change, refactor, addition, or deletion. Run this before reading any diffs — it determines what kind of scrutiny each symbol needs and surfaces the highest-risk changes up front:
```
{{_cli}} review-change-classification
```

#### 3. Assess Breakage Risk
Finds symbols deleted or signature-changed in this PR that are still called from code NOT in the diff. These are the highest-priority breakage risks — flag any with a high caller count as candidates for a blocking review comment before proceeding:
```
{{_cli}} review-safety
```

#### 4. Identify Coverage Gaps
Shows which new and modified symbols have no test references in the codebase. Use the output to prioritize which symbols in steps 1–3 warrant the closest manual review:
```
{{_cli}} review-test-coverage
```

#### 5. Check Complexity Regressions
Shows whether any changed symbols became significantly more complex (more lines, deeper nesting, more parameters). If regressions appear, run `review-symbol-diff` on the flagged symbols to determine whether the increase is justified or warrants a review comment:
```
{{_cli}} review-complexity
```
