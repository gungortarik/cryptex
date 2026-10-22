param([Parameter(Mandatory=$true)][string]$NativeBinary)
$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSVersion.Major -ne 5) { throw 'Run with Windows PowerShell 5.1' }
$binary = Get-Item -LiteralPath $NativeBinary -ErrorAction Stop
if ($binary -isnot [IO.FileInfo]) { throw 'Native binary must be a file' }
$proof = Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\m8-verification\path-regression-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $proof | Out-Null
$fixture = Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\m8-verification\m8-owned-native-' + [guid]::NewGuid().ToString('N'))
$prepare = Join-Path $PSScriptRoot 'prepare-owned-transport-fixture.ps1'
$originalDotNetDirectory = [Environment]::CurrentDirectory
Push-Location $binary.DirectoryName
try {
    [Environment]::CurrentDirectory = $proof
    $relative = '.\' + $binary.Name
    if ([IO.Path]::GetFullPath($relative) -eq $binary.FullName) { throw 'Regression did not separate the two directory contexts' }
    & $prepare -Binary $relative -FixtureRoot $fixture
    $actual = Get-Content -LiteralPath (Join-Path $fixture 'fixture.json') -Raw | ConvertFrom-Json
    if ($actual.binary -ne $binary.FullName) { throw 'Recorded native path mismatch' }
    if (-not (Test-Path -LiteralPath (Join-Path $fixture 'profile\cert.pem'))) { throw 'Native generation did not finish' }
    [IO.File]::WriteAllText((Join-Path $proof 'result.txt'), "PASS: relative PowerShell path resolved before native launch; no server launched.`n", (New-Object Text.UTF8Encoding($false)))
    Write-Output ('PASS native path regression. Proof: ' + $proof)
} finally {
    [Environment]::CurrentDirectory = $originalDotNetDirectory
    Pop-Location
}
