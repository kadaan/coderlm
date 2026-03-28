## Changed Files ({{files|length}} files)

Each file touched by this PR's diff is listed below. Use it to understand the scope of changes before proceeding.

### Column Reference

| Column | Description |
|--------|-------------|
| **status** | What happened to the file: `added`, `modified`, `deleted`, or `renamed` |
| **language** | Detected language — determines how symbols were extracted from this file |

**Files:**
{% toon files files %}

### Recommended Next Steps

Run these commands before reviewing individual files — they surface risk and establish priority across the full diff.

#### 1. Assess Breakage Risk
Identifies changed symbols called from files **not** in this PR. Any flagged symbol is a potential unintended breakage point. Review this output before reading any individual file — it determines where to focus first:
```
{{_cli}} review-safety
```

#### 2. Audit Dependency Changes
Shows all import additions and removals across changed files. New imports may introduce unexpected dependencies or version conflicts; removed imports may indicate deleted or relocated functionality. Flag any unfamiliar new imports for closer review:
```
{{_cli}} review-import-diff
```

#### 3. Identify Coverage Gaps
Lists changed symbols with no associated test coverage. Use the output to prioritize which files in step 4 warrant the most careful manual review:
```
{{_cli}} review-test-coverage
```

#### 4. Review Individual Files
Load full context for each file you intend to review — diff patch, surrounding source, and changed symbols. Prioritize files that contain symbols flagged in steps 1 or 3. For each row in `<data id="files">`:
```
{{_cli}} review-file-context --file "{path}"
```
