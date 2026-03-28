## Impact: {{symbol}} in {{file}} — {{risk}} risk

### Symbol Reference

| Field | Value | Description |
|-------|-------|-------------|
| **change** | `{{change}}` | What happened to this symbol: deleted, signature changed, behavioral change, etc. |
| **unmodified callers** | `{{unmodified_callers|length}}` | Call sites in files NOT part of this PR's diff — these callers had no opportunity to be updated |
| **risk** | `{{risk}}` | Derived from unmodified caller count: `high` means widely used outside this PR; `low` means mostly self-contained |

{% if unmodified_callers %}
### Unmodified Call Sites

These call sites were not touched by this PR and may be affected by the change to `{{symbol}}`.

| Column | Description |
|--------|-------------|
| **symbol** | Calling symbol name |
| **file** | File containing the call site |
| **line** | Line number of the call |

**Callers:**
{% toon callers unmodified_callers %}
{% endif %}

{% if tests %}
### Test References

Test references found for `{{symbol}}`. Use these to assess whether existing tests would catch a behavioral regression:

**Tests:**
{% toon tests tests %}
{% endif %}

### Recommended Next Steps

#### 1. Inspect What Changed
Read the before/after body of `{{symbol}}` to understand the nature of the change. If the signature changed, unmodified callers may fail to compile; if the body changed, they may be silently affected. Use this to assess which unmodified callers are at risk before raising a concern:
```
{{_cli}} review-symbol-diff --symbol {{symbol}} --file {{file}}
```

#### 2. Get a Precise Caller Breakdown
Splits all callers into those updated within this PR and those not, with an explicit risk rating. Run this if the unmodified caller count in step 1 warrants a closer look — the risk rating determines whether to flag this as a blocking issue:
```
{{_cli}} review-reference-check --symbol {{symbol}} --file {{file}}
```

#### 3. Review the Changed File
Load the full diff context for `{{file}}` to see what else changed alongside `{{symbol}}`. Use this if the symbol diff alone doesn't provide enough context to assess the change:
```
{{_cli}} review-file-context --file {{file}}
```
