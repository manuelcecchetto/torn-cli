#!/usr/bin/env bash
set -euo pipefail

preset="${1:-respect}"
limit="${2:-10}"

exec torn ff targets --preset "$preset" --limit "$limit" --pretty
