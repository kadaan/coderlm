## Project Structure{% if total_files %} — {{total_files}} files{% endif %}

The tree below shows the indexed project layout. Use it to orient yourself to the codebase before loading any symbols or source.

```
{{tree}}
```

### Recommended Next Steps

#### 1. List Symbols
Load all symbols in the project, or filter by kind to get a focused inventory. Use the output to identify which files and functions to explore first:
```
{{_cli}} symbols
```
```
{{_cli}} symbols --kind function
```

#### 2. Search for a Symbol
Find a specific symbol by name substring. Use this when you know roughly what you're looking for but not which file it's in:
```
{{_cli}} search "QUERY"
```

#### 3. Grep for a Pattern
Search across all indexed file contents by regex. Use this for identifiers, string literals, or patterns that span multiple symbols or files:
```
{{_cli}} grep "PATTERN"
```
