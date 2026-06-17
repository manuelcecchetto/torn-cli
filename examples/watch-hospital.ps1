param(
  [Parameter(Mandatory = $true)]
  [string]$PlayerId,

  [string]$Interval = "30s"
)

$ErrorActionPreference = "Stop"

torn --watch $Interval --pretty api user basic --id $PlayerId
