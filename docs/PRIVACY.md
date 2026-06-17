# Privacy and Security Model

`torn-cli` is a local API client for Torn API v2 and FFScouter. API responses can contain private player, faction, company, market, travel, message, event, and key-access data. The default posture is local-only processing and aggressive secret redaction.

## Data handled by the tool

The CLI/TUI may process:

- Torn API keys and FFScouter API keys.
- Request paths, query parameters, and optional request bodies.
- Torn and FFScouter response bodies.
- Local configuration, endpoint metadata, saved request templates, and cache entries.

## Data not collected by the project

The project must not add telemetry by default. In particular, the tool should not automatically upload any of the following to third-party services:

- API keys or derived credentials.
- Request URLs, request bodies, or response bodies.
- Cache contents or saved request files.
- Player, faction, company, or account data.
- Crash reports or debug traces containing API data.

If an opt-in diagnostic upload feature is ever proposed, it must be documented, disabled by default, and reviewed for redaction first.

## Secret storage

Recommended local key storage is the private config file managed by:

```bash
torn config set torn-api-key
torn config set ffscouter-api-key
torn config tui
```

`config set` uses a hidden prompt by default. `config tui` masks typed values and never displays existing keys. The TUI can display `/key/info` metadata such as access type, selection names, and owner/faction/company ids so you can verify what the key can do; it still never displays the API key value. User-created log presets are also stored in `config.toml`; they are not secrets, but the file remains private because it may contain API keys. On Unix, the config writer sets the parent directory to `0700` and `config.toml` to `0600`. This is local plaintext storage protected by OS file permissions, not a cloud key vault.

Ignored `.env` files and process environment variables remain supported for development/CI:

```env
TORN_API_KEY=your_torn_api_key_here
FFSCOUTER_API_KEY=your_ffscouter_api_key_here
```

Rules:

- The repository includes `.env.example`, never a real `.env`.
- `.env`, `.env.*`, local cache files, and local databases are ignored by git.
- Real keys must not appear in docs, fixtures, tests, issues, logs, screenshots, or pull requests.
- Prefer API keys with the minimum Torn access level required for the test. Use full-access Torn keys only for commands that require them, such as `torn logs fetch/analyze`.

## Auth handling

### Torn

Use header auth:

```http
Authorization: ApiKey <redacted>
```

Torn API keys must not be placed in URLs. URL query auth is easier to leak through shell history, proxy logs, terminal output, saved requests, and cache keys.

### FFScouter

FFScouter uses query-string auth:

```text
key=<redacted>
```

Because this secret is part of the request URL, all URL formatting should go through one sanitizer that redacts `key`, regardless of case and parameter order. FFScouter responses can also echo the key (`/check-key`, `/register`), so response JSON/text is recursively redacted for configured secrets before rendering.

## Redaction requirements

Redact secrets in:

- config display, including `torn config show`
- verbose/debug logs
- error messages
- saved request listings
- cache metadata and cache inspection
- TUI config and request screens
- FFScouter response bodies that echo configured keys
- generated crash/debug files
- tests and snapshots

Recommended display forms:

```text
short secrets: <redacted>
long secrets: <redacted>
headers: Authorization: ApiKey <redacted>
query: key=<redacted>
```

## Cache and saved requests

Cache keys must be deterministic but secret-free. Inputs can include service, method, normalized path, sorted non-secret query params, and request body hash. Inputs must not include:

- `TORN_API_KEY`
- `FFSCOUTER_API_KEY`
- `Authorization` header value
- `key` auth query parameter value

Saved requests may store service, method, path, query params, optional body, and cache preferences. They must not store resolved full URLs containing auth parameters or any API key value.

## Torn user logs

`torn logs fetch` and `torn logs analyze` require a full-access Torn key and may return highly private account activity. The CLI processes this locally and does not upload it anywhere, but stdout can still be captured by shells, terminals, CI logs, and files you redirect to.

Guidelines:

- Prefer `--table` summaries for quick inspection.
- Use `--include-raw` only when you intentionally need full filtered log entries in JSON output.
- Treat exported CSV/JSON logs as private data.
- Do not paste real log payloads into issues; share only schemas/key names or redacted samples.

## Logs and errors

Default output should be quiet. Verbose mode is allowed for debugging but must remain redacted.

Acceptable:

```text
GET https://api.torn.com/v2/user/basic
Authorization: ApiKey <redacted>
```

Unacceptable:

```text
Authorization: ApiKey <example-secret>
```

When Torn returns invalid/disabled/paused key errors, do not retry that key aggressively because invalid-key bursts can temporarily IP-ban clients.

## Security checklist for changes

Before merging privacy-sensitive code, verify:

- [ ] Torn auth uses `Authorization: ApiKey` and not URL query auth.
- [ ] FFScouter URL display redacts `key=`.
- [ ] Config precedence and `config set` are tested without printing secret values.
- [ ] Cache keys and metadata exclude secrets.
- [ ] Saved requests exclude resolved auth secrets.
- [ ] Errors, logs, TUI screens, and snapshots redact secrets.
- [ ] Tests use placeholder keys only.
- [ ] Optional online tests are skipped unless explicitly enabled.
