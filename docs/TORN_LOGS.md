# Torn logs support

`torn logs` is built around Torn API v2 log endpoints and the current OpenAPI schema. Before calling `/user/log` or log catalog endpoints, the CLI fetches `/key/info` and fails locally if the configured key cannot access the requested selection/filter.

## Research sources

- Official OpenAPI: <https://www.torn.com/swagger/openapi.json>
  - `/user/log` is `getMyLogs`, requires a full-access key, and supports `log`, `cat`, `target`, `limit`, `from`, and `to` query parameters.
  - `UserLog` has `id`, `timestamp`, `details`, `data`, and `params` fields. `details` contains the log type id, title, and category. `data` and `params` are dynamic objects whose keys depend on the log type.
  - `/torn/logcategories`, `/torn/logtypes`, and `/torn/{logCategoryId}/logtypes` expose public log metadata for building a catalog.
- Unofficial TornAPI playground pages, useful as human-readable cross-checks:
  - <https://tornapi.tornplayground.eu/user/log>
  - <https://tornapi.tornplayground.eu/torn/logtypes>
  - <https://tornapi.tornplayground.eu/torn/logcategories>
- Generated client model cross-check: <https://neon0404.github.io/torn-client/types/UserLog.html>

## Commands

```bash
torn logs fetch --since 24h --to now --limit 100 --pretty
torn logs analyze --since 7d --to now --group-by category --table
torn logs analyze --since 30d --group-by type --contains xanax --data-key item --json
torn logs analyze --since 2026-01-01 --to 2026-01-31 --group-by day --csv
torn logs catalog --pretty
torn logs catalog --cat 1 --table
torn logs types --json
torn logs categories --table

torn logs presets list
torn logs presets show security --pretty
torn logs presets run security --since 30d --group-by type
torn logs presets add big-cash --cat 13 --cat 14 --cat 17 --contains money --since 30d --group-by type
```

See [`LOG_PRESETS.md`](LOG_PRESETS.md) for the full built-in preset map and custom preset TOML shape. Built-ins cover all 228 categories observed in the current Torn log catalog.

## Filtering model

Server-side filters sent to Torn:

- `--since` / `--from` -> `from`
- `--to` -> `to`
- `--log <id[,id]>` -> `log`
- `--cat <id>` / `--category <id>` -> `cat`
- `--target <user_id>` -> `target`
- `--limit <n>` for page size
- `--max-pages <n>` to cap pagination manually. If omitted, `--since`-bounded windows auto-page until Torn returns no continuation link, while queries without a lower bound fetch one page by default.

For `/user/log`, Torn exposes older pages through `_metadata.links.prev` for normal newest-first windows. `torn-cli` follows that continuation and deduplicates boundary log ids across pages.

Client-side filters applied after fetch:

- `--contains <text>` searches id, type id, title, category, data JSON, and params JSON.
- `--data-key <key>` keeps logs whose `data` object contains the key.
- `--param-key <key>` keeps logs whose `params` object contains the key.

## Grouping model

`--group-by` accepts:

- `category`
- `type`
- `day`
- `hour`
- `target` (best-effort extraction from common `target`, `target_id`, `user`, `user_id`, `player`, `player_id` fields in `params` then `data`)
- `data-key`
- `param-key`

Analysis output includes:

- total fetched logs and filtered logs
- pagination metadata: pages fetched, resolved max pages, whether results were truncated, and which continuation direction was followed
- group count, first/last timestamps, categories, log type ids, data keys, params keys, and example ids
- observed field shapes per log type: all seen `data` keys, `params` keys, and JSON value types
- optional `--include-raw` raw filtered logs in JSON output

## Privacy notes

`/user/log` requires a full-access Torn key and can reveal private account activity. The CLI does not upload these logs, but stdout/CSV/JSON exports are still sensitive. Do not paste real payloads into public issues; share only redacted samples or field-key inventories.
