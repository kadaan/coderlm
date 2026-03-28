## File Context: {{file}} ({{status}}, {{language}})

**Lines:** {{total_lines}} | **Change type:** {{change_type}} | **Reading strategy:** {{reading_strategy}}

### Field Reference

| Field | Description |
|-------|-------------|
| **change_type** | How this file was affected: `added`, `modified`, `deleted`, or `renamed` |
| **reading_strategy** | How source excerpts were selected: `full_file` shows the entire file; `changed_regions` shows only source context around changed lines (used for large files) |

{% if changed_ranges %}
### Changed Ranges

Line ranges in the head commit that contain modifications.

**Ranges:**
{% toon ranges changed_ranges %}
{% endif %}

{% if symbols %}
### Changed Symbols

Symbols within this file that differ from the base commit.

| Column | Description |
|--------|-------------|
| **name** | Symbol identifier |
| **kind** | Symbol type (function, method, class, etc.) |
| **change** | `added`, `modified`, or `deleted` |

**Symbols:**
{% toon symbols symbols %}
{% endif %}

### Patch
```diff
{{patch}}
```

{% if source_excerpts %}
### Source Excerpts
{% for excerpt in source_excerpts %}
Lines {{excerpt.start_line}}–{{excerpt.end_line}}:
```
{{excerpt.content}}
```
{% endfor %}
{% endif %}

### Recommended Next Steps

#### 1. Inspect Modified Symbols
For each `modified` symbol in the Symbols table above, read the before/after body in isolation to understand what changed without the noise of the full file diff. If the change alters return values, side effects, error handling, or control flow, treat it as high risk and proceed to step 2:
```
{{_cli}} review-symbol-diff --symbol "{name}" --file "{{file}}"
```

#### 2. Measure Blast Radius
For any symbol flagged as high risk in step 1, find all callers NOT updated in this PR. A symbol with many unmodified callers and a behavioral or signature change is a strong candidate for a blocking review comment:
```
{{_cli}} review-impact "{name}" --file "{{file}}"
```
