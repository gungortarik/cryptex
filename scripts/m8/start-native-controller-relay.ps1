param([Parameter(Mandatory=$true)][string]$SessionRoot,
    [Parameter(Mandatory=$true)][string]$RequestZip,
    [ValidateRange(120,1800)][int]$WaitSeconds=600,
    [switch]$ValidateOnly)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'peer-transfer-common.ps1')
$node=$null
$relay=Get-Content -LiteralPath (Join-Path $SessionRoot 'relay.json') -Raw | ConvertFrom-Json
$fixture=Get-Content -LiteralPath (Join-Path $SessionRoot 'sender.json') -Raw | ConvertFrom-Json
if($relay.schema -ne 1 -or $relay.session -cnotmatch '^[0-9a-f]{32}$' -or $relay.peer_id -cne $fixture.native_id){throw 'relay_binding_mismatch'}
if($relay.address -cnotmatch '^tcp://([0-9]+\.[0-9]+\.[0-9]+\.[0-9]+):([0-9]+)$'){throw 'private_relay_endpoint_required'}
$address=$Matches[1]; $port=[int]$Matches[2]
$ip=[Net.IPAddress]::Parse($address); $octets=$ip.GetAddressBytes()
if($ip.ToString() -cne $address -or $octets.Length -ne 4 -or $port -lt 1025 -or $port -gt 65535 -or
    -not ($octets[0] -eq 10 -or ($octets[0] -eq 192 -and $octets[1] -eq 168) -or
        ($octets[0] -eq 172 -and $octets[1] -ge 16 -and $octets[1] -le 31) -or $octets[0] -eq 127)){throw 'private_relay_endpoint_required'}
