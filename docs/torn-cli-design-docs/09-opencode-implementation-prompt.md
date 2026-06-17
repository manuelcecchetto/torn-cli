# 09 OpenCode Implementation Prompt

Use this prompt with OpenCode, Codex, Claude Code, or another autonomous coding agent.

```text
Build a Rust CLI and TUI API workbench for Torn API v2 and FFScouter.

Project name: torn-cli
Binary name: torn
Language: Rust
Primary interface: CLI
Secondary interface: TUI launched with `torn tui`

Important product scope:
- This is primarily an API client/explorer for Torn and FFScouter.
- Do not build the MVP around bank planning, stock planning, or inventory sell planning.
- Those can be future modules, but the foundation must be generic API access, endpoint discovery, output formatting, caching, and TUI browsing.

Required APIs:
1. Torn API v2
   - Default base URL: https://api.torn.com/v2
   - Auth: HTTP header `Authorization: ApiKey <TORN_API_KEY>`
   - Official OpenAPI: https://www.torn.com/swagger/openapi.json

2. FFScouter API
   - Default base URL: https://ffscouter.com/api/v1
   - Auth: query parameter `key=<FFSCOUTER_API_KEY>`
   - Docs: https://ffscouter.com/api-docs

Configuration:
- Read `.env` by default with dotenvy.
- Support environment variables:
  - TORN_API_KEY
  - FFSCOUTER_API_KEY
  - TORN_BASE_URL, default https://api.torn.com/v2
  - FFSCOUTER_BASE_URL, default https://ffscouter.com/api/v1
- Include `.env.example` but never commit a real `.env`.
- Implement `torn config check` and `torn config show --redacted`.
- Never print full API keys.

CLI commands:
Implement this command structure:

`torn config check`
`torn config path`
`torn config show --redacted`

`torn endpoints --service torn|ff|all`
`torn endpoints search <query>`

`torn api get <path>`
`torn api post <path>`
`torn api user basic`
`torn api user bars`
`torn api user inventory`
`torn api user lookup`
`torn api faction basic`
`torn api faction members`
`torn api torn items`
`torn api torn stocks`
`torn api key info`

`torn ff get <path>`
`torn ff check-key`
`torn ff stats --user <id>`
`torn ff stats-history --user <id>`
`torn ff flights --user <id>`
`torn ff targets`
`torn ff announcements`

`torn cache status`
`torn cache clear`

`torn tui`

Generic request behavior:
- `torn api get /user/basic`
- `torn api get /user --param selections=basic,bars`
- `torn ff get /check-key`
- `torn ff get /get-stats --param user_id=123456`
- Allow repeated `--param key=value`.
- Merge params from path query string and `--param`, with explicit `--param` taking precedence.

Output modes:
- Default auto mode.
- `--json` compact JSON.
- `--pretty` pretty JSON.
- `--raw` exact response body.
- `--table` table where possible.
- `--csv` CSV where possible.
- Explicit impossible format should return a clear error.

Caching:
- Use SQLite or a simple file cache. SQLite preferred.
- Cache GET requests by default with TTL 30 seconds.
- Support `--fresh`, `--no-cache`, and `--cache-ttl`.
- Cache keys must exclude secrets.
- POST should not be cached by default.

TUI:
- Use ratatui + crossterm.
- `torn tui` should open a terminal API browser.
- It must reuse the same config, request model, API client, cache, and output formatter as the CLI.
- Initial TUI screens:
  1. Service browser: Torn / FFScouter
  2. Endpoint list and search
  3. Request editor for method/path/query params/body
  4. Response viewer with JSON/raw/table modes
  5. Config viewer with redacted keys
  6. Cache viewer/status

Recommended crates:
- clap with derive
- tokio
- reqwest with json and rustls-tls
- serde, serde_json
- dotenvy
- directories
- anyhow, thiserror
- ratatui, crossterm
- rusqlite with bundled, or another simple cache
- comfy-table
- csv
- sha2, hex
- url

Architecture:
Create modules roughly like:

src/main.rs
src/cli.rs
src/config.rs
src/error.rs
src/request/model.rs
src/http/client.rs
src/http/auth.rs
src/torn/client.rs
src/torn/commands.rs
src/ffscouter/client.rs
src/ffscouter/commands.rs
src/output/json.rs
src/output/table.rs
src/output/csv.rs
src/cache/sqlite.rs
src/endpoints/index.rs
src/tui/app.rs
src/tui/screens/...

Core types:
- Service enum: Torn, Ffscouter
- HttpMethod enum
- ApiRequest struct
- ApiResponse struct
- CachePolicy enum
- OutputMode enum

Security requirements:
- Never print full API keys.
- Redact FFScouter `key=` query parameter in displayed/logged URLs.
- Saved requests, cache keys, logs, and errors must not contain raw secrets.
- Add tests for redaction.

Tests:
Add unit/integration tests for:
- config loading and precedence
- secret redaction
- URL construction and query param merging
- Torn auth header
- FFScouter key query param
- cache key generation excluding secrets
- output formatting
- CLI help and basic command parsing

Acceptance commands:
These must work before considering the task complete:

cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo run -- --help
cargo run -- config check
cargo run -- endpoints --service torn
cargo run -- endpoints --service ff
cargo run -- api get /user/basic --pretty
cargo run -- api get /user --param selections=basic,bars --json
cargo run -- ff get /check-key --pretty
cargo run -- tui

If real API keys are not available, implement mock-server integration tests and make `config check` clearly report missing keys without panicking.

Deliverables:
- Working Rust project
- README with setup and examples
- `.env.example`
- Tests
- TUI starts and exits cleanly
- No hardcoded personal API keys, player IDs, or bank logic
```
