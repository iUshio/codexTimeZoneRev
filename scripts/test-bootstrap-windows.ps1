param([string]$CMake = 'cmake')
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$build = Join-Path $root 'environment/bootstrap-tests'
& $CMake -S (Join-Path $PSScriptRoot 'bootstrap-tests') -B $build -A x64
if ($LASTEXITCODE -ne 0) { throw 'Bootstrap test configuration failed.' }
& $CMake --build $build --config Release
if ($LASTEXITCODE -ne 0) { throw 'Bootstrap test build failed.' }
& (Join-Path $build 'Release/bootstrap_tests.exe')
if ($LASTEXITCODE -ne 0) { throw 'Bootstrap contract tests failed.' }
