## Symbol Diff: {{symbol}} in {{file}} ({{kind}}, {{change}})

{% if signature_changed %}
**Signature changed** — the public interface of this symbol was modified. Any caller NOT in this PR may now fail to compile or behave unexpectedly.

| | Signature |
|---|---|
| **Before** | `{{old_signature}}` |
| **After** | `{{new_signature}}` |
{% endif %}

{% if diff %}
### Unified Diff
```diff
{{diff}}
```
{% endif %}

{% if base_source %}
### Base Source (lines {{base_line_range}})
```
{{base_source}}
```
{% endif %}

{% if head_source %}
### Head Source (lines {{head_line_range}})
```
{{head_source}}
```
{% endif %}

### Recommended Next Steps

#### 1. Assess Caller Risk
Lists all callers of `{{symbol}}` that were **not** updated in this PR.{% if signature_changed %} **Run this** — the signature changed, so un-updated callers are likely breakage points that warrant a blocking review comment.{% else %} Run this if the behavioral change could affect callers silently — altered return values, side effects, error handling, or control flow are signals to proceed:{% endif %}
```
{{_cli}} review-impact --symbol {{symbol}} --file {{file}}
```

#### 2. Get a Detailed Caller Breakdown
For each caller surfaced in step 1, get a full breakdown of updated vs. un-updated callers with an explicit risk rating. Use the risk rating to determine whether to escalate to a blocking review comment:
```
{{_cli}} review-reference-check --symbol {{symbol}} --file {{file}}
```

#### 3. Browse the Full Call Graph
Queries every call site of `{{symbol}}` across the entire codebase regardless of PR membership. Use this if step 2 surfaces unexpected callers and you need broader context on how this symbol is used:
```
{{_cli}} callers {{symbol}} --file {{file}}
```
