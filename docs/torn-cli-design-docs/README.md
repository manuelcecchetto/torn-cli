# Torn CLI Design Documentation

This package contains the design documentation for a Rust-based command-line and terminal UI tool for the Torn API v2 and FFScouter API.

## Product summary

**Binary name:** `torn`  
**Project name:** `torn-cli`  
**Primary goal:** provide a fast, scriptable, typed CLI for Torn and FFScouter APIs.  
**Secondary goal:** provide an interactive TUI API workbench launched with `torn tui`.

This is not primarily a bank planner, stock planner, or finance assistant. Those may be optional plugins or examples later, but the core product is an API client and explorer.

## Documents

| File | Purpose |
|---|---|
| `01-product-brief.md` | Product scope, goals, non-goals, users, success criteria |
| `02-cli-command-design.md` | CLI command tree, examples, output modes, UX conventions |
| `03-api-integration.md` | Torn and FFScouter API integration details, auth, endpoints, rate limits |
| `04-rust-architecture.md` | Proposed Rust module architecture and crate choices |
| `05-tui-design.md` | Interactive TUI design, screens, keyboard controls, state model |
| `06-config-cache-security.md` | Configuration, secrets, cache, security, privacy model |
| `07-output-formats.md` | JSON, raw, table, CSV, and error output contracts |
| `08-mvp-roadmap.md` | MVP phases, acceptance criteria, implementation order |
| `09-opencode-implementation-prompt.md` | Ready-to-use implementation prompt for OpenCode or another coding agent |
| `.env.example` | Example local environment variables |

## Quick implementation target

The initial MVP should make these work:

```bash
torn --help
torn config check
torn api get /user/basic --pretty
torn api get /user?selections=basic,bars --json
torn ff get /check-key --pretty
torn endpoints --service torn
torn endpoints --service ff
torn tui
```

## Public repository docs

The repository-level docs in `../README.md`, `../PRIVACY.md`, `../TESTING.md`, and `../API_COVERAGE.md` summarize the public setup, privacy, testing, and coverage policies derived from this design package.

## Key design rule

The TUI must reuse the same API client, config loader, cache, output formatter, and request model as the CLI. Do not implement a separate TUI-specific API path.
