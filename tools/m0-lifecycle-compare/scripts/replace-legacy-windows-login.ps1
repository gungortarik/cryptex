# Replace only the exact retained comparison registration reported by the user.
[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$SessionRoot, [switch]$Apply)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$base = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Cryptex-Dev\p0-2'))
$session = [IO.Path]::GetFullPath($SessionRoot)
if (-not $session.StartsWith($base.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'SessionRoot must be under the P0-2 runtime root.' }
if (Test-Path -LiteralPath $session) { throw 'Use a fresh SessionRoot.' }
$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..\..')).Path
$binary = Join-Path $repo 'target\debug\cryptexd.exe'
$oldRoot = Join-Path $base 'compare\mode-a-hkcu'
$expected = $binary + ' lifecycle hold --runtime-root ' + $oldRoot + ' --ready-file ' + (Join-Path $oldRoot 'ready') + ' --stop-file ' + (Join-Path $oldRoot 'stop')
$name = 'CryptexDevP02HkcuRun'
$observerName = 'CryptexDevP02LoginObserver'
$key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Run', [bool]$Apply)
if ($null -eq $key) { throw 'Run key is unavailable.' }
$snapshotPath = $null
try {
    $current = $key.GetValue($name, $null, 'DoNotExpandEnvironmentNames')
    if ($current -cne $expected) { throw 'Current Run command does not exactly match the confirmed legacy comparison. Nothing changed.' }
    if ($null -ne $key.GetValue($observerName, $null)) { throw 'Observer registration is occupied. Nothing changed.' }
    if (-not $Apply) { [ordered]@{ exact_legacy_match = $true; observer_absent = $true; apply = $false } | ConvertTo-Json; return }
    $ready = Join-Path $oldRoot 'ready'
    if (Test-Path -LiteralPath $ready) {
        $oldPid = 0
        if (-not [int]::TryParse([IO.File]::ReadAllText($ready).Trim(), [ref]$oldPid)) { throw 'Legacy ready PID is malformed. Nothing changed.' }
        $oldProcess = $null
        try { $oldProcess = [Diagnostics.Process]::GetProcessById($oldPid) } catch [ArgumentException] { }
        if ($null -ne $oldProcess) {
            try { throw 'Legacy ready PID is still present; inspect it before replacement.' } finally { $oldProcess.Dispose() }
        }
    }
    $backupRoot = Join-Path $base ('registration-transitions\' + [Guid]::NewGuid().ToString('N'))
    [void][IO.Directory]::CreateDirectory($backupRoot)
    $snapshotPath = Join-Path $backupRoot 'legacy-run.json'
    $snapshot = [ordered]@{ observed_utc = [DateTime]::UtcNow.ToString('o'); run_value = $name; command = $current; legacy_runtime = $oldRoot; new_session = $session }
    [IO.File]::WriteAllText($snapshotPath, ($snapshot | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
    if ($key.GetValue($name, $null, 'DoNotExpandEnvironmentNames') -cne $expected -or $null -ne $key.GetValue($observerName, $null)) { throw 'Run registration changed during preparation. Nothing removed.' }
    $key.DeleteValue($name, $false)
    try {
        & (Join-Path $PSScriptRoot 'prepare-windows-login.ps1') -Action Prepare -SessionRoot $session
    } catch {
        # Restore the saved legacy command only if neither slot acquired a new owner.
        if ($null -eq $key.GetValue($name, $null) -and $null -eq $key.GetValue($observerName, $null)) {
            $key.SetValue($name, $current, [Microsoft.Win32.RegistryValueKind]::String)
        }
        throw
    }
    [ordered]@{ result = 'PREPARED_REQUIRES_REAL_LOGIN'; legacy_registration_backup = $snapshotPath; session_root = $session } | ConvertTo-Json
} finally { $key.Dispose() }
