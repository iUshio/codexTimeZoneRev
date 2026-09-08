$ErrorActionPreference = 'Stop'
$development = $env:CODEX_TZ_DEV_ROOT
if ($development) {
    $rustup = Join-Path $development 'Rust\rustup'
    $cargo = Join-Path $development 'Rust\cargo'
    if (Test-Path $rustup) { $env:RUSTUP_HOME = $rustup }
    if (Test-Path $cargo) { $env:CARGO_HOME = $cargo }
    $paths = @('NodeJS', 'Git\cmd', 'Rust\cargo\bin') |
        ForEach-Object { Join-Path $development $_ } |
        Where-Object { Test-Path $_ }
    if ($paths) { $env:PATH = ($paths -join ';') + ';' + $env:PATH }
}
if ($env:CODEX_TZ_PROXY) {
    $env:HTTP_PROXY = $env:CODEX_TZ_PROXY
    $env:HTTPS_PROXY = $env:CODEX_TZ_PROXY
    $env:CARGO_HTTP_PROXY = $env:CODEX_TZ_PROXY
}
$vsDevCmd = if ($development) { Join-Path $development 'VisualStudioBuildTools\Common7\Tools\VsDevCmd.bat' } else { '' }
if (-not $vsDevCmd -or -not (Test-Path -LiteralPath $vsDevCmd)) {
    $vsDevCmd = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat'
}
if (-not (Test-Path -LiteralPath $vsDevCmd)) { throw 'Visual Studio 2022 Build Tools not found. Install the Desktop development with C++ workload.' }
cmd /s /c "`"$vsDevCmd`" -arch=x64 -host_arch=x64 >nul && set" | ForEach-Object {
    if ($_ -match '^([^=]+)=(.*)$') { Set-Item -Path "Env:$($matches[1])" -Value $matches[2] }
}
