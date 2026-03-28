## Tests for {{symbol}} — {{tests|length}} found

The table below lists every test that references `{{symbol}}` by name. Use it to understand what behavior is covered before reading the implementation or assessing change risk.

{% if tests %}
### Column Reference

| Column | Description |
|--------|-------------|
| **name** | Test function name |
| **file** | File containing the test |
| **line** | Line number where the test function begins |

**Tests:**
{% toon tests tests %}

### Recommended Next Steps

#### 1. Read Test Implementations
Load the full source body of each test to see what it asserts. Use this to understand what behavior is verified and whether the existing tests reflect the current implementation. For each row in `<data id="tests">`:
```
{{_cli}} impl "{name}" --file "{file}"
```
{% else %}
No tests found referencing `{{symbol}}`. The symbol has no test coverage, or its tests use it indirectly without referencing it by name.
{% endif %}
