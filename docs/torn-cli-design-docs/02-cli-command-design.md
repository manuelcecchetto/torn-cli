# 02 CLI Command Design

## Top-level command shape

```bash
torn [global-options] <command> [command-options]
```

## Global options

```bash
-c, --config <path>        Config file path
--env-file <path>          Load environment variables from a specific .env file
--no-env                   Do not load .env
--cache-dir <path>         Override cache directory
--no-cache                 Disable cache for this request
--fresh                    Bypass cache and force network request
--cache-ttl <duration>     Override cache TTL, e.g. 30s, 5m, 1h
--json                     Emit compact JSON
--pretty                   Emit pretty JSON
--raw                      Emit raw response body
--table                    Emit table output where possible
--csv                      Emit CSV output where possible
--quiet                    Suppress non-essential logs
-v, --verbose              Verbose logs without secrets
-h, --help                 Show help
-V, --version              Show version
```

## Command tree

```text
torn
├── config
│   ├── check
│   ├── path
│   └── show --redacted
├── endpoints
│   ├── --service torn|ff|all
│   └── search <query>
├── api
│   ├── get <path>
│   ├── post <path>
│   ├── user
│   │   ├── basic
│   │   ├── bars
│   │   ├── inventory
│   │   └── lookup
│   ├── faction
│   │   ├── basic
│   │   ├── members
│   │   └── attacksfull
│   ├── torn
│   │   ├── items
│   │   └── stocks
│   ├── market
│   │   └── itemmarket
│   └── key
│       └── info
├── ff
│   ├── get <path>
│   ├── check-key
│   ├── stats
│   ├── stats-history
│   ├── flights
│   ├── targets
│   └── announcements
├── saved
│   ├── list
│   ├── add <name> <request>
│   ├── run <name>
│   └── remove <name>
├── cache
│   ├── status
│   ├── clear
│   └── inspect <key>
└── tui
```

## Generic Torn requests

The generic path command must be flexible enough to reach new endpoints before shortcuts exist.

```bash
torn api get /user/basic
torn api get '/user?selections=basic,bars'
torn api get /faction/members --param striptags=false
torn api get /market/itemmarket --param id=206
torn api post /some/path --body-file payload.json
```

### Query param syntax

Support both direct query string and repeated `--param`:

```bash
torn api get '/user?selections=basic,bars'
torn api get /user --param selections=basic,bars
```

If both are present, merge them, with explicit `--param` taking precedence.

## Generic FFScouter requests

```bash
torn ff get /check-key
torn ff get /get-stats --param user_id=3747263
torn ff get /player-flights --param user_id=3747263
torn ff get /announcements
```

The FFScouter key should be automatically added as a query parameter unless disabled.

## Shortcut commands

Shortcut commands are thin wrappers over generic requests.

Examples:

```bash
torn api user basic
# Equivalent to: torn api get /user/basic


torn api user bars
# Equivalent to: torn api get '/user?selections=bars'


torn ff check-key
# Equivalent to: torn ff get /check-key
```

## Saved requests

Saved requests are local named request templates.

```bash
torn saved add my-bars 'api get /user --param selections=bars'
torn saved run my-bars --pretty
torn saved list
torn saved remove my-bars
```

Saved requests must not store API keys.

## Exit codes

| Code | Meaning |
|---:|---|
| 0 | Success |
| 1 | Generic runtime error |
| 2 | Invalid CLI usage |
| 3 | Missing configuration or API key |
| 4 | Authentication/authorization failure |
| 5 | API returned an error payload |
| 6 | Network error |
| 7 | Cache error |
| 8 | Output formatting/export error |

## Command examples

```bash
# Validate local setup
torn config check

# Discover endpoints
torn endpoints --service torn
torn endpoints search attacks

# Torn API
torn api get /user/basic --pretty
torn api get /user --param selections=basic,bars --json
torn api faction members --table
torn api torn items --json > items.json

# FFScouter
torn ff check-key --pretty
torn ff stats --user 3747263 --pretty
torn ff flights --user 3747263 --csv > flights.csv

# TUI
torn tui
```
