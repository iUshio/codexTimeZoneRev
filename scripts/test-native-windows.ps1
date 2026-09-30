param(
    [string]$FlutterSdk = '',
    [switch]$InstalledPreflight
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
if (!$FlutterSdk) { $FlutterSdk = $env:FLUTTER_ROOT }
if (!$FlutterSdk) {
    $dartCommand = Get-Command dart.bat -ErrorAction SilentlyContinue
    if ($dartCommand) { $FlutterSdk = Split-Path -Parent (Split-Path -Parent $dartCommand.Source) }
}
if (!$FlutterSdk) { throw 'Flutter SDK not found. Set FLUTTER_ROOT or pass -FlutterSdk.' }
$dart = Join-Path $FlutterSdk 'bin\dart.bat'
if (!(Test-Path -LiteralPath $dart)) { throw "Dart SDK not found: $dart" }
$env:PATH = "$(Join-Path $FlutterSdk 'bin');$env:PATH"
$env:CARGO_TARGET_DIR = Join-Path $projectRoot 'environment\flutter-cargo-target'
$cachedCargo = Join-Path $projectRoot 'environment\cargo'
if (Test-Path -LiteralPath $cachedCargo) { $env:CARGO_HOME = $cachedCargo }
function Invoke-Checked([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program failed ($LASTEXITCODE)." }
}
Invoke-Checked 'cargo' @('build', '--release', '--manifest-path', (Join-Path $projectRoot 'native\launcher_core\Cargo.toml'))
$fixtureRoot = Join-Path $projectRoot ('environment\native-acceptance-' + [guid]::NewGuid().ToString('N'))
$clientRoot = Join-Path $fixtureRoot '模拟 客户端'
New-Item -ItemType Directory -Path $clientRoot -Force | Out-Null
New-Item -ItemType File -Path (Join-Path $fixtureRoot '.acceptance-fixture'), (Join-Path $clientRoot '.test-client'), (Join-Path $clientRoot 'icudtl.dat') | Out-Null
Invoke-Checked 'rustc' @('--edition=2021', '--crate-name', 'codex_fixture', (Join-Path $projectRoot 'flutter_app\tool\fixtures\codex.rs'), '-o', (Join-Path $clientRoot 'Codex.exe'))
Push-Location (Join-Path $projectRoot 'flutter_app')
try {
    Invoke-Checked $dart @('compile', 'exe', 'tool/native_acceptance.dart', '-o', (Join-Path $fixtureRoot 'acceptance.exe'))
    $env:CODEX_TZ_NATIVE_LIBRARY = Join-Path $env:CARGO_TARGET_DIR 'release\codex_timezone_core.dll'
    $previousTimezone = $env:TZ
    $previousElectron = $env:ELECTRON_RUN_AS_NODE
    try {
        $env:TZ = 'Etc/UTC'
        $env:ELECTRON_RUN_AS_NODE = 'fixture-parent'
        Invoke-Checked (Join-Path $fixtureRoot 'acceptance.exe') @()
    } finally {
        $env:TZ = $previousTimezone
        $env:ELECTRON_RUN_AS_NODE = $previousElectron
    }
    Write-Output "Isolated evidence retained at: $fixtureRoot"
    if ($InstalledPreflight) {
        # Opt-in: package-owning user only. Never resumes the installed client.
        Invoke-Checked $dart @('compile', 'exe', 'tool/launch_preflight_acceptance.dart', '-o', (Join-Path $fixtureRoot 'preflight.exe'))
        Invoke-Checked (Join-Path $fixtureRoot 'preflight.exe') @()
    }
} finally { Pop-Location }
