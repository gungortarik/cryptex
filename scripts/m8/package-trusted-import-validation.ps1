param([Parameter(Mandatory=$true)][string]$SessionRoot,[Parameter(Mandatory=$true)][string]$SourceArchive,
    [Parameter(Mandatory=$true)][string]$Daemon,[string]$OutputDirectory=(Join-Path $env:USERPROFILE 'Downloads'))
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'peer-transfer-common.ps1')
. (Join-Path $PSScriptRoot 'trusted-import-bundle.ps1')
$settings=Read-PeerSettings (Join-Path $SessionRoot 'package\peer-settings.json')
$fixture=Get-Content -LiteralPath (Join-Path $SessionRoot 'sender.json') -Raw | ConvertFrom-Json
$probe='c223e671-3c5e-413d-a82c-419ed88a508f'
$bundle=Read-TrustedImportBundle $SourceArchive $probe (Join-Path $SessionRoot ('relay-source-'+[guid]::NewGuid().ToString('N').Substring(0,8)))
$manifest=$bundle.manifest
if ($manifest.brain_id -cne '99e94c1a-0c62-48a0-88d2-61b9a146e021' -or $manifest.source_device -cne '74ab8018-fc82-4815-84c6-48d728578184' -or
    $manifest.receiver_device -cne 'a61f5092-6a37-4f7c-acc6-e470d6f438c2') { throw 'original_pair_binding_mismatch' }
$compact=$probe.Replace('-','')
$payloadName='trusted-'+$compact+'.zip'
$hash=(Get-FileHash -LiteralPath $SourceArchive).Hash
Copy-Item -LiteralPath $SourceArchive -Destination (Join-Path $fixture.root $payloadName)
if ((Get-FileHash -LiteralPath (Join-Path $fixture.root $payloadName)).Hash -cne $hash) { throw 'relay_source_changed' }
$stage=Join-Path $SessionRoot ('trusted-package-'+$compact)
if (Test-Path -LiteralPath $stage) { throw 'fresh_package_required' }
New-Item -ItemType Directory -Path $stage | Out-Null
foreach ($name in @('peer-transfer-common.ps1','vm-peer-transfer.ps1','verify-signed-payload.ps1','trusted-import-bundle.ps1','validate-trusted-import-payload.ps1','GUVEN-TEST.cmd')) {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination $stage
}
Copy-Item -LiteralPath $Daemon -Destination (Join-Path $stage 'cryptexd.exe')
$settings.schema=3
$settings.bytes=(Get-Item -LiteralPath $SourceArchive).Length
$settings.sha256=$hash
foreach ($entry in @{
    probe_id=$compact;trusted_probe_id=$probe;payload_name=$payloadName;daemon_sha256=(Get-FileHash -LiteralPath $Daemon).Hash;
    manifest_sha256=$bundle.manifest_sha256;brain_id=$manifest.brain_id;source_device=$manifest.source_device;receiver_device=$manifest.receiver_device;
    receiver_profile='cx-m8-7333e750039442ca88303598be79f625'
}.GetEnumerator()) { $settings | Add-Member NoteProperty $entry.Key $entry.Value }
Write-PeerJson (Join-Path $stage 'peer-settings.json') $settings
Read-PeerSettings (Join-Path $stage 'peer-settings.json') | Out-Null
[IO.File]::WriteAllText((Join-Path $stage 'BASLA.txt'),@'
Ana bilgisayar gondericisi hazirken VM'de ZIP'i ayiklayin ve GUVEN-TEST.cmd acin.
Olusan GUVEN-SONUC.zip (veya AKTARIM-HATA.zip) dosyasini sohbete ekleyin.
Cryptex uygulamasi kapali olsun. Yeni eslesme veya terminal komutu gerekmez.

Onceki onayli iki Cryptex test profili VM'dedir. Ana bilgisayar yalnizca araci
olarak sentetik segmentleri VM'ye iletir. VM'deki daemon kendi OS-held anahtari
ve yerel imzali onay gecmisiyle gercek validate-incoming yolunu calistirir.
Guvenilir batch dogrulanmali; gecerli imzali ama onaysiz cihaz reddedilmeli;
bozulmus kopya reddedilmeli; tekrar dogrulama basarili olmali.
Canonical olaylarin ve onay gunlugunun degismedigi de kontrol edilir.

Bu bir validated_only testi: staging dogrulanir ve atilir. Yeni guven onayi,
kalici canonical ice aktarma/yayinlama veya tam M8 tamamlanmasi degildir.
Ozel anahtarlar aktarilmaz; guvenlik duvari/antivirus ayarlari degistirilmez.
'@)
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$zip=Join-Path $OutputDirectory ('cryptex-guven-test-'+$compact.Substring(0,8)+'.zip')
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
Write-Output ('trusted_package_directory='+$stage)
Write-Output ('package='+$zip)
