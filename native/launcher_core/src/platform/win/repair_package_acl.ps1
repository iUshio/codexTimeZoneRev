param(
    [ValidateSet('Plan','Apply','Rollback')][string]$Mode = 'Plan',
    [string]$PlanPath,
    [string]$PackageFullName,
    [switch]$FunctionsOnly,
    [switch]$RequireCurrent
)
$ErrorActionPreference = 'Stop'
$expectedPackage = $PackageFullName
$expectedFamily = 'OpenAI.Codex_2p2nqsd0c76g0'
$packageRoot = $null
$resultPath = $null
$accessSection = [Security.AccessControl.AccessControlSections]::Access
$targets = @()
$writeAttempted = $false

function Write-JsonFile($Value, [string]$Path) {
    [IO.File]::WriteAllText([IO.Path]::GetFullPath($Path), ($Value | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
}
function Assert-PackageName([string]$Name) {
    if ($Name -cnotmatch '^OpenAI\.Codex_(\d+\.\d+\.\d+\.\d+)_(x64|arm64)__2p2nqsd0c76g0$') {
        throw 'Refusing an unexpected package name or path.'
    }
    return [version]$Matches[1]
}
function Get-Snapshot([string]$Path) {
    $acl = Get-Acl -LiteralPath $Path
    [pscustomobject]@{Path=$Path;Sddl=$acl.Sddl;Owner=$acl.Owner;Dacl=$acl.GetSecurityDescriptorSddlForm($accessSection)}
}
function Get-DaclComparisonKey([string]$Sddl) {
    # Compare only the DACL and its control flags, not owner/group/SACL fields.
    $descriptor = [Security.AccessControl.RawSecurityDescriptor]::new($Sddl)
    $raw = [Security.AccessControl.RawSecurityDescriptor]::new($descriptor.GetSddlForm($accessSection))
    if ($null -eq $raw.DiscretionaryAcl) { return "$([int]$raw.ControlFlags)|null" }
    $allAllow = $true
    $keys = [Collections.Generic.List[string]]::new()
    foreach ($ace in $raw.DiscretionaryAcl) {
        if ($ace -isnot [Security.AccessControl.QualifiedAce] -or
            $ace.AceQualifier -ne [Security.AccessControl.AceQualifier]::AccessAllowed) {
            $allAllow = $false
        }
        $bytes = [byte[]]::new($ace.BinaryLength)
        $ace.GetBinaryForm($bytes, 0)
        $keys.Add([Convert]::ToBase64String($bytes))
    }
    $orderedKeys = $keys.ToArray()
    # Only all-allow lists are order independent. Keep duplicates, callback
    # condition bytes, masks and inheritance flags; deny/unknown ACEs stay ordered.
    if ($allAllow) { [Array]::Sort($orderedKeys, [StringComparer]::Ordinal) }
    "$([int]$raw.ControlFlags)|$($raw.DiscretionaryAcl.Revision)|$allAllow|$($orderedKeys -join ';')"
}
function Test-DaclEquivalent([string]$Left, [string]$Right) {
    (Get-DaclComparisonKey $Left) -ceq (Get-DaclComparisonKey $Right)
}
function Test-ReviewedDacl([string]$Current, $Snapshot, [string]$Operation) {
    if (Test-DaclEquivalent $Current $Snapshot.BeforeDacl) { return $true }
    $Operation -eq 'Rollback' -and (Test-DaclEquivalent $Current $Snapshot.AfterDacl)
}
function Get-CurrentSnapshots([string[]]$Paths) {
    foreach ($path in $Paths) {
        try { Get-Snapshot $path }
        catch { [pscustomobject]@{Path=$path;SnapshotError=$_.Exception.Message} }
    }
}
function Assert-ReviewedSnapshot($Snapshot, [bool]$OnlyExplicit) {
    $before = [Security.AccessControl.RawSecurityDescriptor]::new($Snapshot.BeforeSddl)
    if (!(Test-DaclEquivalent $before.GetSddlForm($accessSection) $Snapshot.BeforeDacl)) {
        throw 'Plan BeforeSddl and BeforeDacl are inconsistent.'
    }
    if (!(Test-DaclEquivalent (Get-NarrowedDacl $Snapshot.BeforeSddl $OnlyExplicit) $Snapshot.AfterDacl)) {
        throw 'Plan does not describe the single approved mask change.'
    }
}
function Get-NarrowedDacl([string]$Sddl, [bool]$OnlyExplicit) {
    $raw = [Security.AccessControl.RawSecurityDescriptor]::new($Sddl)
    $matches = 0
    foreach ($ace in $raw.DiscretionaryAcl) {
        if ($ace -is [Security.AccessControl.CommonAce] -and !$ace.IsCallback -and
            $ace.AceQualifier -eq [Security.AccessControl.AceQualifier]::AccessAllowed -and
            $ace.SecurityIdentifier.Value -eq 'S-1-5-32-545' -and
            $ace.AccessMask -eq 0x1200a9 -and (!$OnlyExplicit -or !$ace.IsInherited)) {
            $ace.AccessMask = 0x120089 # FILE_GENERIC_READ: remove only FILE_EXECUTE.
            $matches++
        }
    }
    if ($matches -ne 1) { throw "Expected exactly one unconditional Users RX rule at this boundary; found $matches." }
    if (!$raw.GetSddlForm($accessSection).Contains(('WIN://SYSAPPID Contains "' + $expectedFamily + '"'))) {
        throw 'Expected conditional package identity rule is missing.'
    }
    $raw.GetSddlForm($accessSection)
}

# The self-test imports helpers without reading or changing the installed package.
if ($FunctionsOnly) { return }

try {
    # Resolve as the interactive package-owning user. Never infer a directory by
    # scanning WindowsApps or follow a stale plan into a different package.
    $registered = @(Get-AppxPackage -Name OpenAI.Codex | Where-Object PackageFamilyName -eq $expectedFamily | Sort-Object Version -Descending)
    if ($Mode -eq 'Plan') {
        if (!$expectedPackage -and $registered.Count) { $expectedPackage = $registered[0].PackageFullName }
        if (!$PlanPath) { $PlanPath = Join-Path $PSScriptRoot ('codex-acl-' + $expectedPackage + '-' + [guid]::NewGuid().ToString('N') + '.json') }
    } else {
        if (!$PlanPath -or !(Test-Path -LiteralPath $PlanPath -PathType Leaf)) { throw 'An existing reviewed plan is required.' }
        $savedPlan = Get-Content -Raw -LiteralPath $PlanPath -Encoding UTF8 | ConvertFrom-Json
        if ($expectedPackage -and $expectedPackage -cne $savedPlan.PackageFullName) { throw 'Package differs from the saved plan.' }
        $expectedPackage = $savedPlan.PackageFullName
    }
    $expectedVersion = Assert-PackageName $expectedPackage
    if ($RequireCurrent -and ($registered.Count -eq 0 -or $registered[0].PackageFullName -cne $expectedPackage)) {
        throw 'The installed version changed. Check the current client again before repairing.'
    }
    $packageRoot = Join-Path $env:ProgramFiles ('WindowsApps\' + $expectedPackage)
    $registration = @($registered | Where-Object PackageFullName -ceq $expectedPackage)
    if ($registration.Count -ne 1 -or $registration[0].InstallLocation -ine $packageRoot) { throw 'The exact package is not registered for this user at the expected location.' }
    $resultPath = [IO.Path]::GetFullPath($PlanPath) + '.' + $Mode.ToLowerInvariant() + '-result.json'
    $targets = @($packageRoot, (Join-Path $packageRoot 'app'), (Join-Path $packageRoot 'app\ChatGPT.exe'))
    [xml]$manifest = Get-Content -Raw -LiteralPath (Join-Path $packageRoot 'AppxManifest.xml') -Encoding UTF8
    if ($manifest.Package.Identity.Name -ne 'OpenAI.Codex' -or [version]$manifest.Package.Identity.Version -ne $expectedVersion) {
        throw 'Installed package changed; regenerate a reviewed plan for the new version.'
    }
    if ($Mode -eq 'Plan') {
        if (Test-Path -LiteralPath $PlanPath) { throw 'The existing ACL backup will not be overwritten. Review it before creating a new plan.' }
        $snapshots = @(foreach ($target in $targets) {
            $snapshot = Get-Snapshot $target
            $proposed = Get-NarrowedDacl $snapshot.Sddl ($target -eq $packageRoot)
            [pscustomobject]@{Path=$snapshot.Path;Owner=$snapshot.Owner;BeforeSddl=$snapshot.Sddl;BeforeDacl=$snapshot.Dacl;AfterDacl=$proposed}
        })
        $plan = [ordered]@{
            CreatedAt=[DateTimeOffset]::Now.ToString('o');PackageFullName=$expectedPackage;PackageRoot=$packageRoot
            Change='Narrow one explicit inheritable unconditional BUILTIN\Users rule from RX (0x1200a9) to read (0x120089). Preserve conditional package execution and all other ACEs/owner. Let Windows propagate inheritance only within this package.'
            Snapshots=$snapshots
        }
        Write-JsonFile $plan $PlanPath
        Write-JsonFile @{Status='planned';PlanPath=[IO.Path]::GetFullPath($PlanPath);Changed=$false} $resultPath
        $plan | ConvertTo-Json -Depth 8
        exit 0
    }
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    if (!([Security.Principal.WindowsPrincipal]::new($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Administrator confirmation is required to repair this protected package DACL; no ownership change will be made.'
    }
    $plan = Get-Content -Raw -LiteralPath $PlanPath -Encoding UTF8 | ConvertFrom-Json
    if ($plan.PackageFullName -ne $expectedPackage -or $plan.PackageRoot -ne $packageRoot -or $plan.Snapshots.Count -ne 3) {
        throw 'Plan scope does not match the reviewed Codex package.'
    }
    for ($index=0; $index -lt $targets.Count; $index++) {
        $snapshot = $plan.Snapshots[$index]
        if ($snapshot.Path -ne $targets[$index]) { throw 'Unexpected path in repair plan.' }
        Assert-ReviewedSnapshot $snapshot ($index -eq 0)
        $current = Get-Snapshot $targets[$index]
        if (!(Test-ReviewedDacl $current.Dacl $snapshot $Mode) -or $current.Owner -ne $snapshot.Owner) { throw "ACL changed since plan: $($targets[$index]); refusing to overwrite it." }
        if ($Mode -eq 'Apply' -and !(Test-DaclEquivalent (Get-NarrowedDacl $current.Sddl ($index -eq 0)) $snapshot.AfterDacl)) {
            throw 'Current ACL does not produce the reviewed single mask change.'
        }
    }

    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class CodexPackageDaclRepair {
    [StructLayout(LayoutKind.Sequential)] struct LUID { public uint Low; public int High; }
    [StructLayout(LayoutKind.Sequential)] struct PRIVILEGE { public uint Count; public LUID Luid; public uint Attributes; }
    [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool CloseHandle(IntPtr h);
    [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr h);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool OpenProcessToken(IntPtr p, uint a, out IntPtr token);
    [DllImport("advapi32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool LookupPrivilegeValueW(string s, string n, out LUID luid);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool AdjustTokenPrivileges(IntPtr t, bool all, ref PRIVILEGE p, uint len, IntPtr prev, IntPtr needed);
    [DllImport("advapi32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool ConvertStringSecurityDescriptorToSecurityDescriptorW(string s, uint rev, out IntPtr sd, out uint len);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool GetSecurityDescriptorDacl(IntPtr sd, out bool present, out IntPtr acl, out bool defaulted);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern IntPtr CreateFileW(string p, uint a, uint share, IntPtr sa, uint creation, uint flags, IntPtr template);
    [DllImport("advapi32.dll")] static extern uint SetSecurityInfo(IntPtr handle, uint type, uint info, IntPtr owner, IntPtr group, IntPtr dacl, IntPtr sacl);
    static void EnablePrivilege(string name) {
        IntPtr token;
        if (!OpenProcessToken(GetCurrentProcess(), 0x28, out token)) throw new System.ComponentModel.Win32Exception();
        try {
            LUID luid;
            if (!LookupPrivilegeValueW(null, name, out luid)) throw new System.ComponentModel.Win32Exception();
            PRIVILEGE p = new PRIVILEGE { Count=1, Luid=luid, Attributes=2 };
            if (!AdjustTokenPrivileges(token, false, ref p, 0, IntPtr.Zero, IntPtr.Zero)) throw new System.ComponentModel.Win32Exception();
            int error = Marshal.GetLastWin32Error();
            if (error != 0) throw new System.ComponentModel.Win32Exception(error);
        } finally { CloseHandle(token); }
    }
    public static void SetDirectoryDacl(string path, string sddl) {
        // Enable only on this short-lived elevated repair process. No owner,
        // system policy, registry or persistent privilege changes are made.
        EnablePrivilege("SeBackupPrivilege"); EnablePrivilege("SeRestorePrivilege");
        IntPtr sd; uint length;
        if (!ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl, 1, out sd, out length)) throw new System.ComponentModel.Win32Exception();
        try {
            bool present, defaulted; IntPtr acl;
            if (!GetSecurityDescriptorDacl(sd, out present, out acl, out defaulted) || !present || acl == IntPtr.Zero) throw new InvalidOperationException("Valid non-null DACL required.");
            // READ_CONTROL | WRITE_DAC, FILE_FLAG_BACKUP_SEMANTICS on exact directory.
            IntPtr handle = CreateFileW(path, 0x60000, 7, IntPtr.Zero, 3, 0x02000000, IntPtr.Zero);
            if (handle == new IntPtr(-1)) throw new System.ComponentModel.Win32Exception();
            try {
                uint error = SetSecurityInfo(handle, 1, 4, IntPtr.Zero, IntPtr.Zero, acl, IntPtr.Zero);
                if (error != 0) throw new System.ComponentModel.Win32Exception((int)error);
            } finally { CloseHandle(handle); }
        } finally { LocalFree(sd); }
    }
}
'@
    $desired = if ($Mode -eq 'Apply') { $plan.Snapshots[0].AfterDacl } else { $plan.Snapshots[0].BeforeDacl }
    $writeAttempted = $true
    [CodexPackageDaclRepair]::SetDirectoryDacl($packageRoot, $desired)
    $verified = @()
    foreach ($snapshot in $plan.Snapshots) {
        $current = Get-Snapshot $snapshot.Path
        $expected = if ($Mode -eq 'Apply') { $snapshot.AfterDacl } else { $snapshot.BeforeDacl }
        if (!(Test-DaclEquivalent $current.Dacl $expected) -or $current.Owner -ne $snapshot.Owner) { throw "Post-write verification failed: $($snapshot.Path). Backup is preserved; inspect before rollback." }
        $verified += $current
    }
    Write-JsonFile @{Status='success';Mode=$Mode;CompletedAt=[DateTimeOffset]::Now.ToString('o');Verified=$verified;PlanPath=$PlanPath} $resultPath
} catch {
    $failure = $_
    $currentSnapshots = @(Get-CurrentSnapshots $targets)
    if ($resultPath) { Write-JsonFile @{Status='failed';Mode=$Mode;Error=$failure.Exception.Message;WriteAttempted=$writeAttempted;CurrentSnapshots=$currentSnapshots;RecordedAt=[DateTimeOffset]::Now.ToString('o');PlanPath=$PlanPath} $resultPath }
    throw $failure
}
