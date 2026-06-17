# 05 TUI Design

## Purpose

The TUI is an interactive API workbench launched with:

```bash
torn tui
```

It is not the primary product surface. It is a richer interface over the same request engine used by the CLI.

## Design goals

1. Browse Torn and FFScouter services.
2. Search endpoints.
3. Edit path/query/body parameters.
4. Run requests.
5. View responses as pretty JSON, raw text, tables, or CSV preview.
6. Save and rerun request templates.
7. Inspect cache and configuration state.

## Non-goals

The TUI should not hardcode personal dashboards such as bank planning or stock optimization in the MVP.

## Screen map

```text
TUI Root
├── Service Browser
│   ├── Torn API
│   └── FFScouter API
├── Request Editor
├── Response Viewer
├── Saved Requests
├── Cache Viewer
└── Config Viewer
```

## Main layout

```text
┌ torn ─────────────────────────────────────────────────────────────┐
│ Service: Torn API v2       Profile: default       Cache: enabled  │
├───────────────┬──────────────────────────────────────────────────┤
│ Groups        │ Endpoints                                        │
│ > user        │ > GET /user/basic                                │
│   faction     │   GET /user?selections=bars                      │
│   torn        │   GET /user/inventory                            │
│   market      │   GET /faction/members                           │
│   key         │   GET /torn/items                                │
├───────────────┴──────────────────────────────────────────────────┤
│ Request: GET /user/basic                                          │
│ Params:                                                           │
│   selections =                                                    │
├──────────────────────────────────────────────────────────────────┤
│ Response                                                          │
│ {                                                                 │
│   "player_id": 3747263,                                          │
│   ...                                                             │
│ }                                                                 │
├──────────────────────────────────────────────────────────────────┤
│ r run  / search  tab focus  s save  f fresh  j json  t table  q quit │
└──────────────────────────────────────────────────────────────────┘
```

## Keyboard controls

| Key | Action |
|---|---|
| `q` | Quit or go back |
| `tab` / `shift-tab` | Move focus between panes |
| `↑` / `↓` | Move selection |
| `enter` | Select endpoint or run focused action |
| `/` | Search endpoints |
| `r` | Run request |
| `f` | Run fresh, bypass cache |
| `s` | Save request template |
| `e` | Export response |
| `j` | Switch to JSON view |
| `t` | Switch to table view |
| `c` | Switch to CSV preview |
| `x` | Raw response view |
| `?` | Help overlay |

## Service browser

The browser displays service groups and endpoints from the local endpoint index.

### Torn API groups

- user
- faction
- torn
- market
- company
- racing
- forum
- property
- key

### FFScouter groups

- stats
- travel
- key
- targets
- announcements

## Request editor

The editor should support:

- method selection
- path editing
- query parameter editing
- JSON body editing for POST requests
- cache policy selection
- auth on/off toggle for debugging public endpoints

Suggested request editor model:

```rust
pub struct RequestDraft {
    pub service: Service,
    pub method: HttpMethod,
    pub path: String,
    pub query: Vec<QueryParamDraft>,
    pub body_text: String,
    pub use_auth: bool,
    pub output_mode: OutputMode,
    pub cache_policy: CachePolicy,
}
```

## Response viewer

The response viewer displays:

- HTTP status
- elapsed time
- cache hit/miss
- selected output format
- response body
- API error details if present

Response header example:

```text
200 OK    148 ms    cache: miss    service: torn
```

## Saved requests

Saved requests should be editable from TUI and runnable from CLI.

Storage should contain no secrets:

```json
{
  "name": "my-bars",
  "service": "torn",
  "method": "GET",
  "path": "/user",
  "query": { "selections": "bars" },
  "body": null,
  "use_auth": true
}
```

## Config viewer

The config viewer should show redacted configuration:

```text
TORN_API_KEY:       present, redacted
FFSCOUTER_API_KEY: present, redacted
TORN_BASE_URL:      https://api.torn.com/v2
FFSCOUTER_BASE_URL: https://ffscouter.com/api/v1
Cache dir:          /home/user/.cache/torn-cli
Config dir:         /home/user/.config/torn-cli
```

## Error display

Errors should be readable and actionable:

```text
Missing FFScouter API key

Set FFSCOUTER_API_KEY in one of:
- .env in the current directory
- ~/.config/torn-cli/config.toml
- process environment
```

## TUI implementation notes

Use `ratatui` and `crossterm`.

The TUI should run a background task for network requests so the interface does not freeze. Use `tokio::sync::mpsc` channels between UI event loop and request worker.

Simplified state model:

```rust
pub struct TuiApp {
    pub active_screen: Screen,
    pub endpoint_index: EndpointIndex,
    pub request_draft: RequestDraft,
    pub response: Option<ApiResponse>,
    pub loading: bool,
    pub error: Option<String>,
}
```
