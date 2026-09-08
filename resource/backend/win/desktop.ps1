param([Parameter(Mandatory=$true)][ValidateSet('build', 'dev')][string]$Action)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Initialize-DevelopmentEnvironment.ps1')
& node (Join-Path $PSScriptRoot '../../desktop.mjs') $Action win --configured
exit $LASTEXITCODE
