param([Parameter(Mandatory=$true)][string]$Daemon,[string]$OutputDirectory=(Join-Path $env:USERPROFILE 'Downloads'))
$ErrorActionPreference='Stop'
$probe=[guid]::NewGuid().ToString('D')
$stage=Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\trusted-packages\' + $probe)
New-Item -ItemType Directory -Path $stage -Force | Out-Null
foreach ($name in @('GUVEN-HAZIRLA.cmd','prepare-trusted-import-vm.ps1')) {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination $stage
}
Copy-Item -LiteralPath $Daemon -Destination (Join-Path $stage 'cryptexd.exe')
$settings=@{schema=1;probe_id=$probe;daemon_sha256=(Get-FileHash -LiteralPath $Daemon).Hash;
    brain_id='99e94c1a-0c62-48a0-88d2-61b9a146e021';
    source_profile='cx-m8-c15206e78335472aac5324aa38a4b9b4';source_device='74ab8018-fc82-4815-84c6-48d728578184';
    receiver_profile='cx-m8-7333e750039442ca88303598be79f625';receiver_device='a61f5092-6a37-4f7c-acc6-e470d6f438c2'}
[IO.File]::WriteAllText((Join-Path $stage 'trusted-probe-settings.json'),($settings | ConvertTo-Json),(New-Object Text.UTF8Encoding($false)))
[IO.File]::WriteAllText((Join-Path $stage 'BASLA.txt'),@'
VM icinde ZIP'i ayiklayin ve GUVEN-HAZIRLA.cmd dosyasini acin.
Olusan GUVEN-HAZIRLIK.zip veya GUVEN-HATA.zip dosyasini sohbete ekleyin.
Cryptex test uygulamasi kapali olsun. Terminal kodu/yeni eslesme gerekmez.

Bu ilk adim, VM'deki ONCEDEN sahibi tarafindan onaylanmis iki izole test
profilinin tam imzali guven gecmisini kontrol eder. Yeni guven onayi olusturmaz.
Yalnizca onceden bilinen sentetik notlari olan kaynak profil kabul edilir.
Daemon kaynaga bir sentetik test notu ekleyip muhurlu segmenti hazirlar.
Alicinin turetilmis arama/veritabani, dogrulanmis mevcut gecmisten yeniden
kurulur. Imzali ayni-Brain ama onaysiz cihazdan bir negatif test kaydi da uretilir.

Disari sadece sentetik muhurlu kayitlar ve genel manifest cikar; ozel anahtarlar
VM'nin OS guvenli deposunda kalir. Eski canonical olaylar degistirilmez.
Ana bilgisayar bu dosyalari bir sonraki aktarimda YALNIZCA ARACI olarak tasir;
guvenilen Cryptex kaynak ve alici profilleri bu VM'dedir. Bu hazirlik henüz ag
aktarimi/ice aktarma sonucu degildir. Sonraki paket daemonun validate-incoming
kurallariyla guvenilir kaydi dogrulayacak; onaysiz/bozuk kayitlari reddedecektir.
Kalici canonical yayinlama bu testle acilmaz.
'@)
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$zip=Join-Path $OutputDirectory ('cryptex-guven-hazirlik-' + $probe.Substring(0,8) + '.zip')
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
Write-Output ('probe_id=' + $probe)
Write-Output ('package=' + $zip)
