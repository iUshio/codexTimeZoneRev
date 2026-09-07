$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Initialize-DevelopmentEnvironment.ps1')
$root = Split-Path $PSScriptRoot
& (Join-Path $root 'backend\build.ps1')
Push-Location $root
try {
    & pnpm.cmd install
    if ($LASTEXITCODE -ne 0) { throw '前端依赖安装失败' }
    & pnpm.cmd build
    if ($LASTEXITCODE -ne 0) { throw 'Vue 构建失败' }
    & pnpm.cmd tauri build --no-bundle
    if ($LASTEXITCODE -ne 0) { throw 'Tauri 构建失败' }
    Copy-Item -LiteralPath (Join-Path $root 'src-tauri\target\release\codextimezonerev.exe') -Destination (Join-Path (Split-Path $root) 'CodexTimeZoneLauncher.exe') -Force
} finally { Pop-Location }
