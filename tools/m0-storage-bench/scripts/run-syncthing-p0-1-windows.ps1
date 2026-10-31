Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
    throw 'Windows is required'
}

$STHOME = Join-Path $env:LOCALAPPDATA 'Cryptex-Dev\syncthing-p0-1'
$ConfigPath = Join-Path $STHOME 'config.xml'

if (-not (Get-Command syncthing -ErrorAction SilentlyContinue)) {
    throw 'syncthing is not available on PATH'
}
if (-not (Test-Path -LiteralPath $ConfigPath -PathType Leaf)) {
    throw "config.xml not found: $ConfigPath"
}

[xml]$Config = Get-Content -LiteralPath $ConfigPath -Raw
$GuiAddress = [string]$Config.configuration.gui.address
if ($GuiAddress -ne '127.0.0.1:18384') {
    throw "unexpected GUI address: $GuiAddress"
}

$Listeners = @($Config.configuration.options.listenAddress | ForEach-Object { [string]$_ })
if ($Listeners.Count -ne 1 -or $Listeners[0] -ne 'tcp://0.0.0.0:22001') {
    throw "unexpected sync listeners: $($Listeners -join ',')"
}

$RequiredOptions = @{
    globalAnnounceEnabled = 'false'
    localAnnounceEnabled  = 'false'
    relaysEnabled         = 'false'
    natEnabled            = 'false'
    autoUpgradeIntervalH  = '0'
    releasesURL           = ''
}
foreach ($Name in $RequiredOptions.Keys) {
    $Node = $Config.configuration.options.SelectSingleNode($Name)
    if ($null -eq $Node) {
        throw "required isolated option missing: $Name"
    }
    $Value = [string]$Node.InnerText
    if ($Value -ne $RequiredOptions[$Name]) {
        throw "unexpected $Name value: $Value"
    }
}

$FolderNodes = @($Config.SelectNodes('/configuration/folder'))
$ActiveFolders = @($FolderNodes | Where-Object { $_.id -and $_.path })
if ($ActiveFolders.Count -ne 0) {
    throw 'active folder entries already exist; refusing pre-pairing launch'
}

$BusyPorts = @(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue |
    Where-Object { $_.LocalPort -in @(18384, 22001) })
if ($BusyPorts.Count -gt 0) {
    $Details = ($BusyPorts | ForEach-Object { "$($_.LocalAddress):$($_.LocalPort)" }) -join ', '
    throw "benchmark listener port already in use: $Details"
}

Write-Output 'scope=isolated_p0_1_syncthing_only'
Write-Output "home=$STHOME"
Write-Output 'gui=127.0.0.1:18384'
Write-Output 'listen_address=tcp://0.0.0.0:22001'
Write-Output 'global_announce=false'
Write-Output 'local_announce=false'
Write-Output 'relays=false'
Write-Output 'nat=false'
Write-Output 'auto_upgrade_interval_h=0'
Write-Output 'releases_url=blank'
Write-Output 'launch_no_upgrade=true'
Write-Output 'transport_scope=direct_tcp_only'
Write-Output 'active_folder_count=0'
Write-Output 'starting_foreground=true'

& syncthing serve "--home=$STHOME" --no-browser --no-restart --no-upgrade
if ($LASTEXITCODE -ne 0) {
    throw "syncthing exited with code $LASTEXITCODE"
}
