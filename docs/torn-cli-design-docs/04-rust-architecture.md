# 04 Rust Architecture

## Recommended crates

```toml
[dependencies]
anyhow = "1"
thiserror = "1"
clap = { version = "4", features = ["derive", "env"] }
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
dotenvy = "0.15"
directories = "5"
chrono = { version = "0.4", features = ["serde"] }
url = "2"
sha2 = "0.10"
hex = "0.4"
comfy-table = "7"
csv = "1"
ratatui = "0.29"
crossterm = "0.28"
rusqlite = { version = "0.32", features = ["bundled"] }
```

Use current versions when implementing. The versions above are design targets, not strict pins.

## Module layout

```text
src/
  main.rs
  cli.rs
  config.rs
  error.rs

  request/
    mod.rs
    model.rs
    builder.rs

  http/
    mod.rs
    client.rs
    auth.rs

  torn/
    mod.rs
    client.rs
    commands.rs
    endpoints.rs

  ffscouter/
    mod.rs
    client.rs
    commands.rs
    endpoints.rs

  output/
    mod.rs
    json.rs
    table.rs
    csv.rs
    raw.rs

  cache/
    mod.rs
    sqlite.rs
    key.rs
    policy.rs

  endpoints/
    mod.rs
    index.rs
    search.rs

  saved/
    mod.rs
    requests.rs

  tui/
    mod.rs
    app.rs
    event.rs
    state.rs
    screens/
      mod.rs
      service_browser.rs
      request_editor.rs
      response_viewer.rs
      cache_viewer.rs
      config_viewer.rs
```

## Layering

```text
CLI parser / TUI input
        ↓
Command handlers / TUI actions
        ↓
Request builder
        ↓
Cache policy
        ↓
HTTP client + auth
        ↓
Response model
        ↓
Output formatter / TUI response viewer
```

The TUI must not implement separate request logic. It should construct the same `ApiRequest` used by the CLI and receive the same `ApiResponse`.

## Core types

### Service

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Service {
    Torn,
    Ffscouter,
}
```

### ApiRequest

```rust
#[derive(Debug, Clone)]
pub struct ApiRequest {
    pub service: Service,
    pub method: HttpMethod,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub body: Option<serde_json::Value>,
    pub use_auth: bool,
    pub cache_policy: CachePolicy,
}
```

### ApiClient

```rust
pub struct ApiClient {
    http: reqwest::Client,
    config: Config,
    cache: Option<CacheStore>,
}

impl ApiClient {
    pub async fn execute(&self, req: ApiRequest) -> Result<ApiResponse, AppError>;
}
```

### OutputMode

```rust
pub enum OutputMode {
    Auto,
    Table,
    JsonCompact,
    JsonPretty,
    Csv,
    Raw,
}
```

## CLI implementation notes

Use `clap` derive for command parsing.

Suggested top-level enum:

```rust
#[derive(Parser)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalOptions,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Config(ConfigCommand),
    Endpoints(EndpointsCommand),
    Api(ApiCommand),
    Ff(FfCommand),
    Saved(SavedCommand),
    Cache(CacheCommand),
    Tui(TuiCommand),
}
```

## Generic request handlers

Implement generic request first:

```rust
async fn handle_generic_get(
    service: Service,
    path: String,
    params: Vec<(String, String)>,
    opts: GlobalOptions,
) -> Result<(), AppError>
```

Then implement shortcuts as wrappers that call the same handler.

## Testing strategy

### Unit tests

- config loading precedence
- URL construction
- query parameter merging
- auth header/query behavior
- key redaction
- cache key generation
- output formatting for simple JSON arrays/objects

### Integration tests

Use a mock HTTP server for:

- Torn auth header attached correctly
- FFScouter key query attached and redacted
- HTTP error mapping
- API error payload mapping
- cache hit/miss behavior

### CLI tests

Use `assert_cmd` or similar for:

```bash
torn --help
torn config check
torn api get /user/basic --no-cache --pretty
torn ff get /check-key --no-cache --pretty
```

## Build targets

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo run -- --help
cargo run -- api get /user/basic --pretty
```
