param([Parameter(Mandatory=$true)][string]$PackageFullName)
$ErrorActionPreference = 'Stop'
$env:PSModulePath = Join-Path $PSHOME 'Modules'
$powershell = Join-Path $PSHOME 'powershell.exe'
$core = Join-Path $PSScriptRoot 'repair_package_acl.ps1'
$plan = Join-Path $PSScriptRoot 'plan.json'
$resultPath = Join-Path $PSScriptRoot 'result.json'
$stage = 'plan'
$result = @{status='failed'; message='Repair did not complete.'}
try {
    & $powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $core -Mode Plan -PlanPath $plan -PackageFullName $PackageFullName -RequireCurrent > (Join-Path $PSScriptRoot 'plan.log') 2>&1
    if ($LASTEXITCODE -ne 0) { throw '权限不符合支持的修复条件，未修改。请查看 plan.log。' }
    $stage = 'uac'
    $child = Start-Process -FilePath $powershell -Verb RunAs -WindowStyle Hidden -PassThru -ArgumentList @('-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',('"' + $core + '"'),'-Mode','Apply','-PlanPath',('"' + $plan + '"'),'-PackageFullName',$PackageFullName,'-RequireCurrent')
    $null = $child.Handle
    $child.WaitForExit()
    $stage = 'verify-result'
    if ($child.ExitCode -ne 0) { throw '权限修复未完成，请查看 plan.json.apply-result.json。' }
    $applied = Get-Content -LiteralPath ($plan + '.apply-result.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($applied.Status -ne 'success') { throw '权限修复结果无效。' }
    $result = @{status='success'; message='权限修复完成，正在重新检查包身份。'}
} catch {
    $exception = $_.Exception
    $cancelled = $false
    while ($null -ne $exception) {
        if ($exception -is [ComponentModel.Win32Exception] -and $exception.NativeErrorCode -eq 1223) { $cancelled = $true }
        $exception = $exception.InnerException
    }
    $result = if ($cancelled) { @{status='cancelled'; message='已取消管理员确认，未执行修复或启动。'} }
        else { @{status='failed'; message=$_.Exception.Message} }
} finally {
    $result.stage = $stage
    [IO.File]::WriteAllText($resultPath, ($result | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
}
if ($result.status -eq 'success') { exit 0 }
exit 1
