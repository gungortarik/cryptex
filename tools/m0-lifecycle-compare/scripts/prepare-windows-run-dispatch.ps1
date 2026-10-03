[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][ValidateSet('Prepare', 'Status', 'Cleanup')][string]$Action,
    [Parameter(Mandatory = $true)][string]$SessionRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$allowedRoot = Join-Path $env:LOCALAPPDATA 'Cryptex-Dev\p0-2'
$sessionPath = [IO.Path]::GetFullPath($SessionRoot)
$allowedPrefix = [IO.Path]::GetFullPath($allowedRoot).TrimEnd('\') + '\'
if (-not $sessionPath.StartsWith($allowedPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'SessionRoot must be under LOCALAPPDATA\Cryptex-Dev\p0-2.'
}
$valueName = 'CryptexDevP02RunDispatch'
$regPath = 'Software\Microsoft\Windows\CurrentVersion\Run'
$manifestPath = Join-Path $sessionPath 'run-dispatch.json'
$markerPath = Join-Path $sessionPath 'run-dispatched.json'
$key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($regPath, $true)
if ($null -eq $key) { throw 'Current-user Run key is unavailable.' }

function Read-RunValue([string]$Name) {
    return $key.GetValue($Name, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
}

try {
    if ($Action -eq 'Prepare') {
        if (Test-Path -LiteralPath $sessionPath) { throw 'Use a new SessionRoot; retained artifacts are never overwritten.' }
        if ($null -ne (Read-RunValue $valueName)) { throw 'The test Run value is already occupied.' }
        [void][IO.Directory]::CreateDirectory($sessionPath)
        $markerScript = Join-Path $sessionPath 'write-marker.ps1'
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'write-windows-run-dispatch-marker.ps1') -Destination $markerScript
        $powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
        $command = '"' + $powershell + '" -NoProfile -NonInteractive -WindowStyle Hidden -File "' + $markerScript + '"'
        if ($command.Length -gt 260) { throw 'Run command exceeds the documented 260-character limit.' }

        # Validate the marker independently before arming logon startup.
        & $powershell -NoProfile -NonInteractive -File $markerScript
        if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $markerPath)) { throw 'Marker self-check failed; Run registration was not written.' }
        $selfCheck = Get-Content -LiteralPath $markerPath -Raw | ConvertFrom-Json
        Remove-Item -LiteralPath $markerPath

        $approvedPath = 'Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run'
        $approvedKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($approvedPath, $false)
        $approvedBytes = $null
        if ($null -ne $approvedKey) {
            try { $approvedBytes = $approvedKey.GetValue($valueName, $null) } finally { $approvedKey.Dispose() }
        }
        $manifest = [ordered]@{
            prepared_utc = [DateTime]::UtcNow.ToString('o')
            session_root = $sessionPath
            marker_path = $markerPath
            marker_script = $markerScript
            marker_script_sha256 = (Get-FileHash -LiteralPath $markerScript -Algorithm SHA256).Hash
            run_value = $valueName
            run_command = $command
            run_command_length = $command.Length
            startup_approved_before = if ($null -eq $approvedBytes) { $null } else { [Convert]::ToBase64String($approvedBytes) }
            self_check = $selfCheck
            login_proof = 'PENDING_REAL_SIGN_OUT_AND_SIGN_IN'
        }
        try {
            $key.SetValue($valueName, $command, [Microsoft.Win32.RegistryValueKind]::String)
            [IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 5), (New-Object Text.UTF8Encoding($false)))
        } catch {
            if ((Read-RunValue $valueName) -eq $command) { $key.DeleteValue($valueName, $false) }
            throw
        }
        $manifest | ConvertTo-Json -Depth 5
    } else {
        if (-not (Test-Path -LiteralPath $manifestPath)) { throw 'Run-dispatch manifest is missing.' }
        $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
        if ($manifest.session_root -ne $sessionPath -or $manifest.run_value -ne $valueName -or $manifest.marker_path -ne $markerPath) {
            throw 'Manifest does not match this session.'
        }
        if ($Action -eq 'Cleanup') {
            $current = Read-RunValue $valueName
            if ($null -ne $current -and $current -ne $manifest.run_command) { throw 'Run value ownership changed; cleanup stopped.' }
            $key.DeleteValue($valueName, $false)
        }
        $current = Read-RunValue $valueName
        $matchesManifest = ($current -eq $manifest.run_command)
        [ordered]@{
            action = $Action
            run_registered = $matchesManifest
            run_value_present = ($null -ne $current)
            run_value_matches_manifest = $matchesManifest
            run_command_length = if ($null -eq $current) { $null } else { ([string]$current).Length }
            marker_present = (Test-Path -LiteralPath $markerPath)
            marker = if (Test-Path -LiteralPath $markerPath) { Get-Content -LiteralPath $markerPath -Raw | ConvertFrom-Json } else { $null }
            session_root = $sessionPath
        } | ConvertTo-Json -Depth 5
    }
} finally { $key.Dispose() }
