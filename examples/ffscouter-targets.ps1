param(
  [string]$Preset = "respect",
  [int]$Limit = 10
)

$ErrorActionPreference = "Stop"

torn ff targets --preset $Preset --limit $Limit --pretty
