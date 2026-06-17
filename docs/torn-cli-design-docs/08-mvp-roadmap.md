# 08 MVP Roadmap

## Phase 0: Project skeleton

Deliverables:

- Rust crate initialized
- `cargo fmt`, `cargo clippy`, `cargo test` configured
- `clap` CLI skeleton
- `.env.example`
- README with setup instructions

Acceptance:

```bash
cargo run -- --help
cargo test
```

## Phase 1: Config and secret handling

Deliverables:

- Config loader
- `.env` loading
- base URL defaults
- secret redaction utility
- `torn config check`
- `torn config show --redacted`

Acceptance:

```bash
torn config check
torn config show --redacted
```

Tests:

- missing keys produce clear errors
- redaction works
- config precedence works

## Phase 2: Generic API request engine

Deliverables:

- `ApiRequest` and `ApiResponse` models
- `ApiClient`
- Torn auth header support
- FFScouter query auth support
- generic GET command for Torn
- generic GET command for FFScouter

Acceptance:

```bash
torn api get /user/basic --pretty
torn api get /user --param selections=basic,bars --json
torn ff get /check-key --pretty
```

Tests:

- Torn key sent as `Authorization: ApiKey ...`
- FFScouter key sent as query param
- logged/displayed URLs redact FFScouter key
- network errors map to app errors

## Phase 3: Output modes

Deliverables:

- compact JSON
- pretty JSON
- raw output
- basic table output
- basic CSV output

Acceptance:

```bash
torn api get /user/basic --json
torn api get /user/basic --pretty
torn api get /user/basic --raw
torn api user bars --table
torn api faction members --csv
```

Tests:

- JSON parses
- CSV includes headers
- impossible explicit formats return clear errors

## Phase 4: Endpoint index and shortcuts

Deliverables:

- local endpoint index bundled in the binary or data dir
- `torn endpoints --service torn`
- `torn endpoints --service ff`
- `torn endpoints search <query>`
- shortcut commands for common Torn and FFScouter endpoints

Shortcut acceptance:

```bash
torn api user basic
torn api user bars
torn api user lookup
torn api faction members
torn api torn items
torn ff check-key
torn ff stats --user 3747263
torn ff flights --user 3747263
```

## Phase 5: Cache

Deliverables:

- SQLite cache
- deterministic cache keys excluding secrets
- default TTL for GET
- `--fresh`
- `--no-cache`
- `--cache-ttl`
- `torn cache status`
- `torn cache clear`

Acceptance:

```bash
torn api get /user/basic
torn api get /user/basic        # cache hit if within TTL
torn api get /user/basic --fresh
torn cache status
torn cache clear
```

Tests:

- cache hit/miss behavior
- key excludes secrets
- expired entries ignored

## Phase 6: Saved requests

Deliverables:

- add/list/run/remove saved requests
- saved request storage without secrets
- TUI can use same saved request file later

Acceptance:

```bash
torn saved add my-bars 'api get /user --param selections=bars'
torn saved list
torn saved run my-bars --pretty
torn saved remove my-bars
```

## Phase 7: TUI MVP

Deliverables:

- `torn tui`
- service browser
- endpoint list/search
- request editor for path and params
- run request
- response viewer
- config viewer

Acceptance:

```bash
torn tui
```

Inside TUI:

- select Torn API
- select `/user/basic`
- run request
- view pretty JSON response
- switch to FFScouter
- run `/check-key`
- quit cleanly

## Phase 8: Hardening

Deliverables:

- comprehensive tests
- documentation
- packaging instructions
- cross-platform terminal sanity checks
- better API error parsing
- optional shell completions

Acceptance:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
torn --help
torn config check
torn api get /user/basic --pretty
torn ff get /check-key --pretty
torn tui
```

## Deferred features

These are intentionally not MVP:

- bank planning
- stock ROI analysis
- inventory valuation
- faction war dashboards
- FFScouter enrichment dashboards
- OpenAPI auto-codegen
- plugin system
- browser automation

They can be built later on top of the API engine.
