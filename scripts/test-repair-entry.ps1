$ErrorActionPreference = 'Stop'
$native = Join-Path $PSScriptRoot '../native/launcher_core/src/platform/win'
$powershell = Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe'
$root = Join-Path ([IO.Path]::GetTempPath()) ('codex-repair-entry-test-' + [guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($root) | Out-Null
$checks = @()
foreach ($scenario in @('success','cancelled','apply-failed','plan-failed')) {
    $folder = Join-Path $root $scenario
    [IO.Directory]::CreateDirectory($folder) | Out-Null
    Copy-Item -LiteralPath (Join-Path $native 'repair_entry.ps1') -Destination $folder
    $fixture = @'
param($Mode,$PlanPath,$PackageFullName,[switch]$RequireCurrent)
if ($Mode -ne 'Plan' -or !$RequireCurrent) { exit 9 }
if ((Split-Path $PSScriptRoot -Leaf) -eq 'plan-failed') { exit 8 }
[IO.File]::WriteAllText($PlanPath, '{}')
exit 0
'@
    [IO.File]::WriteAllText((Join-Path $folder 'repair_package_acl.ps1'), $fixture, [Text.UTF8Encoding]::new($true))
    $runner = @'
function Start-Process {
    param($FilePath,$Verb,$WindowStyle,[switch]$PassThru,$ArgumentList)
    if ($Verb -ne 'RunAs' -or $WindowStyle -ne 'Hidden' -or $ArgumentList -notcontains '-RequireCurrent') { throw 'Unexpected elevation contract.' }
    $scenario = Split-Path $PSScriptRoot -Leaf
    if ($scenario -eq 'cancelled') { throw [ComponentModel.Win32Exception]::new(1223) }
    $code = if ($scenario -eq 'apply-failed') { 1 } else { 0 }
    [IO.File]::WriteAllText((Join-Path $PSScriptRoot 'plan.json.apply-result.json'), '{"Status":"success","Message":"中文结果。"}', [Text.UTF8Encoding]::new($false))
    $p = [pscustomobject]@{Handle=0;ExitCode=$code}
    $p | Add-Member -MemberType ScriptMethod -Name WaitForExit -Value {}
    return $p
}
& (Join-Path $PSScriptRoot 'repair_entry.ps1') -PackageFullName 'OpenAI.Codex_1.2.3.4_x64__2p2nqsd0c76g0'
exit $LASTEXITCODE
'@
    [IO.File]::WriteAllText((Join-Path $folder 'runner.ps1'), $runner, [Text.UTF8Encoding]::new($true))
    & $powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $folder 'runner.ps1')
    $code = $LASTEXITCODE
    $result = Get-Content -LiteralPath (Join-Path $folder 'result.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    $expected = if ($scenario -eq 'success') { 'success' } elseif ($scenario -eq 'cancelled') { 'cancelled' } else { 'failed' }
    if ($result.status -ne $expected -or (($code -eq 0) -ne ($scenario -eq 'success'))) { throw "Unexpected result for $scenario : $code / $($result.status)" }
    $checks += $scenario
}
function Get-AppxPackage {
    param($Name)
    [pscustomobject]@{PackageFamilyName='OpenAI.Codex_2p2nqsd0c76g0';PackageFullName='OpenAI.Codex_2.0.0.0_x64__2p2nqsd0c76g0';Version=[version]'2.0.0.0'}
}
$rejected = $false
try { & (Join-Path $native 'repair_package_acl.ps1') -Mode Plan -PlanPath (Join-Path $root 'stale.json') -PackageFullName 'OpenAI.Codex_1.2.3.4_x64__2p2nqsd0c76g0' -RequireCurrent }
catch { $rejected = $_.Exception.Message.Contains('installed version changed') }
if (!$rejected -or (Test-Path -LiteralPath (Join-Path $root 'stale.json'))) { throw 'Stale repair was not rejected before planning.' }
$checks += 'version-race-rejected'
@{Status='passed';Checks=$checks;Evidence=$root;RealElevation=$false;ProductionPackageTouched=$false} | ConvertTo-Json
