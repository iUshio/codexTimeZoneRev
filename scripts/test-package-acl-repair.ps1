$ErrorActionPreference = 'Stop'
$repairScript = Join-Path $PSScriptRoot '../native/launcher_core/src/platform/win/repair_package_acl.ps1'
. $repairScript -FunctionsOnly
$testDirectory = Join-Path ([IO.Path]::GetTempPath()) ('codex-acl-repair-selftest-' + [Guid]::NewGuid().ToString('N'))
$testRoot = [IO.Directory]::CreateDirectory($testDirectory).FullName
$testApp = [IO.Directory]::CreateDirectory((Join-Path $testRoot 'app')).FullName
$testExe = Join-Path $testApp 'fixture.txt'
[IO.File]::WriteAllText($testExe, 'ACL self-test fixture; never executes.')
$testTargets = @($testRoot, $testApp, $testExe)
$checks = [Collections.Generic.List[string]]::new()

function Assert-Check([bool]$Condition, [string]$Name) {
    if (!$Condition) { throw "Self-test failed: $Name" }
    $checks.Add($Name)
}
function Assert-Rejected([scriptblock]$Operation, [string]$Name) {
    $rejected = $false
    try { & $Operation } catch { $rejected = $true }
    Assert-Check $rejected $Name
}
function Set-TestDacl([string]$Path, [string]$Dacl) {
    $fullPath = [IO.Path]::GetFullPath($Path)
    if ($fullPath -ne $testRoot -and !$fullPath.StartsWith($testRoot + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Self-test refused a path outside its newly created temporary directory.'
    }
    [CodexAclSelfTestWriter]::SetDirectoryDacl($fullPath, $Dacl)
}

try {
    Assert-Check ((Assert-PackageName 'OpenAI.Codex_26.928.1915.0_x64__2p2nqsd0c76g0') -eq [version]'26.928.1915.0') 'new installed version accepted'
    Assert-Check ((Assert-PackageName 'OpenAI.Codex_27.1000.42.0_arm64__2p2nqsd0c76g0') -eq [version]'27.1000.42.0') 'future version and arm64 accepted'
    foreach ($bad in @('', '..\WindowsApps', 'Other.App_26.928.1915.0_x64__2p2nqsd0c76g0', 'OpenAI.Codex_26.928.1915.0_x64__wrong', 'OpenAI.Codex_26.928.1915.0_x64__2p2nqsd0c76g0\app')) {
        Assert-Rejected { Assert-PackageName $bad } 'unexpected package identity or path rejected'
    }
    # Exercise the exact native writer body, without enabling backup/restore
    # privileges: all files in this test tree belong to the current user.
    $parseErrors = $null
    $tokens = $null
    $ast = [Management.Automation.Language.Parser]::ParseFile($repairScript, [ref]$tokens, [ref]$parseErrors)
    Assert-Check ($parseErrors.Count -eq 0) 'repair script parses'
    $nativeSource = @($ast.FindAll({
        param($node)
        $node -is [Management.Automation.Language.StringConstantExpressionAst] -and
        $node.Value.Contains('public static class CodexPackageDaclRepair')
    }, $true))
    Assert-Check ($nativeSource.Count -eq 1) 'one native writer definition'
    $privilegeLine = 'EnablePrivilege("SeBackupPrivilege"); EnablePrivilege("SeRestorePrivilege");'
    Assert-Check ($nativeSource[0].Value.Contains($privilegeLine)) 'test identifies and disables privilege calls'
    Add-Type -TypeDefinition ($nativeSource[0].Value.Replace('CodexPackageDaclRepair', 'CodexAclSelfTestWriter').Replace($privilegeLine, ''))

    $userSid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    $conditional = '(XA;OICI;0x1200a9;;;BU;(WIN://SYSAPPID Contains "OpenAI.Codex_2p2nqsd0c76g0"))'
    $unconditional = '(A;OICI;0x1200a9;;;BU)'
    $userRule = "(A;OICI;FA;;;$userSid)"
    $testDacl = 'D:AI' + $conditional + $unconditional + $userRule
    Set-TestDacl $testRoot $testDacl
    $before = @(foreach ($path in $testTargets) { Get-Snapshot $path })
    $testPlan = @(for ($i = 0; $i -lt $before.Count; $i++) {
        $snapshot = $before[$i]
        [pscustomobject]@{
            Path=$snapshot.Path;Owner=$snapshot.Owner;BeforeSddl=$snapshot.Sddl
            BeforeDacl=$snapshot.Dacl;AfterDacl=(Get-NarrowedDacl $snapshot.Sddl ($i -eq 0))
        }
    })
    foreach ($snapshot in $testPlan) { Assert-ReviewedSnapshot $snapshot ($snapshot.Path -eq $testRoot) }
    $reordered = 'D:AI' + $userRule + $unconditional + $conditional
    Assert-Check (Test-DaclEquivalent $testDacl $reordered) 'all-allow reordering accepted including unchanged callback bytes'
    Assert-Check (!(Test-DaclEquivalent $testDacl ($testDacl + $unconditional))) 'ACE multiplicity preserved'
    Assert-Check (!(Test-DaclEquivalent $testDacl ($testDacl.Replace('D:AI', 'D:PAI')))) 'DACL protection control flag preserved'
    Assert-Check (!(Test-DaclEquivalent $testDacl ($testDacl.Replace('OpenAI.Codex_2p2nqsd0c76g0', 'Unrelated.Identity')))) 'conditional expression preserved'
    $deny = '(D;OICI;0x20;;;BU)'
    Assert-Check (!(Test-DaclEquivalent ('D:AI' + $deny + $testDacl.Substring(4)) ($testDacl + $deny))) 'deny ACE ordering is not normalized'
    $inconsistent = [pscustomobject]@{
        BeforeSddl=$testPlan[0].BeforeSddl;BeforeDacl=$testPlan[0].AfterDacl;AfterDacl=$testPlan[0].AfterDacl
    }
    Assert-Rejected { Assert-ReviewedSnapshot $inconsistent $true } 'inconsistent BeforeSddl and BeforeDacl rejected'
    Assert-Rejected { Get-NarrowedDacl ($testDacl + $unconditional) $true } 'multiple unconditional matching rules rejected'

    # Apply and rollback traverse the native inheritance path on this temp tree.
    Set-TestDacl $testRoot $testPlan[0].AfterDacl
    $after = @(Get-CurrentSnapshots $testTargets)
    for ($i = 0; $i -lt $testPlan.Count; $i++) {
        Assert-Check (Test-DaclEquivalent $after[$i].Dacl $testPlan[$i].AfterDacl) "native apply propagated to fixture $i"
        Assert-Check ($after[$i].Owner -eq $testPlan[$i].Owner) "apply preserves fixture $i owner"
        Assert-Check (Test-ReviewedDacl $testPlan[$i].BeforeDacl $testPlan[$i] 'Rollback') "rollback accepts untouched fixture $i"
        Assert-Check (Test-ReviewedDacl $after[$i].Dacl $testPlan[$i] 'Rollback') "rollback accepts changed fixture $i"
        Assert-Check (!(Test-ReviewedDacl ($after[$i].Dacl + '(A;;FR;;;WD)') $testPlan[$i] 'Rollback')) "rollback rejects unrelated changes on fixture $i"
    }
    # A mixed partial-write state is accepted per path, without relaxing either
    # reviewed state to include any unreviewed ACL change.
    Assert-Check ((Test-ReviewedDacl $after[0].Dacl $testPlan[0] 'Rollback') -and
        (Test-ReviewedDacl $before[1].Dacl $testPlan[1] 'Rollback') -and
        (Test-ReviewedDacl $before[2].Dacl $testPlan[2] 'Rollback')) 'mixed partial-apply rollback preflight accepted'
    Set-TestDacl $testRoot $testPlan[0].BeforeDacl
    $restored = @(Get-CurrentSnapshots $testTargets)
    for ($i = 0; $i -lt $testPlan.Count; $i++) {
        Assert-Check (Test-DaclEquivalent $restored[$i].Dacl $testPlan[$i].BeforeDacl) "native rollback restores fixture $i"
        Assert-Check ($restored[$i].Owner -eq $testPlan[$i].Owner) "rollback preserves fixture $i owner"
    }
    $missing = @(Get-CurrentSnapshots @($testRoot, $testApp, (Join-Path $testRoot 'missing-file')))
    Assert-Check ($missing.Count -eq 3 -and $missing[2].SnapshotError) 'failure snapshot retains all paths including read errors'
    $result = [ordered]@{Status='success';CompletedAt=[DateTimeOffset]::Now.ToString('o');TestRoot=$testRoot;ProductionPackageTouched=$false;CheckCount=$checks.Count;Checks=$checks;Before=$before;After=$after;Restored=$restored}
    Write-JsonFile $result (Join-Path $testRoot 'result.json')
    $result | ConvertTo-Json -Depth 8
} catch {
    $failure = $_
    Write-JsonFile @{Status='failed';Error=$failure.Exception.Message;TestRoot=$testRoot;Checks=$checks;CurrentSnapshots=@(Get-CurrentSnapshots $testTargets);ProductionPackageTouched=$false} (Join-Path $testRoot 'result.json')
    throw $failure
}
