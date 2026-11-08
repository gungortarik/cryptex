param([string] $OutputDirectory = (Join-Path $env:LOCALAPPDATA 'Cryptex-Dev\vm-check-packages'))
$ErrorActionPreference = 'Stop'
$stage = Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\vm-check-packages\stage-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stage -Force | Out-Null
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$files = @('VM-KONTROL.cmd','collect-vm-check.ps1','VM-KONTROL-README.txt')
foreach ($file in $files) {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $file) -Destination $stage
}
$manifest = @($files | ForEach-Object {
    [pscustomobject]@{ file = $_; sha256 = (Get-FileHash -LiteralPath (Join-Path $stage $_) -Algorithm SHA256).Hash }
})
[IO.File]::WriteAllText((Join-Path $stage 'FILES.json'), ($manifest | ConvertTo-Json), (New-Object Text.UTF8Encoding($false)))
$zip = Join-Path $OutputDirectory 'cryptex-vm-kontrol.zip'
if (Test-Path -LiteralPath $zip) { throw 'Package already exists; select another output directory.' }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
Write-Output $zip
