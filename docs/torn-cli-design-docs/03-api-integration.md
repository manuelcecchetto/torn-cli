# 03 API Integration

## Services

### Torn API v2

- Base URL: `https://api.torn.com/v2`
- Official OpenAPI: `https://www.torn.com/swagger/openapi.json`
- Swagger UI: `https://www.torn.com/swagger.php`
- Auth: `Authorization: ApiKey <TORN_API_KEY>`

### FFScouter API

- Base URL: `https://ffscouter.com/api/v1`
- Docs: `https://ffscouter.com/api-docs`
- Auth: query parameter `key=<FFSCOUTER_API_KEY>`

## Environment variables

```env
TORN_API_KEY=
FFSCOUTER_API_KEY=

TORN_BASE_URL=https://api.torn.com/v2
FFSCOUTER_BASE_URL=https://ffscouter.com/api/v1
TORN_API_INDEX_PATH=
```

## Local endpoint index

The tool should ship with a curated endpoint index and optionally support refreshing it from official docs.

Initial groups:

### Torn groups

- `user` — player profile, stats, inventory, crimes, travel, messages
- `faction` — faction data, wars, crimes, revives, stats, territory, applications
- `torn` — global/public Torn data and shared reference endpoints
- `market` — market data
- `company` — company data
- `racing` — racing data
- `forum` — forum-related data
- `property` — property data
- `key` — API key inspection and access endpoints

### FFScouter groups

- `stats`
- `travel`
- `key`
- `targets`
- `announcements`

## Notable Torn endpoints for MVP

```text
GET /user/lookup
GET /user/basic
GET /user?selections=bars
GET /user/inventory
GET /faction/basic
GET /faction/members
GET /faction/attacksfull
GET /torn/items
GET /torn/stocks
GET /market/itemmarket
GET /key/info
```

## Notable FFScouter endpoints for MVP

```text
GET /check-key
GET /get-stats
GET /get-stats-history
GET /player-flights
POST /register
GET /get-targets
GET /announcements
```

## Request model

Represent both Torn and FFScouter requests with the same internal structure:

```rust
pub enum Service {
    Torn,
    Ffscouter,
}

pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

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

## Response model

```rust
pub struct ApiResponse {
    pub service: Service,
    pub status: u16,
    pub headers: HeaderMap,
    pub body_text: String,
    pub body_json: Option<serde_json::Value>,
    pub from_cache: bool,
    pub cache_key: Option<String>,
    pub elapsed_ms: u128,
}
```

## Error handling

Do not assume all API failures are HTTP failures. APIs may return HTTP 200 with an error payload.

Recommended error categories:

```rust
pub enum AppError {
    Config(ConfigError),
    MissingApiKey { service: Service },
    Network(reqwest::Error),
    HttpStatus { status: u16, body: String },
    ApiError { service: Service, code: Option<String>, message: String },
    Json(serde_json::Error),
    Cache(CacheError),
    Output(OutputError),
}
```

## Auth details

### Torn

Attach:

```http
Authorization: ApiKey <TORN_API_KEY>
```

Never add the key to the query string unless explicitly implementing legacy compatibility.

### FFScouter

Attach query parameter:

```text
key=<FFSCOUTER_API_KEY>
```

The displayed/logged URL must redact this:

```text
https://ffscouter.com/api/v1/check-key?key=<redacted>
```

## Rate limits and caching

The tool should avoid aggressive polling. MVP cache policy:

- Default cache TTL: 30 seconds for most GET requests
- `--fresh` bypasses cache and updates it
- `--no-cache` bypasses and does not update cache
- POST requests are never cached by default
- Cache key should include service, method, path, sorted query params, and body hash, but not raw API keys

## Endpoint discovery

MVP can use a static JSON index. Later versions can add:

```bash
torn endpoints refresh
```

This should fetch Torn OpenAPI and FFScouter docs, parse endpoints, and update the local index.

## Verification commands

A working implementation should pass these smoke tests with valid keys:

```bash
torn config check
torn api get /user/basic --pretty
torn api get /user --param selections=bars --pretty
torn ff get /check-key --pretty
torn endpoints --service torn
torn endpoints --service ff
```
