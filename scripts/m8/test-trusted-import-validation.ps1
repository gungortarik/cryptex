param([Parameter(Mandatory=$true)][string]$SourceArchive,[Parameter(Mandatory=$true)][string]$Daemon)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'peer-transfer-common.ps1')
. (Join-Path $PSScriptRoot 'trusted-import-bundle.ps1')
. (Join-Path $PSScriptRoot 'verify-signed-payload.ps1')
$root=Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\trusted-validation-tests\'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$probe='c223e671-3c5e-413d-a82c-419ed88a508f'
$bundle=Read-TrustedImportBundle $SourceArchive $probe (Join-Path $root 'bundle')
$manifest=$bundle.manifest
$result=Invoke-ProbeVerifier $Daemon (Join-Path $bundle.root ('unapproved\'+$manifest.unapproved_segment.relative_path.Replace('/','\'))) (Join-Path $bundle.root 'unapproved-public.json')
if ($result.exit -ne 0 -or ($result.stdout | ConvertFrom-Json).outcome -ne 'signature_verified') { throw 'Actual unapproved signature invalid' }
Write-Output 'PASS: real returned bundle hashes/identity bindings and genuine unapproved signature verified by daemon'
# Exercise orchestration with controlled CLI responses, not fake native evidence.
$stage=Join-Path $root 'mock-package'
New-Item -ItemType Directory -Path $stage | Out-Null
foreach ($name in @('trusted-import-bundle.ps1','verify-signed-payload.ps1','validate-trusted-import-payload.ps1')) {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination $stage
}
[IO.File]::WriteAllText((Join-Path $stage 'cryptexd.exe'),'NOT AN EXECUTABLE - unit mock only')
$profile='cx-m8-'+[guid]::NewGuid().ToString('N')
$receiver=Join-Path $env:TEMP ($profile+'\desktop')
New-Item -ItemType Directory -Path "$receiver\identity","$receiver\bootstrap","$receiver\canonical\events\fixture" -Force | Out-Null
[IO.File]::WriteAllText("$receiver\pilot-runtime.marker","m0-native-desktop-runtime-v1`n")
Write-PeerJson "$receiver\identity\local_device.json" @{device_id=$manifest.receiver_device;brain_id=$manifest.brain_id}
[IO.File]::WriteAllText("$receiver\bootstrap\pending.json",'UNIT MOCK JOURNAL')
[IO.File]::WriteAllText("$receiver\canonical\events\fixture\00000000000000000000.open",'UNIT MOCK SENTINEL - NOT A CANONICAL EVENT')
$settings=[pscustomobject]@{trusted_probe_id=$probe;manifest_sha256=$bundle.manifest_sha256;brain_id=$manifest.brain_id;source_device=$manifest.source_device;
    receiver_device=$manifest.receiver_device;receiver_profile=$profile;daemon_sha256=(Get-FileHash -LiteralPath (Join-Path $stage 'cryptexd.exe')).Hash;sha256=(Get-FileHash -LiteralPath $SourceArchive).Hash}
. (Join-Path $stage 'validate-trusted-import-payload.ps1')
$script:incomingCalls=0
$script:rejectReason='retrieval safety gate is held: Some(CanonicalVerificationUnavailable)'
function Invoke-IncomingProbe($Daemon,$Root,[string[]]$Paths) {
    $script:incomingCalls++
    if ($script:incomingCalls -eq 2) { return [pscustomobject]@{exit=2;stdout='';stderr=$script:rejectReason} }
    if ($script:incomingCalls -eq 3) { return [pscustomobject]@{exit=2;stdout='';stderr='integrity_failure'} }
    return [pscustomobject]@{exit=0;stderr='';stdout='{"outcome":"validated_only","segment_count":1,"event_count":3,"published":false}'}
}
function Invoke-ProbeVerifier($Daemon,$Segment,$Descriptor) {
    return [pscustomobject]@{exit=0;stderr='';stdout='{"outcome":"signature_verified","trust_activated":false,"published":false}'}
}
$report=Test-TrustedImportPayload $settings $SourceArchive $root
if (-not $report.trusted_accepted -or -not $report.tampered_rejected -or -not $report.canonical_event_hashes_unchanged -or $script:incomingCalls -ne 4 -or $report.published) { throw 'Wrapper orchestration failed' }
foreach ($segment in $manifest.segments) {
    $input=Join-Path $receiver ('transport\incoming\'+$segment.relative_path.Replace('/','\'))
    if ((Get-FileHash -LiteralPath $input).Hash -cne $segment.sha256) { throw 'Incoming original not restored' }
}
Write-Output 'PASS: mocked CLI workflow; real file hash checks; tampered inode removed and original restored'
$script:incomingCalls=0
$script:rejectReason='instance already running'
$rejected=$false
try { Test-TrustedImportPayload $settings $SourceArchive $root | Out-Null } catch { $rejected=($_.Exception.Message -eq 'unapproved_rejection_unexpected') }
if (-not $rejected) { throw 'Unrelated failure became an unapproved-device pass' }
Write-Output 'PASS: unrelated CLI failure cannot count as expected rejection'
$badZip=Join-Path $root 'bad.zip'
Copy-Item -LiteralPath $SourceArchive -Destination $badZip
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip=[IO.Compression.ZipFile]::Open($badZip,[IO.Compression.ZipArchiveMode]::Update)
try {
    $entry=$zip.CreateEntry('../../escape.txt')
    $stream=$entry.Open()
    try { $stream.WriteByte(1) } finally { $stream.Dispose() }
} finally { $zip.Dispose() }
$rejected=$false
try { Read-TrustedImportBundle $badZip $probe (Join-Path $root 'bad-extract') | Out-Null } catch { $rejected=$true }
if (-not $rejected -or (Test-Path -LiteralPath (Join-Path $root 'bad-extract'))) { throw 'Unexpected archive entry was extracted' }
Write-Output 'PASS: archive traversal/extra entry rejected before extraction'
