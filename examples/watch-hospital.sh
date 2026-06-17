#!/usr/bin/env bash
set -euo pipefail

player_id="${1:-}"
interval="${2:-30s}"

if [[ -z "$player_id" ]]; then
  echo "usage: $0 <player-id> [interval]" >&2
  exit 2
fi

exec torn --watch "$interval" --pretty api user basic --id "$player_id"
