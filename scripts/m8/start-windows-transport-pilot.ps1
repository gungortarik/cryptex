param(
    [Parameter(Mandatory = $true)]
    [string] $SyncthingExe,
    [switch] $JoiningDevice
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..')).Path
$desktop = (Resolve-Path -LiteralPath (Join-Path $repo 'target\release\cryptex-desktop.exe')).Path
$daemon = (Resolve-Path -LiteralPath (Join-Path $repo 'target\release\cryptexd.exe')).Path
$native = (Resolve-Path -LiteralPath $SyncthingExe).Path
if (Get-Process -Name cryptex-desktop,cryptexd -ErrorAction SilentlyContinue) {
    throw 'Close the running Cryptex test instance before starting another pilot.'
}

$trial = Join-Path ([IO.Path]::GetTempPath()) ('cx-m8-' + [guid]::NewGuid().ToString('N'))
$runtime = Join-Path $trial 'desktop'
$prep = Join-Path $trial 'transport'
$package = Join-Path $trial 'package'
New-Item -ItemType Directory -Path $runtime,$prep,$package | Out-Null
foreach ($root in @($runtime,$prep)) {
    [IO.File]::WriteAllText((Join-Path $root 'pilot-runtime.marker'), "m0-native-desktop-runtime-v1`n")
}
Copy-Item -LiteralPath $native -Destination (Join-Path $package 'syncthing.exe')
foreach ($notice in @('LICENSE.txt','AUTHORS.txt')) {
    Copy-Item -LiteralPath (Join-Path $repo "third_party\syncthing\2.1.5\$notice") -Destination $package
}

$apiListener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
$syncListener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
try {
    $apiListener.Start()
    $syncListener.Start()
    $api = $apiListener.LocalEndpoint.Port
    $sync = $syncListener.LocalEndpoint.Port
} finally {
    $apiListener.Stop()
    $syncListener.Stop()
}
Write-Output "pilot_root=$trial"
& $daemon pilot-transport --confirm-isolated-pilot $prep $package $api $sync
if ($LASTEXITCODE -ne 0) { throw "Pilot transport provisioning failed ($LASTEXITCODE)." }
if (-not $JoiningDevice) {
    & $daemon pilot-fixture timeline-v1 --runtime-root $runtime --confirm-isolated-pilot
    if ($LASTEXITCODE -ne 0) { throw "Timeline fixture failed ($LASTEXITCODE)." }
}

$profile = Join-Path $prep 'managed-profile'
$desktopArguments = @('--cryptex-pilot-runtime', ('"{0}"' -f $runtime),
    '--cryptex-pilot-transport', ('"{0}"' -f $profile), ('"{0}"' -f $package), $api, $sync)
$process = Start-Process -FilePath $desktop -ArgumentList $desktopArguments -PassThru
$ready = Join-Path $runtime 'ready'
$deadline = [DateTime]::UtcNow.AddSeconds(30)
while (-not (Test-Path -LiteralPath $ready)) {
    if ($process.HasExited -or [DateTime]::UtcNow -ge $deadline) {
        throw "Desktop pilot did not become ready. Inspect $runtime for startup diagnostics."
    }
    Start-Sleep -Milliseconds 250
}
[IO.File]::WriteAllText((Join-Path $runtime 'show.request'), "show`n")
Write-Output 'desktop_pilot=ready'
Write-Output 'In Desktop, select Read verified adapter identity.'
