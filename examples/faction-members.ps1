param(
  [Parameter(Mandatory = $true)]
  [string]$FactionId
)

$ErrorActionPreference = "Stop"

torn api faction members --id $FactionId --table
