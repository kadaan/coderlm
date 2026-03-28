## Complexity Delta — {{deltas|length}} symbols analyzed, {{regression_count}} regressions

Each row compares a symbol's complexity metrics before this PR (base) and after (head). A **regression** means one or more metrics increased.

Not every regression is a problem — added functionality legitimately adds lines. Unexpected increases in nesting depth or parameter count are worth reviewing: increased nesting makes logic harder to follow and test; increased parameters may indicate concerns aren't being separated cleanly.

### Column Reference

| Column | Description |
|--------|-------------|
| **symbol** | Symbol name |
| **file** | File containing the symbol |
| **base.lines** | Line count before this PR |
| **head.lines** | Line count after this PR |
| **base.max_nesting** | Deepest nesting level before this PR |
| **head.max_nesting** | Deepest nesting level after this PR |
| **base.parameters** | Parameter count before this PR |
| **head.parameters** | Parameter count after this PR |
| **regression** | `true` if any metric increased |

**Complexity Deltas:**
{% toon deltas deltas %}

{% if regression_count %}
### Regressions

{% for d in deltas %}
{% if d.regression %}
- **{{d.symbol}}** (`{{d.file}}`): lines {{d.base.lines}}→{{d.head.lines}}, nesting {{d.base.max_nesting}}→{{d.head.max_nesting}}, params {{d.base.parameters}}→{{d.head.parameters}}
{% endif %}
{% endfor %}

### Recommended Next Steps

#### 1. Inspect Regressed Symbols
For each regression in the table above (`regression: true`), read the diff to understand what drove the increase. Look specifically for: deeply nested conditionals, new parameters that could be grouped into a struct, or logic that could be extracted into a helper. If you identify any of these, note them as review comments:
```
{{_cli}} review-symbol-diff --symbol "{symbol}" --file "{file}"
```
{% endif %}
