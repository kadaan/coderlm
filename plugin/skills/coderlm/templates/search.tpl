## Search Results for "{{query}}" — {{results|length}} matches

The table below lists every symbol whose name contains `{{query}}`. Use it to locate the specific function, class, or type you're looking for before reading its source.

{% if results %}
### Column Reference

| Column | Description |
|--------|-------------|
| **name** | The symbol identifier as it appears in source |
| **kind** | Symbol type: `function`, `method`, `class`, `struct`, `enum`, `trait`, `interface`, `constant`, `type`, or `module` |
| **file** | File path containing the symbol |
| **line_range** | Start and end line numbers in the file |

**Matches:**
{% toon results results %}

### Recommended Next Steps

#### 1. Read Symbol Implementations
Load the full source body of each match. Use this to confirm which result is the one you're looking for and to understand its logic:
```
{{_cli}} impl "{name}" --file "{file}"
```
{% else %}
No symbols found matching "{{query}}".
{% endif %}
