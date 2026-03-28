## {{symbol}} in {{file}} ({{kind}}, lines {{line_range}})

The source below is the full body of `{{symbol}}` as it exists in the indexed snapshot.

```
{{source}}
```

### Recommended Next Steps

#### 1. Find Callers
List every call site of `{{symbol}}` in the codebase. Use this to understand how and where this symbol is used before making any changes:
```
{{_cli}} callers {{symbol}} --file {{file}}
```

#### 2. Find Tests
List all test references to `{{symbol}}`. Use this to understand what behavior is already covered before assessing risk:
```
{{_cli}} tests {{symbol}} --file {{file}}
```

#### 3. List Local Variables
List the local variable bindings inside `{{symbol}}`. Use this to understand the data flow within the function body:
```
{{_cli}} variables {{symbol}} --file {{file}}
```
