# Documentation

This directory contains public documentation for `torn-cli`.

## User and contributor docs

| File | Purpose |
|---|---|
| [`API_COVERAGE.md`](API_COVERAGE.md) | Torn API v2 and FFScouter coverage model, endpoint index guidance, rate-limit notes |
| [`FFSCOUTER.md`](FFSCOUTER.md) | FFScouter API capabilities: stats, history, flights, activity, hit calling, targets, losses |
| [`PRETTY_OUTPUT.md`](PRETTY_OUTPUT.md) | `--watch` behavior and schema-aware `--pretty` output families |
| [`PRIVACY.md`](PRIVACY.md) | Data handling, secret redaction, cache/saved-request privacy, no-telemetry policy |
| [`PERMISSIONS.md`](PERMISSIONS.md) | Torn key access levels, custom key selection matching, preflight behavior, and TUI permission display |
| [`TESTING.md`](TESTING.md) | Offline tests, optional online smoke tests, safe real-key handling |
| [`TORN_LOGS.md`](TORN_LOGS.md) | Torn user-log API research, catalog, filtering, grouping, and privacy guidance |
| [`LOG_PRESETS.md`](LOG_PRESETS.md) | Built-in and user-created log presets for security, money, combat, faction, items, etc. |
| [`torn-cli-design-docs/`](torn-cli-design-docs/) | Detailed product, CLI, API, architecture, TUI, output, and roadmap design docs |
| [`../examples/`](../examples/) | Safe copy/paste CLI examples for humans and AI agents |
| [`../.claude/skills/torn-cli/SKILL.md`](../.claude/skills/torn-cli/SKILL.md) | Project Agent Skill for using `torn` safely from AI agents |

## Documentation rules

- Use placeholder keys only, such as `your_torn_api_key_here` or `<redacted>`.
- Do not paste real Torn or FFScouter responses if they contain private player/faction data.
- Keep examples generic and avoid hardcoded personal Torn IDs unless clearly marked as fake.
- If API behavior changes, update both the implementation and the relevant docs in the same pull request.
