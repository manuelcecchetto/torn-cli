# 06 Config, Cache, and Security

## Configuration sources

Configuration should be loaded in this order, with later entries overriding earlier ones:

1. Built-in defaults
2. Config file, e.g. `~/.config/torn-cli/config.toml`
3. `.env` file in current directory, unless `--no-env`
4. File passed through `--env-file`
5. Process environment variables
6. CLI flags

## Environment variables

```env
TORN_API_KEY=
FFSCOUTER_API_KEY=

TORN_BASE_URL=https://api.torn.com/v2
FFSCOUTER_BASE_URL=https://ffscouter.com/api/v1

TORN_CACHE_DIR=
TORN_CONFIG_DIR=
TORN_API_INDEX_PATH=
```

## Example config file

```toml
[torn]
base_url = "https://api.torn.com/v2"

[ffscouter]
base_url = "https://ffscouter.com/api/v1"

[cache]
enabled = true
default_ttl_seconds = 30

[output]
default_mode = "auto"
```

API keys may be supported in the config file, but the recommended path is environment variables or `.env`.

## `.env.example`

The repository should include `.env.example`, never a real `.env`.

```env
TORN_API_KEY=your_torn_api_key_here
FFSCOUTER_API_KEY=your_ffscouter_api_key_here
TORN_BASE_URL=https://api.torn.com/v2
FFSCOUTER_BASE_URL=https://ffscouter.com/api/v1
```

## Config commands

```bash
torn config check
torn config path
torn config show --redacted
```

### `torn config check`

Should verify:

- Torn API key exists, or report missing
- FFScouter API key exists, or report missing
- base URLs are valid URLs
- cache directory is writable
- endpoint index is loadable

It should not validate the remote APIs unless passed `--online`.

```bash
torn config check --online
```

## Secret redaction

The tool must redact secrets everywhere:

- logs
- debug output
- config display
- saved requests
- errors
- TUI config screen
- cache metadata

Redaction examples:

```text
abc123456789 -> <redacted>
shortsecret -> <redacted>
```

Never print full keys.

## Auth handling

### Torn

Use an HTTP header:

```http
Authorization: ApiKey <TORN_API_KEY>
```

Do not place Torn keys in URLs.

### FFScouter

Use query parameter auth:

```text
key=<FFSCOUTER_API_KEY>
```

Because this puts a key in the URL, all displayed URLs must be sanitized.

## Cache design

MVP can use SQLite or filesystem JSON. Recommended: SQLite through `rusqlite`.

### Cache table

```sql
CREATE TABLE IF NOT EXISTS responses (
  cache_key TEXT PRIMARY KEY,
  service TEXT NOT NULL,
  method TEXT NOT NULL,
  path TEXT NOT NULL,
  query_hash TEXT NOT NULL,
  body_hash TEXT,
  status INTEGER NOT NULL,
  response_headers TEXT,
  response_body TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL
);
```

### Cache key

Cache key should be deterministic and must not include raw secrets.

Input:

- service
- method
- normalized path
- sorted query params excluding auth key
- request body hash
- output-affecting flags only if they affect network response

Pseudo-code:

```text
sha256(service + method + path + sorted_query_without_secrets + body_hash)
```

## Cache policy

```rust
pub enum CachePolicy {
    Default,
    Disabled,
    Fresh,
    Ttl(Duration),
}
```

Meaning:

| Policy | Behavior |
|---|---|
| Default | Use default TTL for GET requests |
| Disabled | Do not read or write cache |
| Fresh | Skip read, perform network request, update cache |
| Ttl | Use custom TTL |

POST should not be cached unless explicitly enabled later.

## Filesystem locations

Use `directories` crate.

Linux examples:

```text
Config: ~/.config/torn-cli/config.toml
Cache:  ~/.cache/torn-cli/cache.sqlite
Data:   ~/.local/share/torn-cli/
```

## Saved requests security

Saved requests must contain:

- service
- method
- path
- query params
- optional JSON body
- cache preference

Saved requests must not contain:

- Torn API key
- FFScouter API key
- resolved full URLs with secret query params

## Logging

If adding logs, default to quiet. Verbose mode should still redact secrets.

```bash
torn -v api get /user/basic
```

Acceptable verbose output:

```text
GET https://api.torn.com/v2/user/basic
Authorization: ApiKey <redacted>
```

Unacceptable:

```text
Authorization: ApiKey <example-secret>
```

## Privacy model

Torn and FFScouter responses can contain private player/faction data. Avoid automatic telemetry. Do not send request/response data to any third-party service.

## Security acceptance criteria

- [ ] Real keys are never printed by normal commands.
- [ ] Real keys are never stored in saved requests.
- [ ] Cache keys exclude secrets.
- [ ] Displayed FFScouter URLs redact `key=`.
- [ ] `config show` redacts secrets.
- [ ] Tests cover redaction.
