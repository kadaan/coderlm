## Chunks for {{file}} — {{chunks|length}} chunks

The table below divides `{{file}}` into non-overlapping byte-range chunks. Use it to load large files in pieces when the full file is too large to read at once.

{% if chunks %}
### Column Reference

| Column | Description |
|--------|-------------|
| **index** | Zero-based chunk number |
| **start** | Byte offset where the chunk begins |
| **end** | Byte offset where the chunk ends (exclusive) |

**Chunks:**
{% toon chunks chunks %}

### Recommended Next Steps

#### 1. Read Each Chunk
Load the source content for each chunk in order. Use the `start` and `end` byte offsets from the table above:
```
{{_cli}} peek "{{file}}" --start {start} --end {end}
```
{% else %}
No chunks found for `{{file}}`. The file may be empty or not indexed.
{% endif %}
