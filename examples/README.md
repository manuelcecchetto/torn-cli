# torn-cli examples

These examples are safe starting points for humans and AI agents. They assume `torn` is on `PATH` and that keys were configured with `torn config set ...` or environment variables.

> Never commit real Torn or FFScouter keys. Use `.env.example` as the template.

The `.sh` files are for Bash/macOS/Linux/Git Bash. The `.ps1` files are for Windows PowerShell.

## Monitor a player leaving hospital

macOS/Linux/Git Bash:

```bash
./examples/watch-hospital.sh PLAYER_ID 30s
```

Windows PowerShell:

```powershell
.\examples\watch-hospital.ps1 -PlayerId PLAYER_ID -Interval 30s
```

Equivalent raw command:

```bash
torn --watch 30s --pretty api user basic --id PLAYER_ID
```

## Inspect faction members

macOS/Linux/Git Bash:

```bash
./examples/faction-members.sh FACTION_ID
```

Windows PowerShell:

```powershell
.\examples\faction-members.ps1 -FactionId FACTION_ID
```

Equivalent raw command:

```bash
torn api faction members --id FACTION_ID --table
```

## Find FFScouter targets

macOS/Linux/Git Bash:

```bash
./examples/ffscouter-targets.sh respect 10
```

Windows PowerShell:

```powershell
.\examples\ffscouter-targets.ps1 -Preset respect -Limit 10
```

Equivalent raw command:

```bash
torn ff targets --preset respect --limit 10 --pretty
```

## Agent-friendly JSON

For AI agents and scripts, prefer JSON. Pipe into `jq` only after confirming no private data will be exposed:

```bash
torn api user basic --id PLAYER_ID --json | jq '.profile | {id, name, level, status}'
```

PowerShell can parse JSON natively:

```powershell
$response = torn api user basic --id PLAYER_ID --json | ConvertFrom-Json
$response.profile | Select-Object id, name, level, status
```

If PowerShell execution policy blocks `.ps1` scripts, run the raw `torn ...` command instead or use:

```powershell
powershell -ExecutionPolicy Bypass -File .\examples\watch-hospital.ps1 -PlayerId PLAYER_ID
```

Use `--pretty` for human summaries and `--json` for machine parsing.
