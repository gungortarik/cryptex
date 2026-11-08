param([ValidateSet('Prepare','Receive')][string]$Mode='Prepare', [switch]$NoOpen)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'peer-transfer-common.ps1')
$node = $null
try {
    $settings = Read-PeerSettings (Join-Path $PSScriptRoot 'peer-settings.json')
    $stateRoot = Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\peer-transfer\' + $settings.session)
    $statePath = Join-Path $stateRoot 'receiver.json'
    if ($Mode -eq 'Prepare') {
        if (Test-Path -LiteralPath $statePath) {
            $fixture = Get-Content -LiteralPath $statePath -Raw | ConvertFrom-Json
        } else {
            $fixture = New-PeerFixture (Join-Path $PSScriptRoot 'syncthing.exe')
            New-Item -ItemType Directory -Path $stateRoot -Force | Out-Null
            Write-PeerJson $statePath $fixture
        }
        Write-PeerJson (Join-Path $stateRoot 'peer-request.json') @{
            schema=1; scope='synthetic_native_transport_only'; session=$settings.session;
            native_id=$fixture.native_id; binary_sha256=$script:NativeHash
        }
        $output = Join-Path $stateRoot 'ISTEK.zip'
        Compress-Archive -LiteralPath (Join-Path $stateRoot 'peer-request.json') -DestinationPath $output -Force
        Write-Host 'Hazir. ISTEK.zip dosyasini sohbete ekleyin. Sonra size 2-AKTAR dosyasini ne zaman acacaginizi soyleyecegim.'
    } else {
        $fixture = Get-Content -LiteralPath $statePath -Raw | ConvertFrom-Json
        $node = Start-PeerNative $fixture
        Wait-PeerNative $node
        $payload = Add-PeerFolder $node $settings $settings.host_id 'receiveonly' ('tcp://' + $settings.host_address + ':' + $settings.host_port)
        Write-Host 'Test dosyasi bekleniyor (en fazla 3 dakika)...'
        $target = Join-Path $payload 'synthetic.bin'
        $deadline = [DateTime]::UtcNow.AddSeconds(180)
        $matched = $false
        while ([DateTime]::UtcNow -lt $deadline) {
            if ($node.process.HasExited) { throw 'native_exited_during_transfer' }
            $connections = Invoke-PeerApi $node '/rest/system/connections'
            $peer = $connections.connections.PSObject.Properties[$settings.host_id].Value
            if (Test-Path -LiteralPath $target) {
                if ((Get-Item -LiteralPath $target).Length -eq $settings.bytes -and
                    (Get-FileHash -LiteralPath $target).Hash -eq $settings.sha256 -and $peer.connected) { $matched=$true; break }
            }
            Start-Sleep -Milliseconds 500
        }
        if (-not $matched) { throw 'transfer_timeout' }
        Stop-PeerNative $node
        $node = $null
        Write-PeerJson (Join-Path $stateRoot 'transfer-result.json') @{
            schema=1; scope='synthetic_native_transport_only'; session=$settings.session; outcome='received_hash_match';
            receiver_id=$fixture.native_id; sender_id=$settings.host_id; bytes=$settings.bytes; sha256=$settings.sha256;
            canonical_import='not_run'; clean_shutdown=$true; collected_utc=[DateTime]::UtcNow.ToString('o')
        }
        $output = Join-Path $stateRoot 'AKTARIM-SONUC.zip'
        Compress-Archive -LiteralPath (Join-Path $stateRoot 'transfer-result.json') -DestinationPath $output -Force
        Write-Host 'Aktarim tamamlandi. AKTARIM-SONUC.zip dosyasini sohbete ekleyin.'
    }
    if (-not $NoOpen) { Start-Process explorer.exe -ArgumentList ('/select,"{0}"' -f $output) }
    Write-Host $output
} catch {
    $failure = $_.Exception.Message
    try { Stop-PeerNative $node } catch { $failure = 'shutdown_failed_after_error' }
    # Only export a closed diagnostic code, never generated config/key/log text.
    if ($failure -notmatch '^[a-z_]{1,64}$') { $failure='operation_failed' }
    Write-Host ('Kontrol durdu: ' + $failure)
    if ($stateRoot -and (Test-Path -LiteralPath $stateRoot)) {
        Write-PeerJson (Join-Path $stateRoot 'transfer-error.json') @{schema=1; scope='synthetic_native_transport_only'; error_code=$failure}
        $output = Join-Path $stateRoot 'AKTARIM-HATA.zip'
        Compress-Archive -LiteralPath (Join-Path $stateRoot 'transfer-error.json') -DestinationPath $output -Force
        if (-not $NoOpen) { Start-Process explorer.exe -ArgumentList ('/select,"{0}"' -f $output) }
    }
    exit 1
}
