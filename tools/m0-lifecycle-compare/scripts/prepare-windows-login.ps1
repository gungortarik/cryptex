# Reversible P0-2 login experiment. No sign-out, elevation, or policy changes.
[CmdletBinding()]
param(
    [ValidateSet('Prepare', 'Status', 'Cleanup')][string]$Action = 'Prepare',
    [Parameter(Mandatory = $true)][string]$SessionRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$sessionPath = [IO.Path]::GetFullPath($SessionRoot)
$allowedRoot = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Cryptex-Dev\p0-2')) + '\'
if (-not $sessionPath.StartsWith($allowedRoot, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'SessionRoot must be under LOCALAPPDATA\Cryptex-Dev\p0-2.'
}
$runtimePath = Join-Path $sessionPath 'mode-a-hkcu'
$manifestPath = Join-Path $sessionPath 'login-experiment.json'
$daemonValue = 'CryptexDevP02HkcuRun'
$observerValue = 'CryptexDevP02LoginObserver'
$regPath = 'Software\Microsoft\Windows\CurrentVersion\Run'
$key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($regPath, $true)
if ($null -eq $key) { throw 'Current-user Run key is unavailable.' }
$utf8 = New-Object Text.UTF8Encoding($false)

function Read-RunValue([string]$Name) {
    return $key.GetValue($Name, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
}

try {
    if ($Action -eq 'Prepare') {
        if (Test-Path -LiteralPath $sessionPath) { throw 'Use a new SessionRoot; retained artifacts are never overwritten.' }
        if ($null -ne (Read-RunValue $daemonValue) -or $null -ne (Read-RunValue $observerValue)) {
            throw 'A test Run value already exists. Inspect/clean up its owning experiment first.'
        }
        $repoPath = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..\..')).Path
        $harness = Join-Path $repoPath 'target\debug\m0-lifecycle-compare.exe'
        $sourceBinary = Join-Path $repoPath 'target\debug\cryptexd.exe'
        if (-not (Test-Path -LiteralPath $harness) -or -not (Test-Path -LiteralPath $sourceBinary)) { throw 'Build cryptexd and m0-lifecycle-compare first.' }
        [void][IO.Directory]::CreateDirectory($runtimePath)
        $pinnedBinary = Join-Path $runtimePath 'cryptexd.exe'
        $observerScript = Join-Path $runtimePath 'observe-windows-login.ps1'
        Copy-Item -LiteralPath $sourceBinary -Destination $pinnedBinary
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'observe-windows-login.ps1') -Destination $observerScript
        $powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
        $observerCommand = '"' + $powershell + '" -NoProfile -NonInteractive -WindowStyle Hidden -File "' + $observerScript + '"'
        if ($observerCommand.Length -gt 260) { throw 'Observer Run command exceeds 260 characters; use a shorter SessionRoot.' }
        $null = & $pinnedBinary --help
        if ($LASTEXITCODE -ne 0) { throw 'Pinned daemon cannot execute; no login registration was written.' }
        $sourceCommit = (& git -C $repoPath rev-parse HEAD).Trim()
        $sourceBranch = (& git -C $repoPath branch --show-current).Trim()
        $sourceDirty = [bool](& git -C $repoPath status --porcelain)
        $registrationOutput = & $harness hkcu-run register --runtime-root $sessionPath --cryptexd $pinnedBinary 2>&1
        if ($LASTEXITCODE -ne 0) { throw ('Daemon registration failed: ' + ($registrationOutput -join ' ')) }
        $daemonCommand = Read-RunValue $daemonValue
        try {
            $key.SetValue($observerValue, $observerCommand, [Microsoft.Win32.RegistryValueKind]::String)
            $manifest = [ordered]@{
                prepared_utc = [DateTime]::UtcNow.ToString('o'); session_root = $sessionPath; runtime_root = $runtimePath
                source_commit = $sourceCommit; source_branch = $sourceBranch; source_dirty = $sourceDirty
                binary = $pinnedBinary; binary_sha256 = (Get-FileHash -LiteralPath $pinnedBinary -Algorithm SHA256).Hash
                daemon_value = $daemonValue; daemon_command = $daemonCommand
                observer_value = $observerValue; observer_command = $observerCommand
                observer_sha256 = (Get-FileHash -LiteralPath $observerScript -Algorithm SHA256).Hash
                login_proof = 'PENDING_REAL_SIGN_OUT_AND_SIGN_IN'; observation_seconds = 600
            }
            [IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json), $utf8)
        } catch {
            # Only revert entries owned by this failed preparation.
            if ((Read-RunValue $daemonValue) -eq $daemonCommand) { $key.DeleteValue($daemonValue, $false) }
            if ((Read-RunValue $observerValue) -eq $observerCommand) { $key.DeleteValue($observerValue, $false) }
            throw
        }
        if (Test-Path -LiteralPath (Join-Path $runtimePath 'stop')) { throw 'Unexpected stop marker after registration.' }
        Write-Output ($manifest | ConvertTo-Json)
    } else {
        $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
        if ($manifest.session_root -ne $sessionPath -or $manifest.runtime_root -ne $runtimePath -or $manifest.daemon_value -ne $daemonValue -or $manifest.observer_value -ne $observerValue) {
            throw 'Experiment manifest does not match this session.'
        }
        if ($Action -eq 'Cleanup') {
            # Check both values before changing either. Never remove a replacement.
            foreach ($pair in @(@($daemonValue, $manifest.daemon_command), @($observerValue, $manifest.observer_command))) {
                $current = Read-RunValue $pair[0]
                if ($null -ne $current -and $current -ne $pair[1]) { throw ('Run value changed ownership: ' + $pair[0]) }
            }
            $readyPath = Join-Path $runtimePath 'ready'
            if (Test-Path -LiteralPath $readyPath) {
                $daemonId = 0
                if ([int]::TryParse([IO.File]::ReadAllText($readyPath).Trim(), [ref]$daemonId) -and $daemonId -gt 0) {
                    $daemonProcess = $null
                    try { $daemonProcess = [Diagnostics.Process]::GetProcessById($daemonId) }
                    catch [ArgumentException] { } # PID already gone; not a shutdown failure.
                    if ($null -ne $daemonProcess) {
                        try {
                            if (-not [string]::Equals($daemonProcess.MainModule.FileName, $manifest.binary, [StringComparison]::OrdinalIgnoreCase)) { throw 'Ready PID belongs to another executable; cleanup stopped.' }
                            [IO.File]::WriteAllText((Join-Path $runtimePath 'stop'), "stop`n", $utf8)
                            if (-not $daemonProcess.WaitForExit(5000)) { throw 'Experiment daemon did not clean-stop; artifacts and registration retained.' }
                        } finally { $daemonProcess.Dispose() }
                    }
                }
            }
            $key.DeleteValue($daemonValue, $false)
            $key.DeleteValue($observerValue, $false)
            $stopPath = Join-Path $runtimePath 'stop'
            if (Test-Path -LiteralPath $stopPath) { Remove-Item -LiteralPath $stopPath }
        }
        $results = @()
        $observations = Join-Path $runtimePath 'observations'
        if (Test-Path -LiteralPath $observations) {
            $results = @(Get-ChildItem -LiteralPath $observations -Directory | ForEach-Object {
                $resultPath = Join-Path $_.FullName 'result.json'
                if (Test-Path -LiteralPath $resultPath) { Get-Content -LiteralPath $resultPath -Raw | ConvertFrom-Json }
            })
        }
        [ordered]@{
            action = $Action; session_root = $sessionPath; runtime_root = $runtimePath
            daemon_registered = ((Read-RunValue $daemonValue) -eq $manifest.daemon_command)
            observer_registered = ((Read-RunValue $observerValue) -eq $manifest.observer_command)
            stop_present = (Test-Path -LiteralPath (Join-Path $runtimePath 'stop'))
            observations = $results
        } | ConvertTo-Json -Depth 8
    }
} finally { $key.Dispose() }
