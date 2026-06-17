# 07 Output Formats

## Output mode philosophy

The CLI must support both humans and scripts.

Default behavior should be readable, but every command must be automation-friendly through stable JSON output.

## Output modes

| Flag | Mode | Purpose |
|---|---|---|
| none | auto | Choose table for simple structures, pretty JSON for nested structures |
| `--json` | compact JSON | Machine-readable output |
| `--pretty` | pretty JSON | Human-readable complete response |
| `--raw` | raw body | Exact response body from API |
| `--table` | table | Human-readable table where possible |
| `--csv` | CSV | Export tabular data |

Only one explicit output mode should be allowed at a time. If multiple are passed, return CLI usage error.

## JSON output contract

`--json` should emit a stable wrapper by default unless `--body-only` is added later.

Recommended wrapper:

```json
{
  "service": "torn",
  "status": 200,
  "from_cache": false,
  "elapsed_ms": 148,
  "data": {}
}
```

For API error:

```json
{
  "service": "torn",
  "status": 200,
  "error": {
    "kind": "api_error",
    "code": "...",
    "message": "..."
  }
}
```

## Pretty JSON

`--pretty` should pretty-print the same wrapper or the raw parsed body. Pick one and keep it stable. Recommended: pretty wrapper, with a later `--body-only` option if needed.

## Raw output

`--raw` prints exactly the response body as returned by the API.

No wrapper. No table. No additional labels.

This is useful for piping to `jq` if the user does not want metadata.

## Table output

Table output should be best-effort.

Good candidates:

- arrays of objects
- flat objects
- selected known shortcuts, such as bars or config status

Poor candidates:

- deeply nested JSON
- arbitrary mixed arrays
- large object graphs

If table formatting is not possible, either:

1. fall back to pretty JSON in auto mode, or
2. return an error in explicit `--table` mode.

## CSV output

CSV output should only work for tabular data.

Rules:

- Include a header row.
- Flatten only one level by default.
- For nested fields, either JSON-encode the cell or require `--flatten` later.
- In explicit `--csv` mode, return an error if the response is not tabular.

Example:

```bash
torn api faction members --csv > members.csv
```

## Error output

Human-readable errors go to stderr.

Example:

```text
Error: Missing Torn API key

Set TORN_API_KEY in one of:
- .env in the current directory
- ~/.config/torn-cli/config.toml
- process environment
```

In `--json` mode, errors should be JSON and still go to stdout or stderr consistently. Recommended: stdout for command-result errors, stderr for CLI/runtime errors. Document this clearly.

JSON error example:

```json
{
  "error": {
    "kind": "missing_api_key",
    "service": "torn",
    "message": "Missing Torn API key"
  }
}
```

## Redaction in output

Secrets must be redacted in all modes except raw body, because raw body is direct API data. Since request URLs/headers are not part of raw body, keys should still not appear.

If FFScouter returns a key in a response, consider redacting by default unless `--unsafe-show-secrets` is explicitly added later. MVP should avoid such an unsafe flag.

## Known shortcut table examples

### `torn api user bars --table`

```text
Metric   Current   Maximum
Energy   150       150
Nerve    45        45
Happy    5000      5000
Life     3621      3621
```

### `torn config check`

```text
Item                 Status
TORN_API_KEY         present
FFSCOUTER_API_KEY    missing
Torn base URL        ok
FFScouter base URL   ok
Cache directory      writable
Endpoint index       loaded
```

## Output acceptance criteria

- [ ] `--json` returns parseable compact JSON.
- [ ] `--pretty` returns parseable pretty JSON.
- [ ] `--raw` returns only the response body.
- [ ] `--table` works for flat objects and arrays.
- [ ] `--csv` works for arrays of objects.
- [ ] Explicit impossible format returns a clear error.
- [ ] Auto mode gracefully falls back to pretty JSON.
- [ ] Secrets are redacted.
