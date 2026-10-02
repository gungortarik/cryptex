# P0-2 test-only observer. Never launches, stops, or restarts the daemon.
# HKCU Run launches cryptexd directly; this independent process observes it.
[CmdletBinding()]
param(
    [string]$RuntimeRoot = '',
    [string]$Cryptexd = '',
    [ValidateRange(1, 1800)][int]$MaxSeconds = 600,
    [ValidateRange(1, 600)][int]$StartTimeoutSeconds = 180
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ([string]::IsNullOrWhiteSpace($RuntimeRoot)) { $RuntimeRoot = $PSScriptRoot }
if ([string]::IsNullOrWhiteSpace($Cryptexd)) { $Cryptexd = Join-Path $PSScriptRoot 'cryptexd.exe' }
$runtimePath = [IO.Path]::GetFullPath($RuntimeRoot)
$allowedRoot = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Cryptex-Dev\p0-2')) + '\'
if (-not $runtimePath.StartsWith($allowedRoot, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Observer runtime must be under LOCALAPPDATA\Cryptex-Dev\p0-2.'
}
$expectedBinary = [IO.Path]::GetFullPath($Cryptexd)
$startedUtc = [DateTime]::UtcNow
$observationId = $startedUtc.ToString('yyyyMMddTHHmmssfffZ') + '-' + [Guid]::NewGuid().ToString('N').Substring(0, 8)
$outputRoot = Join-Path $runtimePath ('observations\' + $observationId)
[void][IO.Directory]::CreateDirectory($outputRoot)
$utf8 = New-Object Text.UTF8Encoding($false)
$logPath = Join-Path $outputRoot 'observer.jsonl'
$daemonProcess = $null
$outcome = 'observer_error'
$exitCode = $null
$exitHex = $null
$observerExit = 0

function Write-Observation([string]$Event, [hashtable]$Fields = @{}) {
    $record = [ordered]@{ utc = [DateTime]::UtcNow.ToString('o'); event = $Event; observer_pid = $PID }
    foreach ($key in $Fields.Keys) { $record[$key] = $Fields[$key] }
    [IO.File]::AppendAllText($logPath, (($record | ConvertTo-Json -Compress -Depth 6) + "`n"), $utf8)
}

function Save-ControlSnapshot([string]$Phase) {
    foreach ($name in @('ready', 'daemon.lock', 'lifecycle.status', 'stop')) {
        try {
            $path = Join-Path $runtimePath $name
            if ([IO.File]::Exists($path)) {
                $info = Get-Item -LiteralPath $path
                Write-Observation 'control_file' @{ phase = $Phase; name = $name; present = $true; modified_utc = $info.LastWriteTimeUtc.ToString('o'); bytes = $info.Length }
                if ($info.Length -le 16384) {
                    [IO.File]::WriteAllText((Join-Path $outputRoot ($Phase + '-' + $name + '.txt')), [IO.File]::ReadAllText($path), $utf8)
                }
            } else {
                Write-Observation 'control_file' @{ phase = $Phase; name = $name; present = $false }
            }
        } catch {
            # Reading the held daemon lock is expected to fail on Windows.
            # Record the unavailable snapshot without interrupting observation.
            Write-Observation 'control_file_unavailable' @{ phase = $Phase; name = $name; error = $_.Exception.Message }
        }
    }
}

try {
    Write-Observation 'observer_started' @{ runtime_root = $runtimePath; expected_binary = $expectedBinary; max_seconds = $MaxSeconds; start_timeout_seconds = $StartTimeoutSeconds }
    Save-ControlSnapshot 'initial'
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $lastHeartbeat = -5.0
    $lastMissingPid = 0
    while ($clock.Elapsed.TotalSeconds -lt $MaxSeconds) {
        if ($null -eq $daemonProcess) {
            $readyPath = Join-Path $runtimePath 'ready'
            if ([IO.File]::Exists($readyPath)) {
                $daemonId = 0
                if ([int]::TryParse([IO.File]::ReadAllText($readyPath).Trim(), [ref]$daemonId) -and $daemonId -gt 0) {
                    $candidate = $null
                    try {
                        $candidate = [Diagnostics.Process]::GetProcessById($daemonId)
                        # Open and retain a process handle before exit so ExitCode remains available.
                        $null = $candidate.Handle
                        $actualBinary = $candidate.MainModule.FileName
                        $processStartedUtc = $candidate.StartTime.ToUniversalTime()
                        $readyWrittenUtc = (Get-Item -LiteralPath $readyPath).LastWriteTimeUtc
                        if (-not [string]::Equals($actualBinary, $expectedBinary, [StringComparison]::OrdinalIgnoreCase) -or $processStartedUtc -gt $readyWrittenUtc.AddSeconds(2)) {
                            $candidate.Dispose()
                            $candidate = $null
                            $outcome = 'pid_identity_mismatch'
                            Write-Observation $outcome @{ daemon_pid = $daemonId }
                            break
                        }
                        $daemonProcess = $candidate
                        Write-Observation 'attached' @{ daemon_pid = $daemonId; started_utc = $processStartedUtc.ToString('o'); binary = $actualBinary }
                        Save-ControlSnapshot 'attached'
                    } catch {
                        if ($null -ne $candidate) { $candidate.Dispose() }
                        if ($lastMissingPid -ne $daemonId) {
                            Write-Observation 'attach_failed' @{ daemon_pid = $daemonId; error = $_.Exception.Message }
                            $lastMissingPid = $daemonId
                        }
                    }
                }
            }
            if ($null -eq $daemonProcess -and $clock.Elapsed.TotalSeconds -ge $StartTimeoutSeconds) {
                $outcome = 'not_attached_before_timeout'
                break
            }
            Start-Sleep -Milliseconds 100
        } else {
            if ($daemonProcess.WaitForExit(1000)) {
                $exitCode = $daemonProcess.ExitCode
                $unsignedExit = [BitConverter]::ToUInt32([BitConverter]::GetBytes([int]$exitCode), 0)
                $exitHex = ('0x{0:X8}' -f $unsignedExit)
                $outcome = 'process_exited'
                Write-Observation $outcome @{ daemon_pid = $daemonProcess.Id; exit_code = $exitCode; exit_hex = $exitHex; stop_present = [IO.File]::Exists((Join-Path $runtimePath 'stop')) }
                break
            }
        }
        if ($clock.Elapsed.TotalSeconds - $lastHeartbeat -ge 5) {
            Write-Observation 'heartbeat' @{ attached = ($null -ne $daemonProcess); elapsed_seconds = [Math]::Round($clock.Elapsed.TotalSeconds, 2); stop_present = [IO.File]::Exists((Join-Path $runtimePath 'stop')) }
            $lastHeartbeat = $clock.Elapsed.TotalSeconds
        }
    }
    if ($outcome -eq 'observer_error') {
        if ($null -eq $daemonProcess) { $outcome = 'not_attached_before_timeout' }
        else { $outcome = 'alive_at_observation_deadline' }
    }
} catch {
    $outcome = 'observer_error'
    $observerExit = 2
    Write-Observation 'observer_error' @{ error = $_.Exception.Message }
} finally {
    Save-ControlSnapshot 'final'
    # Do not call lifecycle status: preserve the original persisted daemon report.
    try {
        $events = @(Get-WinEvent -FilterHashtable @{ LogName = 'Microsoft-Windows-CodeIntegrity/Operational'; StartTime = $startedUtc.AddMinutes(-5) } -ErrorAction Stop |
            Where-Object { $_.Message -match 'cryptexd\.exe|observe-windows-login\.ps1' } |
            ForEach-Object { [ordered]@{ utc = $_.TimeCreated.ToUniversalTime().ToString('o'); id = $_.Id; message = $_.Message; xml = $_.ToXml() } })
        [IO.File]::WriteAllText((Join-Path $outputRoot 'code-integrity.json'), (ConvertTo-Json -InputObject $events -Depth 6), $utf8)
        Write-Observation 'code_integrity_collected' @{ count = $events.Count }
    } catch {
        if ($_.FullyQualifiedErrorId -like 'NoMatchingEventsFound*') {
            [IO.File]::WriteAllText((Join-Path $outputRoot 'code-integrity.json'), '[]', $utf8)
            Write-Observation 'code_integrity_collected' @{ count = 0 }
        } else {
            Write-Observation 'code_integrity_unavailable' @{ error = $_.Exception.Message }
        }
    }
    $result = [ordered]@{ outcome = $outcome; exit_code = $exitCode; exit_hex = $exitHex; started_utc = $startedUtc.ToString('o'); ended_utc = [DateTime]::UtcNow.ToString('o'); output_root = $outputRoot; login_proof = 'NOT_ESTABLISHED_BY_OBSERVER_ALONE' }
    [IO.File]::WriteAllText((Join-Path $outputRoot 'result.json'), ($result | ConvertTo-Json), $utf8)
    if ($null -ne $daemonProcess) { $daemonProcess.Dispose() }
    Write-Output ($result | ConvertTo-Json -Compress)
}
exit $observerExit
