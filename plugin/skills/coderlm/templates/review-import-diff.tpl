## Import Changes — {{files|length}} files affected

Import changes reveal shifts in what each file depends on. **Added imports** may introduce new external dependencies, expand the attack surface, or indicate new code paths that need testing. **Removed imports** may reflect dead code cleanup, deleted functionality, or a dependency being replaced.

{% if files %}
### Summary

| Column | Description |
|--------|-------------|
| **file** | File with import changes |
| **added** | Number of import statements added |
| **removed** | Number of import statements removed |

**Files:**
{% toon files files %}

### Import Detail

{% for f in files %}
**{{f.file}}** (+{{f.added_imports|length}} added, -{{f.removed_imports|length}} removed)
{% for imp in f.added_imports %}
  + `{{imp.text}}` (line {{imp.line}})
{% endfor %}
{% for imp in f.removed_imports %}
  - `{{imp.text}}` (line {{imp.line}})
{% endfor %}
{% endfor %}

### Recommended Next Steps

#### 1. Review Each Affected File
Load the full diff and source context for each file to see how new imports are used and whether removed imports leave any call sites dangling. Flag any added import from an unfamiliar or unexpected package as a dependency worth scrutinizing. For each row in `<data id="files">`:
```
{{_cli}} review-file-context --file "{file}"
```

{% else %}
No import changes detected.
{% endif %}
