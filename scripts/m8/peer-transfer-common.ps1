$ErrorActionPreference = 'Stop'
$script:NativeHash = '36A0F7BC372F64FA7CC4F5654FA324C0DD9F7FEF2E07565E00C6E1CF73F50344'
function Write-PeerJson($Path, $Value) {
    [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
}
function Assert-PeerBinary($Path) {
    if ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash -ne $script:NativeHash) { throw 'native_hash_mismatch' }
}
function Read-PeerSettings($Path) {
    if ((Get-Item -LiteralPath $Path).Length -gt 16KB) { throw 'oversized_settings' }
    $settings = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    if ($settings.schema -ne 1 -or $settings.session -notmatch '^[a-f0-9]{32}$' -or
        $settings.host_id -notmatch '^([A-Z2-7]{7}-){7}[A-Z2-7]{7}$' -or
        $settings.sha256 -notmatch '^[A-F0-9]{64}$' -or $settings.bytes -ne 32768 -or
        $settings.host_port -lt 1025 -or $settings.host_port -gt 65535) { throw 'invalid_settings' }
    $ip = [Net.IPAddress]::Parse($settings.host_address)
    $b = $ip.GetAddressBytes()
    if ($b.Length -ne 4 -or -not ($b[0] -eq 10 -or ($b[0] -eq 192 -and $b[1] -eq 168) -or
        ($b[0] -eq 172 -and $b[1] -ge 16 -and $b[1] -le 31) -or $b[0] -eq 127)) { throw 'private_ipv4_required' }
    return $settings
}
function New-PeerFixture($Binary) {
    Assert-PeerBinary $Binary
    $root = Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\m8-verification\m8-owned-native-' + [guid]::NewGuid().ToString('N'))
    & (Join-Path $PSScriptRoot 'prepare-owned-transport-fixture.ps1') -Binary $Binary -FixtureRoot $root | Out-Null
    $fixture = Get-Content -LiteralPath (Join-Path $root 'fixture.json') -Raw | ConvertFrom-Json
    $retainedBinary = Join-Path $root 'syncthing.exe'
    Copy-Item -LiteralPath $Binary -Destination $retainedBinary
    $fixture.binary = $retainedBinary
    $fixture | Add-Member NoteProperty root $root
    return $fixture
}
function Set-PeerListener($Fixture, $Address) {
    $path = Join-Path $Fixture.root 'profile\config.xml'
    $doc = New-Object Xml.XmlDocument
    $doc.XmlResolver = $null
    $doc.Load($path)
    $doc.configuration.options.listenAddress = $Address
    $doc.Save($path)
}
function Start-PeerNative($Fixture) {
    Assert-PeerBinary $Fixture.binary
    $config = New-Object Xml.XmlDocument
    $config.XmlResolver = $null
    $config.Load((Join-Path $Fixture.root 'profile\config.xml'))
    $start = New-Object Diagnostics.ProcessStartInfo
    $start.FileName = $Fixture.binary
    $start.Arguments = 'serve --home "' + (Join-Path $Fixture.root 'profile') + '" --no-browser --no-restart --no-upgrade --log-file "' + (Join-Path $Fixture.root 'native.log') + '"'
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.EnvironmentVariables.Clear()
    foreach ($name in @('SYSTEMROOT','WINDIR','TEMP','TMP','LOCALAPPDATA','APPDATA','USERPROFILE')) {
        $value = [Environment]::GetEnvironmentVariable($name)
        if ($null -ne $value) { $start.EnvironmentVariables[$name] = $value }
    }
    $start.EnvironmentVariables['STNODEFAULTFOLDER'] = '1'
    $process = [Diagnostics.Process]::Start($start)
    return [pscustomobject]@{ fixture=$Fixture; process=$process; api_key=[string]$config.configuration.gui.apikey;
        stdout_task=$process.StandardOutput.ReadToEndAsync(); stderr_task=$process.StandardError.ReadToEndAsync() }
}
function Invoke-PeerApi($Node, $Route, $Method = 'GET', $Body = $null) {
    $parameters = @{ Uri=('http://127.0.0.1:' + $Node.fixture.api_port + $Route); Method=$Method;
        Headers=@{'X-API-Key'=$Node.api_key}; TimeoutSec=3; ErrorAction='Stop' }
    if ($null -ne $Body) { $parameters.Body = $Body | ConvertTo-Json -Depth 16; $parameters.ContentType='application/json' }
    Invoke-RestMethod @parameters
}
function Wait-PeerNative($Node) {
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    while ([DateTime]::UtcNow -lt $deadline) {
        if ($Node.process.HasExited) { throw 'native_start_failed' }
        try {
            $status = Invoke-PeerApi $Node '/rest/system/status'
            if ($status.myID -ne $Node.fixture.native_id) { throw 'identity_mismatch' }
            return
        } catch { Start-Sleep -Milliseconds 250 }
    }
    throw 'native_ready_timeout'
}
function Add-PeerFolder($Node, $Settings, $PeerId, $Role, $PeerAddress) {
    $config = Invoke-PeerApi $Node '/rest/config'
    if ($config.options.globalAnnounceEnabled -or $config.options.localAnnounceEnabled -or
        $config.options.relaysEnabled -or $config.options.natEnabled -or
        $config.gui.address -ne ('127.0.0.1:' + $Node.fixture.api_port)) { throw 'isolation_failed' }
    $payload = Join-Path $Node.fixture.root 'payload'
    if (@($config.folders).Count -ne 0) {
        $folder = @($config.folders)[0]
        $ids = @($folder.devices | ForEach-Object deviceID | Sort-Object)
        $expectedIds = @($Node.fixture.native_id,$PeerId | Sort-Object)
        $remote = @($config.devices | Where-Object deviceID -eq $PeerId)
        if (@($config.folders).Count -ne 1 -or @($config.devices).Count -ne 2 -or
            $folder.id -ne ('cx-native-' + $Settings.session) -or $folder.path -ne $payload -or
            $folder.type -ne $Role -or ($ids -join ',') -ne ($expectedIds -join ',') -or
            $remote.Count -ne 1 -or @($remote[0].addresses).Count -ne 1 -or $remote[0].addresses[0] -ne $PeerAddress) {
            throw 'retained_folder_mismatch'
        }
        return $payload
    }
    if (@($config.devices).Count -ne 1) { throw 'isolation_failed' }
    $device = Invoke-PeerApi $Node '/rest/config/defaults/device'
    $device.deviceID = $PeerId
    $device.name = 'Isolated transport test peer'
    $device.addresses = @($PeerAddress)
    $device.autoAcceptFolders = $false
    Invoke-PeerApi $Node '/rest/config/devices' 'POST' $device | Out-Null
    New-Item -ItemType Directory -Path $payload | Out-Null
    $folder = Invoke-PeerApi $Node '/rest/config/defaults/folder'
    $folder.id = 'cx-native-' + $Settings.session
    $folder.label = 'Isolated synthetic transport test'
    $folder.path = $payload
    $folder.type = $Role
    $folder.rescanIntervalS = 5
    $folder.fsWatcherEnabled = $false
    $folder.devices = @(@{deviceID=$Node.fixture.native_id; encryptionPassword=''},@{deviceID=$PeerId; encryptionPassword=''})
    Invoke-PeerApi $Node '/rest/config/folders' 'POST' $folder | Out-Null
    return $payload
}
function Stop-PeerNative($Node) {
    if ($null -eq $Node) { return }
    if (-not $Node.process.HasExited) {
        try { Invoke-PeerApi $Node '/rest/system/shutdown' 'POST' | Out-Null } catch { }
        if (-not $Node.process.WaitForExit(25000)) {
            $Node.process.Kill()
            $Node.process.WaitForExit(5000) | Out-Null
            throw 'native_forced_stop'
        }
    }
    if ($Node.process.ExitCode -ne 0) { throw 'native_unclean_exit' }
    $log = Join-Path $Node.fixture.root 'native.log'
    if ((Test-Path -LiteralPath $log) -and (Select-String -LiteralPath $log -SimpleMatch 'Database failed to stop within 10s' -Quiet)) {
        throw 'native_database_stop_failed'
    }
}
