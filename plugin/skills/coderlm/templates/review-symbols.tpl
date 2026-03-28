## Changed Symbols ({{symbols|length}} total)

Every symbol that differs between the base and head commits is listed below. Use it to identify which functions, classes, and methods to inspect before drilling into individual diffs.

### Column Reference

| Column | Description |
|--------|-------------|
| **name** | The symbol identifier as it appears in source |
| **kind** | Symbol type: `function`, `method`, `class`, `struct`, `enum`, `trait`, `interface`, `constant`, `type`, or `module` |
| **change** | What happened: `added`, `modified`, `deleted`, or `moved` |
| **file** | File path in the head commit |

**Symbols:**
{% toon symbols symbols %}

### Recommended Next Steps

#### 1. Inspect Symbol Diffs
For each symbol you want to understand in detail, load its before/after body diff, unified diff, and signature change indicator. Prioritize `modified` and `deleted` symbols — these are most likely to affect existing callers:
```
{{_cli}} review-symbol-diff --symbol "{name}" --file "{file}"
```
