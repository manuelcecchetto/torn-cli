# 01 Product Brief

## Name

- **Binary:** `torn`
- **Repository/package:** `torn-cli`
- **Interactive mode:** `torn tui`

## One-liner

`torn` is a Rust CLI and TUI API workbench for the official Torn API v2 and FFScouter API.

## Primary goals

1. Provide a fast, reliable, scriptable CLI for Torn API v2.
2. Provide first-class FFScouter access from the same binary.
3. Support both ergonomic shortcuts and generic raw API calls.
4. Support human-readable output and automation-friendly JSON/CSV.
5. Keep credentials local and never print API keys.
6. Add a TUI as an interactive browser/editor/viewer over the same request engine.

## Non-goals

The core product should not be centered around:

- bank planning
- stock planning
- inventory sell planning
- war automation
- browser automation
- bot behavior that violates Torn rules

These can exist later as optional commands, saved request templates, or plugins, but the foundation is an API client/explorer.

## Target users

### Primary user

A Torn player or faction operator who wants a terminal-native API tool for:

- inspecting account/faction data
- debugging API responses
- exporting data
- combining Torn and FFScouter information
- quickly running saved API queries

### Secondary user

Developers building Torn-related scripts who want:

- a stable command-line interface
- JSON output
- endpoint discovery
- reusable request templates
- local caching

## Product principles

1. **CLI first, TUI second.** The CLI must be useful without the TUI.
2. **Generic access first, shortcuts second.** Every API endpoint should be reachable even before typed shortcuts exist.
3. **No secrets in output.** Redact API keys in logs, errors, debug output, saved requests, and crash reports.
4. **Composable output.** `--json`, `--csv`, and stable exit codes matter.
5. **One engine.** CLI and TUI share API client, config, cache, request model, and formatters.
6. **No hardcoded personal logic.** The tool should not hardcode Manuel-specific IDs, keys, bank values, or workflows.

## Success criteria

MVP success means:

- A user can configure keys in `.env` or config file.
- A user can call arbitrary Torn v2 paths.
- A user can call arbitrary FFScouter paths.
- Built-in shortcuts exist for the most common endpoints.
- Output can be JSON, pretty JSON, raw, table, or CSV where applicable.
- The TUI can browse services/endpoints, edit params, run requests, and view responses.
- The implementation is testable and documented.

## Anti-scope examples

Avoid building the MVP around commands like:

```bash
torn bank plan
torn stocks optimize
torn inventory sell-plan
```

Those can be later modules. The MVP should instead prioritize:

```bash
torn api get /user/basic
torn api get /user?selections=basic,bars
torn ff get /check-key
torn ff get /get-stats --param user_id=123456
torn endpoints --service torn
torn tui
```
