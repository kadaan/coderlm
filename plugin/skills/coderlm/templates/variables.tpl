## Variables in {{function}} — {{variables|length}} bindings

The table below lists every local variable binding inside `{{function}}`. Use it to understand the data flow within the function before reading the full implementation.

{% if variables %}
### Column Reference

| Column | Description |
|--------|-------------|
| **name** | Variable identifier as it appears in source |
| **function** | Enclosing function name |

**Variables:**
{% toon variables variables %}

### Recommended Next Steps

#### 1. Read the Full Function
Load the complete source body of `{{function}}` to see how these variables are used in context:
```
{{_cli}} impl {{function}} --file {{file}}
```

#### 2. Find Callers
List every call site of `{{function}}` to understand how inputs arrive and what the caller does with the result:
```
{{_cli}} callers {{function}} --file {{file}}
```
{% else %}
No local variable bindings found in `{{function}}`. The function may be empty, or variable extraction is not supported for this language.
{% endif %}
