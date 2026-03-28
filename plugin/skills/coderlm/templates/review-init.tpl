## Review Ready: {{base_ref}}..{{head_ref}}

The review index is built and attached to the current session. All `review-*` commands are now available.

**Commits:** `{{base_commit}}` → `{{head_commit}}`
**Files changed:** +{{stats.files_added}} added, ~{{stats.files_modified}} modified, -{{stats.files_deleted}} deleted
**Symbols changed:** +{{stats.symbols_added}} added, ~{{stats.symbols_modified}} modified, -{{stats.symbols_deleted}} deleted, →{{stats.symbols_moved}} moved

{% if stale %}
**Warning:** The head ref `{{head_ref}}` has moved since this review was indexed. The diff may not reflect the current state of the branch. Run `review-update` to reindex against the current head.
{% endif %}

### Recommended Next Steps

#### 1. Get the Full Diff Summary
Shows the complete change scope with all file and symbol counts. Run this first to confirm the review covers the diff you expect before running any deeper analysis.
```
{{_cli}} review-summary
```

#### 2. Enumerate Changed Files
Lists every file touched by the diff with its change type and language. Use it to understand the PR's scope and identify which areas to investigate before reading any diffs.
```
{{_cli}} review-files
```

#### 3. Classify Changes by Type
Classifies each changed symbol as an API surface change, behavioral change, refactor, addition, or deletion. Run this to determine what kind of scrutiny the diff requires.
```
{{_cli}} review-change-classification
```

#### 4. Assess Breakage Risk
Finds symbols whose signature changed or were deleted, but are still called from unmodified code. Run this if step 3 surfaces any `api_surface_change` or `deleted` symbols.
```
{{_cli}} review-safety
```