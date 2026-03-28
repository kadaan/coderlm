## Reference Check: {{symbol}} in {{file}}

The table below shows how callers of `{{symbol}}` are distributed across this PR's diff and the broader codebase.

### Symbol Reference

| Field | Value | Description |
|-------|-------|-------------|
| **signature_changed** | `{{signature_changed}}` | Whether the symbol's public interface was modified (parameters, return type, or visibility) |
| **total_references** | `{{total_references}}` | Total callers found in the codebase |
| **updated in this PR** | `{{updated_in_pr}}` | Callers whose files are included in this diff — presumably updated alongside the change |
| **not updated** | `{{not_updated|length}}` | Callers in files NOT changed by this PR — they had no opportunity to be updated and may now be broken or silently affected |
| **risk** | `{{risk}}` | Derived from the `not_updated` count: `high` means many callers are at risk; `low` means few or none |

{% if not_updated %}
### Un-Updated Call Sites

These call sites were not touched by this PR and may be broken or silently affected by the change to `{{symbol}}`.

| Column | Description |
|--------|-------------|
| **file** | File containing an un-updated call site |
| **line** | Line number of the call |

**Call Sites:**
{% toon not_updated not_updated %}

### Recommended Next Steps

#### 1. Review the Symbol Change
Read the before/after diff for `{{symbol}}` to understand the nature of the change before assessing caller risk. If the signature changed, un-updated callers may fail to compile; if the body changed, they may be silently affected:
```
{{_cli}} review-symbol-diff --symbol {{symbol}} --file {{file}}
```

#### 2. Review Each Un-Updated Call Site
For each un-updated call site, load the calling file to see the call in context. Verify whether the caller is actually affected — some may use `{{symbol}}` in a way that isn't impacted by this change. Flag any that are at risk as candidates for a blocking review comment:
```
{{_cli}} review-file-context --file "{file}"
```

{% else %}
All callers of `{{symbol}}` are within files updated by this PR, or there are no external callers.

### Recommended Next Steps

#### 1. Review the Symbol Change
Read the before/after diff for `{{symbol}}` to confirm the nature and scope of the change:
```
{{_cli}} review-symbol-diff --symbol {{symbol}} --file {{file}}
```
{% endif %}
