# Example AI-agent brief

Use this prompt with an AI coding agent that has shell access in a project where `torn` is installed:

```text
You can use the `torn` CLI to inspect Torn API v2 and FFScouter data. Safety rules:
- Never print, log, or commit API keys or full private logs.
- Use `torn config check` to verify key presence/access.
- Prefer `--json` for machine parsing, `--pretty` for human summaries, and `--table` for compact watch output.
- For watch-style commands, use bounded shell timeouts unless the user explicitly wants a long-running monitor.

Task: Check player PLAYER_ID, summarize their level/status/last action, and if they are in hospital estimate when they leave.

Suggested command:
torn api user basic --id PLAYER_ID --json
```

For reusable agent behavior, install or rely on the project skill at `.claude/skills/torn-cli/SKILL.md`.
