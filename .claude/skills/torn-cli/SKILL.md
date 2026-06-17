---
name: torn-cli
description: Use the torn CLI safely from an AI agent to inspect Torn API v2 and FFScouter data, monitor player status/hospital timers, summarize faction/user/log data, and avoid leaking API keys or private responses.
license: MIT
compatibility: Designed for Claude Code/project Agent Skills; requires the torn binary or cargo run from this repository, plus user-provided Torn/FFScouter API keys.
metadata:
  author: manuelcecchetto
  repository: https://github.com/manuelcecchetto/torn-cli
---

# torn-cli agent skill

Use this skill when a user asks an AI agent to inspect Torn API v2, FFScouter, Torn logs, player/faction status, hospital timers, or this repository's `torn` CLI behavior.

## Safety rules

1. Never print, log, commit, or paste real Torn/FFScouter API keys.
2. Treat Torn responses as private player/faction/account data unless the user says otherwise.
3. Use `--json` for machine parsing and `--pretty`/`--table` for user-facing summaries.
4. Prefer narrow selections and filters. Do not fetch full logs or broad private data unless explicitly requested.
5. For `--watch`, use a bounded shell timeout in automation unless the user explicitly wants a long-running monitor.
6. If command output could contain secrets or sensitive data, summarize it instead of dumping it.

## Quick environment check

From the repository:

```bash
cargo run --quiet -- config check
```

If `torn` is installed on `PATH`:

```bash
torn config check
```

Use `torn config set torn-api-key` and `torn config set ffscouter-api-key` for interactive setup. Do not pass keys on the command line unless the user explicitly accepts shell-history risk.

## Common tasks

### Check a player's status

Machine-readable:

```bash
torn api user basic --id PLAYER_ID --json
```

Human summary:

```bash
torn api user basic --id PLAYER_ID --pretty
```

Monitor for hospital release interactively:

```bash
torn --watch 30s --pretty api user basic --id PLAYER_ID
```

For bounded automation on GNU/Linux, use `timeout`. On Windows PowerShell, prefer one-off polling from the agent or ask the user to run the watch interactively and stop it with Ctrl+C.

### Summarize faction members

```bash
torn api faction members --id FACTION_ID --table
```

### Check FFScouter stats

```bash
torn ff stats --target PLAYER_ID --json
```

### Find targets

```bash
torn ff targets --preset respect --limit 10 --pretty
```

### Analyze logs cautiously

Only use full-access log commands when the user asks for account-log analysis:

```bash
torn logs analyze --since 7d --group-by category --table
```

For content searches, keep windows and filters tight:

```bash
torn logs analyze --since 24h --contains xanax --group-by type --json
```

## Output handling

- `--json`: parse with `jq` or agent code; best for exact fields.
- `--pretty`: schema-aware, colored on terminals; best for summaries.
- `--table`: compact rows; best for watch/status overviews.
- `--raw`: only when debugging raw API behavior.
- `--csv`: for spreadsheets or simple tabular exports.

## Repository maintenance

Before suggesting code changes, run:

```bash
cargo fmt --check
cargo test --all
cargo clippy --all-targets --all-features -- -D warnings
```

When adding docs or examples, use copy/paste-safe placeholders like `PLAYER_ID` instead of angle brackets, and never include real keys.
