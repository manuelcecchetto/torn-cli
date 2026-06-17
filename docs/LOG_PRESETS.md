# Torn log presets

`torn logs presets` turns the raw `/user/log` stream into reusable investigations.

The built-in preset map was built from the live Torn v2 log catalog (`/torn/logcategories` and `/torn/logtypes`). At implementation time the catalog contained **228 categories** and **1,147 log types**. The built-in preset test asserts that every observed category id is covered by at least one themed preset.

## Commands

```bash
torn logs presets list
torn logs presets show security --pretty
torn logs presets run security --since 30d --group-by type
torn logs presets run money --since 90d --csv
torn logs presets run api-keys --limit 25 --pretty
```

Create your own preset:

```bash
torn logs presets add big-cash \
  --description "large cash-related events" \
  --cat 13 --cat 14 --cat 17 --cat 59 --cat 112 \
  --contains money \
  --since 30d \
  --group-by type

torn logs presets run big-cash --since 7d
torn logs presets remove big-cash
```

User presets are stored in `config.toml` under `[logs.presets.<name>]`. Names are normalized to lowercase and may use letters, numbers, dash, underscore, or dot. A user preset can shadow a built-in preset by passing `--force` to `add`.

## Run behavior

- Presets use the same permission preflight as `torn logs fetch` and `torn logs analyze`.
- A preset with one category calls `/user/log?cat=<id>`.
- A preset with multiple categories fans out one bounded request per category, then de-duplicates by log entry id.
- `--limit` and `--max-pages` apply per category in multi-category presets.
- `--log`, `--cat`, `--contains`, `--data-key`, and `--param-key` passed at run time are merged with the preset definition.
- Output supports the global `--json`, `--pretty`, `--table`, and `--csv` modes.

## Built-in presets

| Preset | Default window | Categories | Purpose |
|---|---:|---:|---|
| `all` | `24h` | 0 | Discovery across recent logs without category filtering. |
| `security` | `30d` | 9 | Authentication, API keys, preferences, captcha, staff/reporting, recovery/closure. |
| `api-keys` | `90d` | 1 | API key creation/edit/delete audit events. |
| `account-lifecycle` | `30d` | 22 | Account/profile lifecycle, donator, referrals, preferences, newsletters. |
| `communications-social` | `14d` | 21 | Friends, enemies, ignores, messages, events, forums, lists, newspaper/social content. |
| `money` | `30d` | 13 | Cash, bank, checks, loans, vault, piggy bank, offshore bank, faction payouts. |
| `points-credits-tokens` | `30d` | 19 | Points, credits, refills, donator, tokens, token shop, bunker bucks. |
| `market-trading` | `14d` | 14 | Item market, bazaar, parcels, auctions, trades, stocks, points market, property rental. |
| `items` | `14d` | 41 | Item movement/use families, ammo/mods/equipment, shops, dump, museum, city finds, relics. |
| `combat` | `7d` | 30 | Attacks, hospital/jail/life/radiation, bounties, revives, ammo/mods/equipping, targets. |
| `faction-war` | `14d` | 14 | Faction activity, respect, dirty bombs, NAPs/treaties, OCs, territory war, payouts. |
| `travel-property` | `30d` | 11 | Travel, property, rentals, estate agents, upkeep, display case, bunker. |
| `company-job` | `14d` | 11 | Company, company specials, jobs, job points, applications, working stats. |
| `progression-training` | `7d` | 37 | Energy/nerve/happy/life/stat changes, education, merits, gym, addiction, skills, hunting. |
| `crimes` | `14d` | 9 | Crimes, viruses, organized crimes, success/failure/critical failure outcomes. |
| `casino-gambling` | `14d` | 15 | Casino tokens and game-specific casino logs. |
| `racing` | `30d` | 4 | Racing and racing points in/out. |
| `competitions-seasonal` | `90d` | 23 | Awards, missions, seasonal events, competitions, articles/headlines, relics/keepsakes. |
| `staff-moderation` | `90d` | 4 | Staff, reporting, account closure/recovery, moderation-adjacent audit events. |

## TOML shape

```toml
[logs.presets.big-cash]
description = "large cash-related events"
categories = ["13", "14", "17", "59", "112"]
contains = ["money"]
group_by = "type"
since = "30d"
limit = 100
max_pages = 1
```

Available fields:

- `description`
- `categories`
- `log_ids`
- `contains`
- `data_keys`
- `param_keys`
- `group_by`: `category`, `type`, `day`, `hour`, `target`, `data-key`, `param-key`
- `since`, `to`, `target`
- `limit`, `max_pages`
