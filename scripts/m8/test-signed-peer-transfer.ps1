param([Parameter(Mandatory=$true)][string]$NativeBinary,[Parameter(Mandatory=$true)][string]$Daemon)
$ErrorActionPreference='Stop'
$base=@(& powershell.exe -NoProfile -File (Join-Path $PSScriptRoot 'test-peer-transfer.ps1') -NativeBinary $NativeBinary)
if ($LASTEXITCODE -ne 0) { throw 'Base transport regression failed' }
$sessionRoot=($base | Where-Object { $_ -like 'Evidence root: *' }).Substring(15)
$built=@(& powershell.exe -NoProfile -File (Join-Path $PSScriptRoot 'package-signed-peer-transfer.ps1') -SessionRoot $sessionRoot -Daemon $Daemon -OutputDirectory (Join-Path $env:LOCALAPPDATA 'Cryptex-Dev\peer-transfer\test-packages'))
if ($LASTEXITCODE -ne 0) { throw 'Signed package failed' }
$stage=($built | Where-Object { $_ -like 'signed_package_directory=*' }).Substring(25)
$start=New-Object Diagnostics.ProcessStartInfo
$start.FileName="$env:SYSTEMROOT\System32\WindowsPowerShell\v1.0\powershell.exe"
$start.Arguments='-NoProfile -File "' + (Join-Path $PSScriptRoot 'start-peer-sender.ps1') + '" -SessionRoot "' + $sessionRoot + '" -RequestPath "' + (Join-Path $sessionRoot 'peer-request.json') + '" -PackageDirectory "' + $stage + '" -WaitSeconds 30'
$start.UseShellExecute=$false
$start.CreateNoWindow=$true
$start.RedirectStandardOutput=$true
$start.RedirectStandardError=$true
$sender=[Diagnostics.Process]::Start($start)
$stdout=$sender.StandardOutput.ReadToEndAsync()
$stderr=$sender.StandardError.ReadToEndAsync()
& powershell.exe -NoProfile -File (Join-Path $stage 'vm-peer-transfer.ps1') -Mode Receive -NoOpen
$receiverExit=$LASTEXITCODE
if (-not $sender.WaitForExit(45000)) { throw 'Sender did not stop' }
if ($receiverExit -ne 0 -or $sender.ExitCode -ne 0) { Write-Output $stdout.Result; Write-Output $stderr.Result; throw 'Signed native transfer failed' }
$result=Get-Content -LiteralPath (Join-Path $sessionRoot 'signed-transfer-result.json') -Raw | ConvertFrom-Json
if (-not $result.signature_check.signature_verified -or -not $result.signature_check.tampered_copy_rejected -or
    $result.signature_check.trust_activated -ne $false -or $result.signature_check.published -ne $false -or $result.canonical_import -ne 'not_run') {
    throw 'Signed result scope mismatch'
}
if (Get-ChildItem -LiteralPath $stage -Filter '*.seg') { throw 'Source segment leaked into VM package' }
Write-Output 'PASS: base transport regression; retained native profiles; daemon sealed signed event transfer; positive signature; tampered copy rejection; no trust/publication'
Write-Output ('Evidence root: ' + $sessionRoot)
