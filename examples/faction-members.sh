#!/usr/bin/env bash
set -euo pipefail

faction_id="${1:-}"

if [[ -z "$faction_id" ]]; then
  echo "usage: $0 <faction-id>" >&2
  exit 2
fi

exec torn api faction members --id "$faction_id" --table
