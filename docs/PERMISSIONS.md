# Torn key permissions

`torn-cli` resolves Torn key capabilities before making known Torn API requests. This avoids predictable API error 16 (`Access level of this key is not high enough`) and prevents needless calls with invalid permissions.

## Sources

- Official Torn API docs: <https://www.torn.com/api.html>
  - Torn defines predefined access levels: Public, Minimal Access, Limited Access, and Full Access.
  - Custom keys can grant exact selections and should be treated with the same care as high-access keys.
  - Invalid or access-too-low retries should not be hammered because Torn can temporarily block abusive traffic.
- Official OpenAPI: <https://www.torn.com/swagger/openapi.json>
  - `ApiKeyAccessTypeEnum`: `Custom`, `Public Only`, `Minimal Access`, `Limited Access`, `Full Access`.
  - `/key/info` is available for any valid key and returns `info.access`, `info.selections`, owner ids, faction/company flags, and custom log permissions.
  - Endpoint metadata contains per-selection access levels used by the bundled endpoint index.
- Human-readable cross-check: <https://tornapi.tornplayground.eu/key/info>

## CLI behavior

Before known Torn requests, the CLI:

1. fetches `/key/info` with the configured Torn key;
2. resolves the target path/query into one or more selections using `assets/endpoint-index.json`;
3. compares the request against the key type and selection list;
4. exits before the target request if the key cannot satisfy it.

Examples:

```bash
torn config check --online       # includes permission summary
torn config permissions          # only permission summary
torn api user basic              # preflight allows if key can read user/basic
torn logs fetch --limit 1        # preflight denies unless full/custom log permission exists
```

Preflight is skipped for `--no-auth`, FFScouter, and `/key/info` itself.

## Predefined keys

Predefined keys are additive:

| Key type | CLI interpretation |
|---|---|
| Public Only | Can call indexed selections requiring Public. |
| Minimal Access | Public + Minimal. |
| Limited Access | Public + Minimal + Limited. |
| Full Access | Public + Minimal + Limited + Full. Required for broad `/user/log`. |

## Custom keys

Custom keys are evaluated by exact `/key/info` selection lists:

```text
info.selections.user = ["basic", "profile", ...]
info.selections.faction = ["basic", "members", ...]
```

A custom key can call a known selection only when that group/selection appears in the key info response. Unknown future selections are allowed through only when torn-cli cannot prove denial; known missing selections are denied locally.

## Custom log permissions

`/key/info` also exposes:

```text
info.access.log.custom_permissions
info.access.log.available[] = { category_id, log_ids[] }
```

For custom keys with log-specific permissions, `torn-cli` verifies `/user/log` only when the request includes one of:

- `--log <id[,id]>`, where every requested id is listed in `available[].log_ids`; or
- `--cat <id>`, where the category id is listed in `available[].category_id`.

If the custom key has limited log permissions and the request omits both `--log` and `--cat`, the CLI denies locally and asks for a narrower filter.

## TUI behavior

`torn config tui` shows a local permission summary fetched from `/key/info`:

- access type and level;
- owner/faction/company ids reported by Torn;
- faction/company access flags;
- total selection count;
- selection names grouped by Torn API section;
- custom log permission summary when present.

The TUI never displays the API key value itself. On Unix, saved config files are written with private permissions.
