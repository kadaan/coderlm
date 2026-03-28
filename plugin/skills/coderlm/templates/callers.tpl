## Callers of {{symbol}} — {{callers|length}} call sites

The table below lists every location in the codebase that calls `{{symbol}}`. Use it to understand the symbol's usage before changing its signature or behavior.

{% if callers %}
### Column Reference

| Column | Description |
|--------|-------------|
| **caller** | Name of the function or method containing the call site |
| **file** | File containing the call site |
| **line** | Line number of the call |

**Call Sites:**
{% toon callers callers %}

### Recommended Next Steps

#### 1. Read Caller Implementations
Load the full source body of each calling function to understand how it uses `{{symbol}}`. Use this to assess whether a change to `{{symbol}}` will affect each caller:
```
{{_cli}} impl "{caller}" --file "{file}"
```
{% else %}
No call sites found for `{{symbol}}`. The symbol is either unused or its callers were not indexed.
{% endif %}
