param([Parameter(Mandatory=$true)][string]$NativeBinary)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'peer-transfer-common.ps1')
$output = @(& (Join-Path $PSScriptRoot 'package-peer-transfer.ps1') -NativeBinary $NativeBinary -HostAddress 127.0.0.1 -OutputDirectory (Join-Path $env:LOCALAPPDATA 'Cryptex-Dev\peer-transfer\test-packages'))
$sessionRoot = ($output | Where-Object { $_ -like 'session_root=*' }).Substring(13)
$stage = Join-Path $sessionRoot 'package'
$settings = Read-PeerSettings (Join-Path $stage 'peer-settings.json')
& powershell.exe -NoProfile -File (Join-Path $stage 'vm-peer-transfer.ps1') -Mode Prepare -NoOpen
if ($LASTEXITCODE -ne 0) { throw 'Receiver prepare failed' }
$receiverRoot = Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\peer-transfer\' + $settings.session)
$requestPath = Join-Path $receiverRoot 'peer-request.json'
$requestBytes = Get-Content -LiteralPath $requestPath -Raw
if ($requestBytes -match 'apikey|BEGIN|private|root|binary"') { throw 'Private request content' }
$request = $requestBytes | ConvertFrom-Json
if ($request.native_id -eq $settings.host_id) { throw 'Identities must differ' }
Write-Output 'PASS: native preparation; independent identity; public request only'
$senderFile = Join-Path $PSScriptRoot 'start-peer-sender.ps1'
$start = New-Object Diagnostics.ProcessStartInfo
$start.FileName = "$env:SYSTEMROOT\System32\WindowsPowerShell\v1.0\powershell.exe"
$start.Arguments = '-NoProfile -File "' + $senderFile + '" -SessionRoot "' + $sessionRoot + '" -RequestPath "' + $requestPath + '" -WaitSeconds 30'
$start.UseShellExecute=$false
$start.CreateNoWindow=$true
$start.RedirectStandardOutput=$true
$start.RedirectStandardError=$true
$sender = [Diagnostics.Process]::Start($start)
$stdout=$sender.StandardOutput.ReadToEndAsync()
$stderr=$sender.StandardError.ReadToEndAsync()
& powershell.exe -NoProfile -File (Join-Path $stage 'vm-peer-transfer.ps1') -Mode Receive -NoOpen
$receiverExit=$LASTEXITCODE
if (-not $sender.WaitForExit(45000)) { throw 'Sender test did not stop in time' }
if ($sender.ExitCode -ne 0 -or $receiverExit -ne 0) {
    Write-Output $stdout.Result
    Write-Output $stderr.Result
    throw 'Native transfer test failed'
}
$result=Get-Content -LiteralPath (Join-Path $receiverRoot 'transfer-result.json') -Raw | ConvertFrom-Json
if ($result.outcome -ne 'received_hash_match' -or $result.bytes -ne 32768 -or -not $result.clean_shutdown -or $result.canonical_import -ne 'not_run') {
    throw 'False transfer success'
}
Write-Output 'PASS: two native processes; 32768-byte hash match; clean shutdown; no canonical import claim'
$bad=Join-Path $sessionRoot 'wrong-request.json'
$request.session='0'*32
Write-PeerJson $bad $request
$savedPreference = $ErrorActionPreference
try {
    $ErrorActionPreference = 'Continue'
    & powershell.exe -NoProfile -File $senderFile -SessionRoot $sessionRoot -RequestPath $bad -WaitSeconds 15 *> (Join-Path $sessionRoot 'expected-rejection.txt')
} finally { $ErrorActionPreference = $savedPreference }
if ($LASTEXITCODE -eq 0) { throw 'Wrong session accepted' }
$errorResult=Get-Content -LiteralPath (Join-Path $sessionRoot 'sender-error.json') -Raw | ConvertFrom-Json
if ($errorResult.error_code -ne 'request_binding_mismatch') { throw 'Unexpected rejection' }
Write-Output 'PASS: wrong-session request rejected before network startup'
$settings.host_address = '8.8.8.8'
$invalidSettings = Join-Path $sessionRoot 'invalid-settings.json'
Write-PeerJson $invalidSettings $settings
$rejected=$false
try { Read-PeerSettings $invalidSettings | Out-Null } catch { $rejected=($_.Exception.Message -eq 'private_ipv4_required') }
if (-not $rejected) { throw 'Public Internet endpoint accepted' }
Write-Output 'PASS: public endpoint refused'
Write-Output ('Evidence root: ' + $sessionRoot)
