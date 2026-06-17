# API Coverage

`torn-cli` is designed to cover the official Torn API v2 and FFScouter without requiring every endpoint to be hand-coded as a bespoke command.

## Coverage strategy

1. **Generic path access is the compatibility floor.** Users should be able to call any documented Torn v2 path with `torn api get <path>` and any supported FFScouter path with `torn ff get <path>`.
2. **Endpoint index powers discovery.** A bundled endpoint index should list services, groups, methods, paths, parameters, and auth requirements.
3. **Shortcuts are thin wrappers.** Commands like `torn api user basic` should delegate to the same request model as `torn api get /user/basic`.
4. **OpenAPI refresh is reviewable.** Torn endpoint metadata should be generated from `https://www.torn.com/swagger/openapi.json` with a custom User-Agent and pinned in the repository so schema diffs are reviewed.

## Torn API v2

Target base URL:

```text
https://api.torn.com/v2
```

Auth:

```http
Authorization: ApiKey <redacted>
```

Repository research observed Torn OpenAPI 3.1.0, schema version 6.0.0, and 205 documented `GET` paths. The endpoint index should cover all documented v2 paths across these groups:

| Group | Coverage intent |
|---|---|
| `user` | profile, bars, battlestats, inventory, messages, events, travel, races, reports, logs, and related user selections |
| `faction` | basic faction data, members, attacks, chains, crimes, revives, reports, territories, wars, applications, and ranked wars |
| `torn` | global reference data such as items, stocks, honors, medals, crimes, education, properties, log categories, and attack logs |
| `company` | company profile, employees, applications, news, stock, search, snapshots, and lookup/timestamp endpoints |
| `market` | item market, bazaar, auction house, properties, rentals, lookup, and timestamp endpoints |
| `racing` | cars, tracks, races, records, upgrades, lookup, and timestamp endpoints |
| `forum` | categories, threads, posts, lookup, and timestamp endpoints |
| `property` | property details, lookup, and timestamp endpoints |
| `key` | API key info and log endpoints |

Generic examples:

```bash
torn api get /user/basic --pretty
torn api get /user --param selections=basic,bars --json
torn api get /faction/members --param striptags=false --table
torn api get /market/itemmarket --param id=206 --json
torn api get /key/info --pretty
```

Dedicated log helpers cover the full-access `/user/log` endpoint plus public log metadata endpoints:

```bash
torn logs fetch --since 24h --to now --limit 100 --pretty
torn logs analyze --since 7d --group-by category --table
torn logs analyze --since 30d --group-by data-key --json
torn logs catalog --pretty
```

See [`TORN_LOGS.md`](TORN_LOGS.md) for researched field structure, filtering, grouping, and privacy caveats.

OpenAPI arrays use comma-separated form-style serialization, so flags like `--param selections=basic,bars` should preserve comma-separated values.

## FFScouter

Target base URL:

```text
https://ffscouter.com/api/v1
```

Auth uses a query parameter:

```text
key=<redacted>
```

Generic and shortcut coverage includes:

```bash
torn ff check-key --pretty
torn ff status --pretty
torn ff register --agree-to-data-policy
torn ff stats --target 123456,789012 --json
torn ff stats-history --target 123456 --since 30d --limit 20 --json
torn ff flights --target 123456 --pretty
torn ff activity player --target 123456 --since 24h --bucket 900 --json
torn ff activity faction --faction 89 --since 24h --bucket 3600 --json
torn ff hits claims --pretty
torn ff hits claim --target 123456 --yes
torn ff hits unclaim --target 123456 --yes
torn ff hits wipe --yes
torn ff targets --preset respect --limit 25 --json
torn ff targets --min-level 20 --max-ff 2.5 --factionless --json
torn ff losses quote --quantity 10 --price-per-loss 300000
torn ff losses seller-contracts --pretty
torn ff losses seller-claims --pretty
torn ff losses seller-order --order 12345 --pretty
torn ff losses seller-claim --order 12345 --slots 10 --yes
torn ff losses seller-complete --claim-id <id> --yes
torn ff announcements --pretty
```

Because FFScouter credentials are query parameters, all displayed/logged URLs and cache keys must remove or redact `key`. Response bodies are also recursively redacted for configured secrets because `/check-key` and `/register` can echo the API key.

## Endpoint index files

The design-doc seed index is available at:

```text
docs/torn-cli-design-docs/endpoint-index-seed.json
```

Future generated indexes should be deterministic and reviewable. Suggested generated metadata per endpoint:

- service (`torn` or `ffscouter`)
- method
- path
- group/tag
- summary/description
- path parameters
- query parameters
- auth behavior
- known access level or permission notes
- cache hints when known

## Rate limits and errors

Implementation should account for:

- Torn's documented 100 requests/minute per-user limit across all keys.
- Torn service-cache behavior, often around 30 seconds.
- Torn API error payloads that may arrive with HTTP 200.
- Access-level failures for private selections and faction/company endpoints.
- Invalid-key behavior that should stop retries rather than hammering the service.

## Acceptance checks

When the implementation is complete, coverage checks should include:

- [ ] endpoint index count matches the pinned Torn OpenAPI snapshot
- [ ] every indexed operation is reachable through generic path access
- [ ] shortcuts delegate to generic request construction
- [ ] schema-aware `--pretty` summaries cover common response wrapper families and fall back safely for uncommon schemas
- [ ] `--watch` repeats GET requests with cache bypass and colored time prefixes
- [ ] path query strings and repeated `--param` flags merge deterministically
- [ ] Torn auth is attached as a header
- [ ] FFScouter auth is attached as a query parameter and redacted in display
- [ ] cache keys exclude auth secrets
- [ ] full-access `/user/log` helpers expose `from`/`to`, log id, category, target, grouping, field inventory, and safe JSON/CSV/table output
- [ ] log presets cover every observed Torn log category and support user-defined TOML presets
- [ ] permission preflight uses `/key/info` plus endpoint-index access metadata before known Torn requests
