[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$SessionRoot)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'get-windows-process-context.ps1')
$processContext = Get-WindowsProcessContext
$sessionPath = [IO.Path]::GetFullPath($SessionRoot)
$allowedPrefix = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Cryptex-Dev\p0-2')).TrimEnd('\') + '\'
if (-not $sessionPath.StartsWith($allowedPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'SessionRoot must be under LOCALAPPDATA\Cryptex-Dev\p0-2.'
}
$manifest = Get-Content -LiteralPath (Join-Path $sessionPath 'run-dispatch.json') -Raw | ConvertFrom-Json
if ($manifest.session_root -ne $sessionPath -or $manifest.run_value -ne 'CryptexDevP02RunDispatch' -or
    $manifest.marker_script -ne (Join-Path $sessionPath 'write-marker.ps1') -or
    $manifest.marker_path -ne (Join-Path $sessionPath 'run-dispatched.json')) {
    throw 'Manifest does not match this dispatch experiment.'
}
$errors = [Collections.Generic.List[object]]::new()
$observed = [DateTime]::UtcNow
$sessionId = [Diagnostics.Process]::GetCurrentProcess().SessionId
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
try { $userSid = $identity.User.Value } finally { $identity.Dispose() }
$registry = @()
foreach ($viewName in @('Registry64', 'Registry32')) {
    $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey('CurrentUser', $viewName)
    try {
        $run = $base.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Run', $false)
        $command = $null
        if ($null -ne $run) {
            try { $command = $run.GetValue($manifest.run_value, $null, 'DoNotExpandEnvironmentNames') }
            finally { $run.Dispose() }
        }
        $approved = $base.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run', $false)
        $approvalBytes = $null
        if ($null -ne $approved) {
            try { $approvalBytes = $approved.GetValue($manifest.run_value, $null) }
            finally { $approved.Dispose() }
        }
        $registry += [ordered]@{
            view = $viewName; present = ($null -ne $command)
            matches_manifest = ($command -eq $manifest.run_command)
            command_length = if ($null -eq $command) { $null } else { $command.Length }
            startup_approved_base64 = if ($null -eq $approvalBytes) { $null } else { [Convert]::ToBase64String($approvalBytes) }
        }
    } finally { $base.Dispose() }
}
$policies = @()
foreach ($hive in @('CurrentUser', 'LocalMachine')) {
    $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey($hive, 'Registry64')
    try {
        $key = $base.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Policies\Explorer', $false)
        try {
            $policies += [ordered]@{
                hive = $hive
                disable_current_user_run = if ($null -eq $key) { $null } else { $key.GetValue('DisableCurrentUserRun', $null) }
                disable_local_machine_run = if ($null -eq $key) { $null } else { $key.GetValue('DisableLocalMachineRun', $null) }
            }
        } finally { if ($null -ne $key) { $key.Dispose() } }
    } finally { $base.Dispose() }
}
$explorers = @()
try {
    $explorers = @(Get-CimInstance Win32_Process -Filter "Name = 'explorer.exe'" |
        Where-Object { $_.SessionId -eq $sessionId } | ForEach-Object {
            [ordered]@{ pid = $_.ProcessId; session_id = $_.SessionId; created_utc = $_.CreationDate.ToUniversalTime().ToString('o') }
        })
} catch { $errors.Add(@{ source = 'explorer'; error = $_.Exception.Message }) }
$prepared = [DateTime]::Parse($manifest.prepared_utc).ToUniversalTime()
$events = @()
try {
    # Retain only Run enumeration and this experiment's launch events, not
    # unrelated startup application command lines or entire event messages.
    # Use explicit UTC in XPath; Windows PowerShell's hashtable time filter
    # can interpret DateTime boundaries differently from PowerShell 7.
    $startUtc = $prepared.ToString('yyyy-MM-ddTHH:mm:ss.fffffffZ')
    $endUtc = $observed.ToString('yyyy-MM-ddTHH:mm:ss.fffffffZ')
    $query = '<QueryList><Query Id="0" Path="Microsoft-Windows-Shell-Core/Operational"><Select Path="Microsoft-Windows-Shell-Core/Operational">*[System[(EventID=9705 or EventID=9706 or EventID=9707 or EventID=9708) and TimeCreated[@SystemTime&gt;="' + $startUtc + '" and @SystemTime&lt;="' + $endUtc + '"]]]</Select></Query></QueryList>'
    $rawEvents = @(Get-WinEvent -FilterXml $query -ErrorAction Stop)
    foreach ($event in $rawEvents) {
        $xml = [xml]$event.ToXml()
        $fields = @($xml.Event.EventData.Data | ForEach-Object { $_.InnerText })
        $related = @($fields | Where-Object { $null -ne $_ -and ($_.Contains($manifest.run_value) -or $_.Contains($manifest.marker_script) -or $_.Contains($sessionPath)) }).Count -gt 0
        if ($event.Id -in @(9705, 9706) -or $related) {
            $events += [ordered]@{
                id = $event.Id; record_id = $event.RecordId
                utc = $event.TimeCreated.ToUniversalTime().ToString('o')
                experiment_launch = $related
                registry_key = if ($event.Id -in @(9705, 9706)) { $fields } else { @() }
                experiment_fields = if ($related) { $fields } else { @() }
            }
        }
    }
} catch {
    if ($_.FullyQualifiedErrorId -like 'NoMatchingEventsFound*') {
        # Successful query with no matching events is distinct from unavailable log.
    } else { $errors.Add(@{ source = 'shell_core'; error = $_.Exception.Message }) }
}
$marker = $null
if (Test-Path -LiteralPath $manifest.marker_path) {
    $marker = Get-Content -LiteralPath $manifest.marker_path -Raw | ConvertFrom-Json
}
$report = [ordered]@{
    experiment = 'm0-p0-2-run-dispatch-observation'; observed_utc = $observed.ToString('o')
    observer_pid = $PID; observer_session_id = $sessionId; observer_user_sid = $userSid
    observer_process_bits = [IntPtr]::Size * 8
    process_context = $processContext
    session_root = $sessionPath; prepared_utc = $manifest.prepared_utc
    marker_script_hash_matches = ((Get-FileHash -LiteralPath $manifest.marker_script -Algorithm SHA256).Hash -eq $manifest.marker_script_sha256)
    registry = $registry; policies = $policies; explorers = $explorers
    execution_policy = @(Get-ExecutionPolicy -List | ForEach-Object { @{ scope = $_.Scope.ToString(); policy = $_.ExecutionPolicy.ToString() } })
    shell_events = $events; marker_present = ($null -ne $marker); marker = $marker
    collection_errors = @($errors)
    classification = if ($null -ne $marker) { 'MARKER_PRESENT_REQUIRES_LOGIN_CORRELATION' } else { 'DISPATCH_NOT_OBSERVED_CAUSE_OPEN' }
}
$observationRoot = Join-Path $sessionPath 'dispatch-observations'
[void][IO.Directory]::CreateDirectory($observationRoot)
$outputPath = Join-Path $observationRoot ($observed.ToString('yyyyMMddTHHmmssfffffffZ') + '-' + [Guid]::NewGuid().ToString('N') + '.json')
[IO.File]::WriteAllText($outputPath, ($report | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
[ordered]@{
    evidence_path = $outputPath; classification = $report.classification
    collection_error_count = $errors.Count; marker_present = $report.marker_present
    run_matches_both_views = (@($registry | Where-Object { -not $_.matches_manifest }).Count -eq 0)
    packaged_process = $processContext.packaged_process
} | ConvertTo-Json
