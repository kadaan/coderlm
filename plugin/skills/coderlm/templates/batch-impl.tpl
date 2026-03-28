## Implementations — {{results|length}} found{% if errors %}, {{errors|length}} errors{% endif %}

Each section below contains the full source body of one requested symbol. Use it to read multiple implementations in a single call before deciding which ones to investigate further.

{% for r in results %}
### {{r.symbol}} in {{r.file}} (lines {{r.line_range}})
```
{{r.source}}
```
{% endfor %}

{% if errors %}
### Errors

The following symbols could not be resolved:

{% for e in errors %}
- **{{e.symbol}}** in `{{e.file}}`: {{e.error}}
{% endfor %}
{% endif %}

### Recommended Next Steps

#### 1. Find Callers of a Symbol
For any symbol whose usage you need to understand, list every call site in the codebase:
```
{{_cli}} callers SYMBOL --file FILE
```

#### 2. Find Tests for a Symbol
For any symbol whose coverage you need to verify, list all test references:
```
{{_cli}} tests SYMBOL --file FILE
```
