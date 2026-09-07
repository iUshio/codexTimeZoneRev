$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Initialize-DevelopmentEnvironment.ps1')
$root = Split-Path $PSScriptRoot
& (Join-Path $root 'backend\build.ps1')
Push-Location $root
try { & pnpm.cmd tauri dev } finally { Pop-Location }
