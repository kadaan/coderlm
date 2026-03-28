## Body-Change Safety — {{issues|length}} issues, {{safe_changes_count}} safe changes

These symbols changed their implementation (body) while keeping the same signature. Because the signature didn't change, their callers weren't required to update — they will compile and run without modification. However, the behavior those callers rely on may have quietly changed.

{% if issues %}
| Column | Description |
|--------|-------------|
| **symbol** | The symbol whose body changed |
| **file** | File containing the symbol |
| **callers** | Number of unmodified call sites found outside this PR |

**Changed Symbols:**
{% toon issues issues %}

### Recommended Next Steps

#### 1. Inspect What Changed
Read the before/after body of each flagged symbol to understand whether the behavioral change is likely to affect callers silently. If the change alters return values, side effects, error handling, or control flow, treat it as high risk and proceed to step 2:
```
{{_cli}} review-symbol-diff --symbol "{symbol}" --file "{file}"
```

#### 2. Measure the Blast Radius
For any symbol you flagged as high risk in step 1, find its unmodified callers. A symbol with many unmodified callers and a behavioral change is a strong candidate for a blocking review comment:
```
{{_cli}} review-impact "{symbol}" --file "{file}"
```
{% else %}
No body-change safety issues found. All symbols with behavioral changes either had no external callers, or all callers were updated within this PR.
{% endif %}
