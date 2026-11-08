param([Parameter(Mandatory=$true)][string]$NativeBinary,
    [Parameter(Mandatory=$true)][string]$HostAddress,
    [string]$OutputDirectory=(Join-Path $env:USERPROFILE 'Downloads'))
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'peer-transfer-common.ps1')
$fixture = New-PeerFixture $NativeBinary
$session = [guid]::NewGuid().ToString('N')
$root = Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\peer-transfer\' + $session)
New-Item -ItemType Directory -Path $root -Force | Out-Null
$stage = Join-Path $root 'package'
New-Item -ItemType Directory -Path $stage | Out-Null
# Random synthetic bytes are never canonical events or personal content.
$bytes = New-Object byte[] 32768
$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
try { $rng.GetBytes($bytes) } finally { $rng.Dispose() }
$source = Join-Path $fixture.root 'synthetic.bin'
[IO.File]::WriteAllBytes($source,$bytes)
$config = New-Object Xml.XmlDocument
$config.Load((Join-Path $fixture.root 'profile\config.xml'))
$syncPort = ([Uri][string]$config.configuration.options.listenAddress).Port
$settings = @{schema=1; session=$session; host_id=$fixture.native_id; host_address=$HostAddress;
    host_port=$syncPort; bytes=32768; sha256=(Get-FileHash -LiteralPath $source).Hash}
Write-PeerJson (Join-Path $stage 'peer-settings.json') $settings
Read-PeerSettings (Join-Path $stage 'peer-settings.json') | Out-Null
Write-PeerJson (Join-Path $root 'sender.json') $fixture
foreach ($name in @('peer-transfer-common.ps1','prepare-owned-transport-fixture.ps1','vm-peer-transfer.ps1','1-HAZIRLA.cmd','2-AKTAR.cmd')) {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination $stage
}
Copy-Item -LiteralPath $NativeBinary -Destination (Join-Path $stage 'syncthing.exe')
foreach ($name in @('LICENSE.txt','AUTHORS.txt')) {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot "..\..\third_party\syncthing\2.1.5\$name") -Destination $stage
}
[IO.File]::WriteAllText((Join-Path $stage 'BASLA.txt'), @'
1. Bu paketi VM icinde ayiklayin.
2. 1-HAZIRLA.cmd dosyasini acin. Olusan ISTEK.zip dosyasini sohbete ekleyin.
3. Ana bilgisayar hazir oldugu soylenince 2-AKTAR.cmd dosyasini acin.
4. AKTARIM-SONUC.zip veya AKTARIM-HATA.zip dosyasini sohbete ekleyin.

Bu test iki ayri Syncthing kimligiyle 32 KiB rastgele test dosyasi aktarir.
Cryptex eslesmelerini/kayitlarini kullanmaz veya degistirmez.
Canonical import ve M8 kabul testi ayri olarak beklemektedir.
API sadece loopback'e baglanir. Global/yerel kesif, relay, NAT ve otomatik
guncelleme kapali. Guvenlik duvari/antivirus ayari degistirilmez.
Ozel anahtarlar sadece bu makinenin korumali test profilinde kalir.
Yonetici hakki gerekmez. Ag engeli varsa rapor basarisizlik gosterir.
'@)
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$zip = Join-Path $OutputDirectory ('cryptex-aktarim-' + $session.Substring(0,8) + '.zip')
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
Write-Output ('session_root=' + $root)
Write-Output ('package=' + $zip)
