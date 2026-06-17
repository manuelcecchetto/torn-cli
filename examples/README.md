# torn-cli examples

These examples are safe starting points for humans and AI agents. They assume `torn` is on `PATH` and that keys were configured with `torn config set ...` or environment variables.

> Never commit real Torn or FFScouter keys. Use `.env.example` as the template.

## Monitor a player leaving hospital

```bash
./examples/watch-hospital.sh <player-id> 30s
```

Equivalent raw command:

```bash
torn --watch 30s --pretty api user basic --id <player-id>
```

## Inspect faction members

```bash
./examples/faction-members.sh <faction-id>
```

Equivalent raw command:

```bash
torn api faction members --id <faction-id> --table
```

## Find FFScouter targets

```bash
./examples/ffscouter-targets.sh respect 10
```

Equivalent raw command:

```bash
torn ff targets --preset respect --limit 10 --pretty
```

## Agent-friendly JSON

For AI agents and scripts, prefer JSON and pipe into `jq` only after confirming no private data will be exposed:

```bash
torn api user basic --id <player-id> --json | jq '.profile | {id, name, level, status}'
```

Use `--pretty` for human summaries and `--json` for machine parsing.
