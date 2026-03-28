## Grep: "{{pattern}}" — {{matches|length}} matches

Each result below shows the matching line and surrounding context. Use it to locate where a pattern is used across the codebase before loading any symbol or file in detail.

{% if matches %}
{% for m in matches %}
**{{m.file}}:{{m.line}}**
```
{{m.context}}
```
{% endfor %}

### Recommended Next Steps

#### 1. List Symbols in Matched Files
Load the symbol inventory for each file containing a match. Use this to identify which functions surround the matched lines before reading implementations:
```
{{_cli}} symbols --file "{file}"
```
{% else %}
No matches found for `{{pattern}}`.
{% endif %}
