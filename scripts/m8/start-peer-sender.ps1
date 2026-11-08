param([Parameter(Mandatory=$true)][string]$SessionRoot,
    [Parameter(Mandatory=$true)][string]$RequestPath,
    [ValidateRange(15,1800)][int]$WaitSeconds=600)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'peer-transfer-common.ps1')
$node = $null
try {
    $settings = Read-PeerSettings (Join-Path $SessionRoot 'package\peer-settings.json')
    if ((Get-Item -LiteralPath $RequestPath).Length -gt 4096) { throw 'oversized_request' }
    $request = Get-Content -LiteralPath $RequestPath -Raw | ConvertFrom-Json
    if ($request.schema -ne 1 -or $request.scope -ne 'synthetic_native_transport_only' -or
        $request.session -ne $settings.session -or $request.binary_sha256 -ne $script:NativeHash -or
        $request.native_id -notmatch '^([A-Z2-7]{7}-){7}[A-Z2-7]{7}$' -or $request.native_id -eq $settings.host_id) {
        throw 'request_binding_mismatch'
    }
    $fixture = Get-Content -LiteralPath (Join-Path $SessionRoot 'sender.json') -Raw | ConvertFrom-Json
    Set-PeerListener $fixture ('tcp://' + $settings.host_address + ':' + $settings.host_port)
    $node = Start-PeerNative $fixture
    Wait-PeerNative $node
    $payload = Add-PeerFolder $node $settings $request.native_id 'sendonly' 'dynamic'
    Copy-Item -LiteralPath (Join-Path $fixture.root 'synthetic.bin') -Destination (Join-Path $payload 'synthetic.bin')
    Invoke-PeerApi $node ('/rest/db/scan?folder=cx-native-' + $settings.session) 'POST' | Out-Null
    Write-Output 'sender_ready_waiting_for_vm'
    $deadline = [DateTime]::UtcNow.AddSeconds($WaitSeconds)
    $complete = $false
    while ([DateTime]::UtcNow -lt $deadline) {
        if ($node.process.HasExited) { throw 'native_exited_during_transfer' }
        $completion = Invoke-PeerApi $node ('/rest/db/completion?folder=cx-native-' + $settings.session + '&device=' + $request.native_id)
        if ($completion.globalBytes -eq 32768 -and $completion.needBytes -eq 0 -and $completion.completion -eq 100) {
            $complete=$true; break
        }
        Start-Sleep -Seconds 1
    }
    # Keep the source available while the receiver verifies bytes and shuts down.
    # Completion may reach the sender before the receiver's next polling tick.
    if ($complete) {
        $closeDeadline = [DateTime]::UtcNow.AddSeconds(15)
        while ([DateTime]::UtcNow -lt $closeDeadline) {
            $connections = Invoke-PeerApi $node '/rest/system/connections'
            $peer = $connections.connections.PSObject.Properties[$request.native_id].Value
            if (-not $peer.connected) { break }
            Start-Sleep -Milliseconds 250
        }
    }
    Stop-PeerNative $node
    $node=$null
    if (-not $complete) { throw 'sender_wait_timeout' }
    Write-PeerJson (Join-Path $SessionRoot 'sender-result.json') @{schema=1; session=$settings.session;
        scope='synthetic_native_transport_only'; outcome='peer_completion_observed'; clean_shutdown=$true}
    Write-Output 'sender_peer_completion_observed'
} catch {
    $failure=$_.Exception.Message
    try { Stop-PeerNative $node } catch { $failure='shutdown_failed_after_error' }
    if ($failure -notmatch '^[a-z_]{1,64}$') { $failure='operation_failed' }
    Write-PeerJson (Join-Path $SessionRoot 'sender-error.json') @{schema=1; error_code=$failure}
    throw $failure
}
