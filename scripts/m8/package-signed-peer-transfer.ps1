param([Parameter(Mandatory=$true)][string]$SessionRoot,
    [Parameter(Mandatory=$true)][string]$Daemon,
    [string]$OutputDirectory=(Join-Path $env:USERPROFILE 'Downloads'))
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'peer-transfer-common.ps1')
$settings=Read-PeerSettings (Join-Path $SessionRoot 'package\peer-settings.json')
$fixture=Get-Content -LiteralPath (Join-Path $SessionRoot 'sender.json') -Raw | ConvertFrom-Json
$probe=[guid]::NewGuid().ToString('N')
$runtime=Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\sp\' + $probe.Substring(0,16))
if (Test-Path -LiteralPath $runtime) { throw 'fresh_signer_runtime_required' }
New-Item -ItemType Directory -Path $runtime | Out-Null
$sid=[Security.Principal.WindowsIdentity]::GetCurrent().User
$acl=New-Object Security.AccessControl.DirectorySecurity
$acl.SetOwner($sid)
$acl.SetAccessRuleProtection($true,$false)
$acl.AddAccessRule((New-Object Security.AccessControl.FileSystemAccessRule($sid,'FullControl','ContainerInherit,ObjectInherit','None','Allow')))
Set-Acl -LiteralPath $runtime -AclObject $acl
[IO.File]::WriteAllText((Join-Path $runtime 'pilot-runtime.marker'),"m0-native-desktop-runtime-v1`n",(New-Object Text.UTF8Encoding($false)))
& $Daemon pilot-fixture signed-transfer-v1 --runtime-root $runtime --confirm-isolated-pilot
if ($LASTEXITCODE -ne 0) { throw 'signed_fixture_creation_failed' }
$descriptor=Join-Path $runtime 'signed-probe.json'
$public=Get-Content -LiteralPath $descriptor -Raw | ConvertFrom-Json
$deviceStem=([BitConverter]::ToString([Text.Encoding]::UTF8.GetBytes($public.device_id))).Replace('-','').ToLowerInvariant()
$segment=Join-Path $runtime ('canonical\events\' + $deviceStem + '\00000000000000000000.seg')
$payloadName='signed-' + $probe + '.seg'
Copy-Item -LiteralPath $segment -Destination (Join-Path $fixture.root $payloadName)
$stage=Join-Path $SessionRoot ('signed-package-' + $probe)
New-Item -ItemType Directory -Path $stage | Out-Null
foreach ($name in @('peer-transfer-common.ps1','vm-peer-transfer.ps1','verify-signed-payload.ps1','IMZALI-TEST.cmd')) {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination $stage
}
Copy-Item -LiteralPath $Daemon -Destination (Join-Path $stage 'cryptexd.exe')
Copy-Item -LiteralPath $descriptor -Destination (Join-Path $stage 'signed-probe.json')
$settings.schema=2
$settings.bytes=(Get-Item -LiteralPath $segment).Length
$settings.sha256=(Get-FileHash -LiteralPath $segment).Hash
$settings | Add-Member NoteProperty probe_id $probe
$settings | Add-Member NoteProperty payload_name $payloadName
$settings | Add-Member NoteProperty daemon_sha256 (Get-FileHash -LiteralPath $Daemon).Hash
$settings | Add-Member NoteProperty descriptor_sha256 (Get-FileHash -LiteralPath $descriptor).Hash
Write-PeerJson (Join-Path $stage 'peer-settings.json') $settings
Read-PeerSettings (Join-Path $stage 'peer-settings.json') | Out-Null
[IO.File]::WriteAllText((Join-Path $stage 'BASLA.txt'),@'
VM'de ZIP'i ayiklayin ve IMZALI-TEST.cmd dosyasina cift tiklayin.
Ana bilgisayardaki gonderici hazir olmalidir. Test en fazla 3 dakika bekler.
Olusan IMZALI-SONUC.zip (veya AKTARIM-HATA.zip) dosyasini sohbete ekleyin.

Onceki basarili aktarim testinin VM profili kullanilir. Yeniden eslesme yoktur.
Daemonun OS guvenli deposundaki anahtarla imzaladigi tek bir sentetik kayit
aktarilir. VM'deki cryptexd cerceveyi, kaydi, icerik hash'ini, Ed25519 imzasini
ve paketle verilen genel kimligi dogrular. Degistirilmis ayri kopya reddedilir.
Alinan dosyanin kendisi degistirilmez. Ozel anahtar tasinmaz.

Bu test imza dogrulamasidir; guven listesine kabul veya canonical import degildir.
Eski Cryptex verileri/ag/antivirus ayarlari degistirilmez.
'@)
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$zip=Join-Path $OutputDirectory ('cryptex-imzali-' + $probe.Substring(0,8) + '.zip')
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
Write-Output ('signed_package_directory=' + $stage)
Write-Output ('package=' + $zip)
