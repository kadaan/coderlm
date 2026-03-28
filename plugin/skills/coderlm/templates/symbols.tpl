## Symbols — {{symbols|length}} found

The table below lists every symbol matching the query. Use it to identify which functions, methods, or types to inspect before loading any source.

{% if symbols %}
### Column Reference

| Column | Description |
|--------|-------------|
| **name** | The symbol identifier as it appears in source |
| **kind** | Symbol type: `function`, `method`, `class`, `struct`, `enum`, `trait`, `interface`, `constant`, `type`, or `module` |
| **file** | File path containing the symbol |
| **line_range** | Start and end line numbers in the file |

**Symbols:**
{% toon symbols symbols %}

### Recommended Next Steps

#### 1. Read Symbol Implementations
Load the full source body of each symbol you want to inspect. Use this to understand what a function does before checking who calls it:
```
{{_cli}} impl "{name}" --file "{file}"
```
{% else %}
No symbols found matching the given filters.
{% endif %}
