param([Parameter(Mandatory=$true)][string]$Package)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive=[IO.Compression.ZipFile]::OpenRead($Package)
try {
    $names=@($archive.Entries | ForEach-Object FullName | Sort-Object)
    $expected=@('BASLA.txt','cryptexd.exe','GUVEN-HAZIRLA.cmd','prepare-trusted-import-vm.ps1','trusted-probe-settings.json' | Sort-Object)
    if (($names -join ',') -ne ($expected -join ',')) { throw 'Unexpected package contents' }
} finally { $archive.Dispose() }
$root=Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\trusted-package-tests\' + [guid]::NewGuid().ToString('N'))
Expand-Archive -LiteralPath $Package -DestinationPath $root
$configPath=Join-Path $root 'trusted-probe-settings.json'
$config=Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
if ((Get-FileHash -LiteralPath (Join-Path $root 'cryptexd.exe')).Hash -ne $config.daemon_sha256) { throw 'Bundled daemon mismatch' }
$config.source_profile='cx-m8-' + [guid]::NewGuid().ToString('N')
[IO.File]::WriteAllText($configPath,($config | ConvertTo-Json),(New-Object Text.UTF8Encoding($false)))
$output=@(& powershell.exe -NoProfile -File (Join-Path $root 'prepare-trusted-import-vm.ps1') -NoOpen)
if ($LASTEXITCODE -eq 0) { throw 'Missing profile became a success' }
$errorZip=@($output | Where-Object { $_ -like '*GUVEN-HATA.zip' })[-1]
if (-not (Test-Path -LiteralPath $errorZip)) { throw 'Single error archive missing' }
Write-Output 'PASS: package contains no event data/keys; daemon hash; missing profile gives error archive'
$tokens=$null;$parseErrors=$null
$ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $root 'prepare-trusted-import-vm.ps1'),[ref]$tokens,[ref]$parseErrors)
if ($parseErrors.Count -ne 0) { throw 'Parser failure' }
$definition=$ast.Find({param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Export-ProbeSegment'},$true)
. ([ScriptBlock]::Create($definition.Extent.Text))
$segmentRoot=Join-Path $root 'fixture'
$relative=('a'*72) + '/00000000000000000000.seg'
$source=Join-Path $segmentRoot ('canonical\events\' + $relative.Replace('/','\'))
New-Item -ItemType Directory -Path (Split-Path $source -Parent) -Force | Out-Null
[IO.File]::WriteAllBytes($source,[byte[]](1,2,3))
$hash=(Get-FileHash -LiteralPath $source).Hash
$segment=[pscustomobject]@{relative_path=$relative;sha256=$hash;bytes=3}
$destination=Join-Path $root 'export'
Export-ProbeSegment $segmentRoot $segment $destination
if ((Get-FileHash -LiteralPath $source).Hash -ne $hash) { throw 'Source changed during export' }
$segment.relative_path='../not-a-segment'
$rejected=$false
try { Export-ProbeSegment $segmentRoot $segment $destination } catch { $rejected=$true }
if (-not $rejected) { throw 'Traversal accepted' }
$segment.relative_path=$relative
$segment.sha256='0'*64
$rejected=$false
try { Export-ProbeSegment $segmentRoot $segment $destination } catch { $rejected=$true }
if (-not $rejected) { throw 'Mismatched source accepted' }
Write-Output 'PASS: read-only export preserves source; traversal and hash mismatch rejected'
