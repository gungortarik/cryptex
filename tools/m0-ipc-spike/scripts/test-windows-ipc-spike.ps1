[CmdletBinding()]
param([string]$Binary = '')

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ([string]::IsNullOrWhiteSpace($Binary)) {
    $Binary = Join-Path $PSScriptRoot '..\..\..\target\debug\m0-ipc-spike.exe'
}
$binaryPath = (Resolve-Path -LiteralPath $Binary).Path
$runtimeRoot = Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\p0-3\ipc-windows-' + [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffZ'))
[void][IO.Directory]::CreateDirectory($runtimeRoot)
$id = 'win-' + [DateTime]::UtcNow.ToString('HHmmssfff')
$pipe = 'cryptex-m0-p0-3-' + $id
$utf8 = New-Object Text.UTF8Encoding($false)
$processes = [Collections.Generic.List[Diagnostics.Process]]::new()
$checks = [Collections.Generic.List[object]]::new()

function Read-SharedText([string]$Path) {
    $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
    try {
        $reader = New-Object IO.StreamReader($stream)
        return $reader.ReadToEnd()
    } finally { $stream.Dispose() }
}

function Start-SpikeServer {
    $stdout = Join-Path $runtimeRoot ('server-' + $processes.Count + '.stdout')
    $stderr = Join-Path $runtimeRoot ('server-' + $processes.Count + '.stderr')
    $server = Start-Process -FilePath $binaryPath -ArgumentList @('serve', '--id', $id) -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    $processes.Add($server)
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    while ([DateTime]::UtcNow -lt $deadline) {
        if ($server.HasExited) { throw ('IPC server exited: ' + [IO.File]::ReadAllText($stderr)) }
        if ((Test-Path -LiteralPath $stdout) -and (Read-SharedText $stdout) -match 'ready id=') { return $server }
        Start-Sleep -Milliseconds 50
    }
    throw 'IPC server did not become ready.'
}

function Invoke-SpikeClient([string]$Role, [string]$Capability = 'status.read', [int]$Version = 1, [bool]$ExpectAccepted = $true) {
    $savedPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = @(& $binaryPath client --id $id --role $Role --capability $Capability --version $Version 2>$null)
        $exitCode = $LASTEXITCODE
    } finally { $ErrorActionPreference = $savedPreference }
    if ($output.Count -lt 1) { throw ('No response from ' + $Role + ' client.') }
    $response = $output[0] | ConvertFrom-Json
    if ([bool]$response.accepted -ne $ExpectAccepted) { throw ('Unexpected authorization result for ' + $Role + ': ' + ($response | ConvertTo-Json -Compress)) }
    if ($ExpectAccepted -and $exitCode -ne 0) { throw ('Authorized client exited ' + $exitCode) }
    if (-not $ExpectAccepted -and $exitCode -eq 0) { throw 'Unauthorized client unexpectedly exited successfully.' }
    $checks.Add([ordered]@{ role = $Role; capability = $Capability; version = $Version; accepted = [bool]$response.accepted; error = $response.error })
    return $response
}

function Invoke-RawPipeRequest([byte[]]$Payload, [string]$ExpectedError) {
    $client = [IO.Pipes.NamedPipeClientStream]::new('.', $pipe, [IO.Pipes.PipeDirection]::InOut)
    try {
        $client.Connect(5000)
        $client.Write($Payload, 0, $Payload.Length)
        $client.Flush()
        $responseBytes = New-Object 'System.Collections.Generic.List[byte]'
        while ($responseBytes.Count -le 4096) {
            $next = $client.ReadByte()
            if ($next -lt 0) { break }
            $responseBytes.Add([byte]$next)
            if ($next -eq 10) { break }
        }
        $response = [Text.Encoding]::UTF8.GetString($responseBytes.ToArray()) | ConvertFrom-Json
        if ($response.error -ne $ExpectedError) { throw ('Expected ' + $ExpectedError + ', got ' + $response.error) }
        $checks.Add([ordered]@{ role = 'raw'; capability = $null; version = $null; accepted = $false; error = $response.error })
    } finally { $client.Dispose() }
}

try {
    $server = Start-SpikeServer
    foreach ($role in @('desktop', 'cli', 'mcp')) { $null = Invoke-SpikeClient -Role $role }
    $null = Invoke-SpikeClient -Role 'cli' -Version 2 -ExpectAccepted $false
    $null = Invoke-SpikeClient -Role 'mcp' -Capability 'memory.write' -ExpectAccepted $false
    $null = Invoke-SpikeClient -Role 'unknown' -ExpectAccepted $false

    $malformed = [Text.Encoding]::UTF8.GetBytes("{`n")
    Invoke-RawPipeRequest -Payload $malformed -ExpectedError 'malformed_request'
    $oversized = New-Object byte[] 65537
    for ($index = 0; $index -lt $oversized.Length; $index++) { $oversized[$index] = 65 }
    Invoke-RawPipeRequest -Payload $oversized -ExpectedError 'frame_too_large'

    $server.Kill()
    if (-not $server.WaitForExit(5000)) { throw 'IPC server did not stop for the restart test.' }
    $retryOut = Join-Path $runtimeRoot 'restart-client.stdout'
    $retryErr = Join-Path $runtimeRoot 'restart-client.stderr'
    $retryClient = Start-Process -FilePath $binaryPath -ArgumentList @('client', '--id', $id, '--role', 'desktop') -WindowStyle Hidden -PassThru -RedirectStandardOutput $retryOut -RedirectStandardError $retryErr
    $processes.Add($retryClient)
    Start-Sleep -Milliseconds 500
    $restartedServer = Start-SpikeServer
    if (-not $retryClient.WaitForExit(12000)) { throw 'IPC client did not reconnect after the daemon restart.' }
    $retryClient.Refresh()
    $reconnectResponse = [IO.File]::ReadAllText($retryOut).Trim() | ConvertFrom-Json
    if (-not $retryClient.HasExited) { throw 'Reconnect client process remained active after writing its response.' }
    if (-not $reconnectResponse.accepted -or $reconnectResponse.daemon_pid -ne $restartedServer.Id) {
        throw 'Reconnect client did not receive a response from the restarted server process.'
    }
    $checks.Add([ordered]@{ role = 'desktop'; capability = 'status.read'; version = 1; accepted = $true; error = $null; reconnected_to_pid = $reconnectResponse.daemon_pid })

    $summary = [ordered]@{
        experiment = 'm0-p0-3-windows-local-ipc'
        runtime_root = $runtimeRoot
        endpoint = '\\.\pipe\' + $pipe
        windows_acl = 'EXPLICIT_CURRENT_USER_SID_CONFIGURED_OTHER_ACCOUNT_DENIAL_NOT_TESTED'
        server_pid_before_restart = $server.Id
        server_pid_after_restart = $restartedServer.Id
        checks = @($checks)
        result = 'PASS_WINDOWS_SPIKE_ONLY_MACOS_NOT_TESTED'
    }
    [IO.File]::WriteAllText((Join-Path $runtimeRoot 'summary.json'), ($summary | ConvertTo-Json -Depth 8), $utf8)
    $summary | ConvertTo-Json -Depth 8
} finally {
    foreach ($process in $processes) {
        if ($null -ne $process) {
            try {
                if (-not $process.HasExited) {
                    $process.Kill()
                    if (-not $process.WaitForExit(5000)) { Write-Warning ('Process did not stop during cleanup: ' + $process.Id) }
                }
            } catch { Write-Warning ('Could not stop test process ' + $process.Id + ': ' + $_.Exception.Message) }
            $process.Dispose()
        }
    }
}