$result=[ordered]@{schema=1;session=$relay.session;scope='untrusted_host_relay_only';outcome='not_completed'}
try {
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    if((Get-Item -LiteralPath $RequestZip).Length -gt 32MB){throw 'request_size_limit'}
    $zip=[IO.Compression.ZipFile]::OpenRead($RequestZip)
    $stage=Join-Path $SessionRoot ('request-'+[guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $stage | Out-Null
    try {
        if($zip.Entries.Count -gt 40){throw 'request_entry_limit'}
        $entries=@{}; $total=0L
        foreach($entry in $zip.Entries) {
            # Windows PowerShell 5.1 Compress-Archive emits backslashes.
            # Normalize BEFORE closed-name validation, deduplication and writing.
            $name=$entry.FullName.Replace('\','/')
            if($name.EndsWith('/')) {
                if($name -cnotmatch '^payload/([0-9a-f]{72}/)?$'){throw 'unsafe_request_directory'}
                continue
            }
            if($entries.ContainsKey($name) -or
                ($name -cne 'request.json' -and $name -cnotmatch '^payload/[0-9a-f]{72}/[0-9]{20}\.seg$') -or
                (($entry.ExternalAttributes -shr 16) -band 0xF000) -eq 0xA000){throw 'unsafe_request_entry'}
            $total+=$entry.Length
            if($total -gt 32MB -or $entry.Length -gt 8MB){throw 'request_expansion_limit'}
            $entries[$name]=$entry
        }
        if(-not $entries.ContainsKey('request.json') -or $entries['request.json'].Length -gt 16KB -or $entries.Count -gt 17){throw 'request_manifest_limit'}
        foreach($name in $entries.Keys) {
            $dest=Join-Path $stage $name
            New-Item -ItemType Directory -Path ([IO.Path]::GetDirectoryName($dest)) -Force | Out-Null
            [IO.Compression.ZipFileExtensions]::ExtractToFile($entries[$name],$dest,$false)
        }
    } finally {$zip.Dispose()}
    $request=Get-Content -LiteralPath (Join-Path $stage 'request.json') -Raw | ConvertFrom-Json
    if($request.schema -ne 1 -or $request.session -cne $relay.session -or $request.relay_id -cne $relay.peer_id -or
        $request.folder_id -cne ('cx-native-'+$relay.session) -or $request.receiver_id -cnotmatch '^([A-Z2-7]{7}-){7}[A-Z2-7]{7}$' -or
        $request.receiver_id -ceq $relay.peer_id -or @($request.segments).Count -lt 1 -or @($request.segments).Count -gt 16){throw 'request_binding_mismatch'}
    $expectedBytes=0L; $seen=@{}
    foreach($segment in $request.segments) {
        $name='payload/'+$segment.relative_path
        if($segment.relative_path -cnotmatch '^[0-9a-f]{72}/[0-9]{20}\.seg$' -or $seen.ContainsKey($name) -or
            -not $entries.ContainsKey($name) -or $segment.sha256 -cnotmatch '^[0-9a-f]{64}$' -or
            $segment.bytes -le 0 -or $segment.bytes -gt 8MB){throw 'segment_manifest_invalid'}
        $seen[$name]=$true
        $path=Join-Path $stage $name
        if((Get-Item -LiteralPath $path).Length -ne $segment.bytes -or (Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant() -cne $segment.sha256){throw 'segment_digest_mismatch'}
        $expectedBytes+=$segment.bytes
    }
    if($entries.Count -ne $seen.Count+1){throw 'unlisted_payload'}
    if($ValidateOnly) {
        $result.outcome='request_validated_only'; $result.native_launched=$false
        return
    }
    Set-PeerListener $fixture $relay.address
    $node=Start-PeerNative $fixture
    Wait-PeerNative $node
    $payload=Add-PeerFolder $node $relay $request.receiver_id 'sendonly' 'dynamic'
    foreach($segment in $request.segments) {
        $dest=Join-Path $payload $segment.relative_path
        New-Item -ItemType Directory -Path ([IO.Path]::GetDirectoryName($dest)) -Force | Out-Null
        if(Test-Path -LiteralPath $dest){throw 'relay_payload_already_exists'}
        Copy-Item -LiteralPath (Join-Path $stage ('payload/'+$segment.relative_path)) -Destination $dest
    }
    Invoke-PeerApi $node ('/rest/db/scan?folder='+$request.folder_id) 'POST' | Out-Null
    Write-Output 'relay_ready_waiting_for_vm'
    $deadline=[DateTime]::UtcNow.AddSeconds($WaitSeconds); $completedAt=$null; $connected=$false
    while([DateTime]::UtcNow -lt $deadline) {
        if($node.process.HasExited){throw 'relay_exited'}
        $connections=Invoke-PeerApi $node '/rest/system/connections'
        $peer=$connections.connections.PSObject.Properties[$request.receiver_id].Value
        if($peer -and $peer.connected){$connected=$true}
        $completion=Invoke-PeerApi $node ('/rest/db/completion?folder='+$request.folder_id+'&device='+$request.receiver_id)
        if($connected -and $completion.globalBytes -eq $expectedBytes -and $completion.needBytes -eq 0 -and $completion.completion -eq 100 -and $null -eq $completedAt){$completedAt=[DateTime]::UtcNow}
        # Keep the source alive across both the forced native restart and reopen.
        if($null -ne $completedAt -and ([DateTime]::UtcNow-$completedAt).TotalSeconds -ge 120){break}
        Start-Sleep -Milliseconds 250
    }
    Stop-PeerNative $node; $node=$null
    if($null -eq $completedAt){throw 'relay_completion_timeout'}
    $result.outcome='peer_completion_observed'; $result.clean_shutdown=$true
    $result.receiver_id=$request.receiver_id; $result.bytes=$expectedBytes
} catch {
    $code=$_.Exception.Message
    try {Stop-PeerNative $node} catch {$code='relay_shutdown_failed'}
    if($code -cnotmatch '^[a-z_]{1,64}$'){$code='relay_operation_failed'}
    $result.outcome='failed'; $result.error_code=$code
} finally {
    Write-PeerJson (Join-Path $SessionRoot 'relay-result.json') $result
}
if($result.outcome -eq 'failed'){throw $result.error_code}
Write-Output 'relay_completion_observed_receiver_receipt_still_required'
