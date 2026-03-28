## Change Classification — {{classifications|length}} symbols

Each changed symbol is classified by the nature of its change. Use the classifications to decide what scrutiny each symbol requires before reading diffs.

### Classification Reference

| Classification | Meaning |
|---------------|---------|
| **`api_surface_change`** | The symbol's signature changed (parameters, return type, or visibility). Any caller not updated in this PR may fail to compile or behave unexpectedly. |
| **`behavioral`** | The body changed but the signature is identical. Callers aren't syntactically broken, but the behavior they rely on may have changed silently. |
| **`refactor`** | Restructuring only — no observable behavioral change expected. |
| **`new`** | Symbol was added in this PR. |
| **`deleted`** | Symbol was removed in this PR. |

### Column Reference

| Column | Description |
|--------|-------------|
| **symbol** | Symbol name |
| **file** | File containing the symbol |
| **classification** | Change classification (see table above) |

**Classifications:**
{% toon classifications classifications %}

### Recommended Next Steps

#### 1. Verify API Surface Changes
For each `api_surface_change` in the Classifications table above, confirm which callers were NOT updated in this PR and get an explicit risk rating. Do this before flagging any breaking change — the risk rating determines whether to block the PR or note it as acceptable:
```
{{_cli}} review-reference-check --symbol "{symbol}" --file "{file}"
```

#### 2. Inspect Behavioral Changes
For each `behavioral` change, read the before/after body diff to determine whether the behavioral change is likely to affect callers silently. If the change alters return values, side effects, error handling, or control flow, treat it as high risk and run `review-body-safety` in step 3:
```
{{_cli}} review-symbol-diff --symbol "{symbol}" --file "{file}"
```

#### 3. Measure Cross-Cutting Behavioral Risk
If you flagged any behavioral changes as high risk in step 2, run this to surface all behavioral-change symbols whose callers are in unmodified files. Use the output to identify which symbols warrant a blocking review comment:
```
{{_cli}} review-body-safety
```
