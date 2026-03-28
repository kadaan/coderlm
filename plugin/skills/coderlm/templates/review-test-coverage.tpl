## Test Coverage

Coverage is determined by symbol-name reference: a symbol is **covered** if at least one test file references its name, and **uncovered** if no test file does. This is a heuristic — it does not verify what the test actually asserts.

{% if covered %}
### Covered ({{covered|length}} symbols with tests)

These changed symbols have at least one test referencing them. Verify that existing tests still accurately reflect the new behavior — a passing test that no longer tests the right thing is as bad as no test at all.

| Column | Description |
|--------|-------------|
| **symbol** | Symbol name |
| **file** | File containing the symbol |
| **test_count** | Number of unique tests referencing this symbol |

**Covered Symbols:**
{% toon covered covered %}
{% endif %}

{% if uncovered %}
### Uncovered ({{uncovered|length}} symbols without tests)

These symbols were added or modified in this PR with no test references found anywhere in the codebase. Shipping them without tests means regressions will go undetected.

| Column | Description |
|--------|-------------|
| **symbol** | Symbol name |
| **file** | File containing the symbol |
| **kind** | Symbol type (function, method, class, etc.) |

**Uncovered Symbols:**
{% toon uncovered uncovered %}

### Recommended Next Steps

#### 1. Prioritize by Impact
For each uncovered symbol, check how widely it is called — a symbol with many callers and no tests is a higher-priority gap than a leaf function called once. Use the caller count to decide which gaps to flag as blocking vs. informational:
```
{{_cli}} review-impact "{symbol}" --file "{file}"
```

{% endif %}

{% if not uncovered and not covered %}
No added or modified symbols found in this PR's diff.
{% endif %}
