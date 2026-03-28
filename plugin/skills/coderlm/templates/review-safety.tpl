## Safety Report — {{issues|length}} issues, {{safe_changes_count}} safe changes

This report flags symbols that were **deleted or had their signature changed** in this PR but are still called from code NOT in the diff. Each issue represents a call site that was not updated and may now be broken.

{% if issues %}
### Column Reference

| Column | Description |
|--------|-------------|
| **symbol** | The changed or deleted symbol |
| **file** | File containing the symbol in the head commit |
| **callers** | Number of unmodified call sites found outside this PR |

**Issues:**
{% toon issues issues %}

### Recommended Next Steps

#### 1. Inspect What Changed
Read the before/after diff for each flagged symbol to determine whether the change is backward-compatible or a hard break. If the signature changed, un-updated callers may fail to compile; if the symbol was deleted, all un-updated callers are broken:
```
{{_cli}} review-symbol-diff --symbol "{symbol}" --file "{file}"
```

#### 2. Measure the Blast Radius
For each flagged symbol, find all callers NOT updated in this PR. Use the caller count to prioritize which symbols to escalate — high caller counts are strong candidates for a blocking review comment:
```
{{_cli}} review-impact "{symbol}" --file "{file}"
```

#### 3. Check Behavioral Changes
This report only catches signature changes and deletions. Run `review-body-safety` to surface the complementary case: symbols that kept their signature but changed behavior, whose callers are equally at risk of silent breakage:
```
{{_cli}} review-body-safety
```

{% else %}
No safety issues found. All deleted and signature-changed symbols either had no external callers, or all callers were updated within this PR.

### Recommended Next Steps

#### 1. Check Behavioral Changes
This report only catches signature changes and deletions. Run `review-body-safety` to surface the complementary case: symbols that kept their signature but changed behavior. Those callers won't appear here but are equally at risk of silent breakage:
```
{{_cli}} review-body-safety
```
{% endif %}
