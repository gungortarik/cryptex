param([Parameter(Mandatory=$true)][string]$NativeBinary)
$ErrorActionPreference = 'Stop'
$nativeItem = Get-Item -LiteralPath $NativeBinary -ErrorAction Stop
if ($nativeItem -isnot [IO.FileInfo]) { throw 'Native binary must be an existing filesystem file' }
$NativeBinary = $nativeItem.FullName
$source = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$manifest = Join-Path $source 'evidence\m8\m8-startup-status-source-sha256-20261022.txt'
$utf8 = New-Object Text.UTF8Encoding($false)
foreach ($line in [IO.File]::ReadAllLines($manifest)) {
    if (-not $line) { continue }
    $parts = $line -split '  ', 2
    if ($parts.Length -ne 2 -or $parts[0] -notmatch '^[a-f0-9]{64}$' -or $parts[1] -match '(^|/)\.\.(/|$)') { throw 'Invalid source manifest' }
    $file = Join-Path $source $parts[1]
    $normalized = [Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes($file)).Replace("`r`n", "`n")
    $sha = [Security.Cryptography.SHA256]::Create()
    try { $actual = ([BitConverter]::ToString($sha.ComputeHash($utf8.GetBytes($normalized)))).Replace('-','').ToLowerInvariant() } finally { $sha.Dispose() }
    if ($actual -ne $parts[0]) { throw ('Source mismatch: ' + $parts[1]) }
}
$proof = Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\m8-verification\profile-batch-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $proof | Out-Null
$env:CARGO_TARGET_DIR = Join-Path $env:LOCALAPPDATA 'Cryptex-Dev\build\m8-managed-profile-guest'
$nativeFixture = Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\m8-verification\m8-owned-native-' + [guid]::NewGuid().ToString('N'))
$results = New-Object 'System.Collections.Generic.List[object]'
function Invoke-CargoStep([string]$Name, [string[]]$Arguments) {
    $cargoPath = (Get-Command cargo -CommandType Application -ErrorAction Stop).Source; $global:LASTEXITCODE = 1; $savedPreference = $ErrorActionPreference; try { $ErrorActionPreference = 'Continue'; & $cargoPath @Arguments *> (Join-Path $proof ($Name + '.log')) } finally { $ErrorActionPreference = $savedPreference }
    $code = $global:LASTEXITCODE
    $results.Add([ordered]@{step=$Name; exit=$code})
    [IO.File]::WriteAllText((Join-Path $proof 'results.json'), (ConvertTo-Json -InputObject @($results.ToArray()) -Depth 5), $utf8)
    if ($code -ne 0) { throw ('Verification stopped at ' + $Name + '; see ' + $proof) }
}
Push-Location $source
try {
    Invoke-CargoStep 'fmt' @('fmt','--all','--','--check')
    Invoke-CargoStep 'package-tests' @('test','--locked','-p','cryptexd','-p','cryptex-events')
    Invoke-CargoStep 'clippy' @('clippy','--locked','-p','cryptexd','--all-targets','--','-D','warnings')
    Invoke-CargoStep 'build' @('build','--locked','-p','cryptexd')
    & (Join-Path $PSScriptRoot 'prepare-owned-transport-fixture.ps1') -Binary $NativeBinary -FixtureRoot $nativeFixture
    $previousFixture = $env:CRYPTEX_M8_NATIVE_FIXTURE
    $previousDaemonBinary = $env:CRYPTEX_M8_DAEMON_BINARY
    try {
        $env:CRYPTEX_M8_NATIVE_FIXTURE = $nativeFixture
        $env:CRYPTEX_M8_DAEMON_BINARY = Join-Path $env:CARGO_TARGET_DIR 'debug\cryptexd.exe'
        Invoke-CargoStep 'native-owned' @('test','--locked','-p','cryptexd','--lib','owned_process::tests::native_syncthing_lifecycle_fixture','--','--ignored','--exact','--test-threads=1')
        Invoke-CargoStep 'native-private' @('test','--locked','-p','cryptexd','--lib','owned_process::tests::native_syncthing_private_profile_and_os_credential_fixture','--','--ignored','--exact','--test-threads=1')
        Invoke-CargoStep 'native-pinned-package' @('test','--locked','-p','cryptexd','--lib','syncthing_package::tests::pinned_package_owned_generation_fixture','--','--ignored','--exact','--test-threads=1')
        Invoke-CargoStep 'native-provision' @('test','--locked','-p','cryptexd','--lib','managed_profile::native::tests::real_pinned_profile_provision_reopen_and_collision_recovery','--','--ignored','--exact','--test-threads=1')
    } finally { $env:CRYPTEX_M8_NATIVE_FIXTURE = $previousFixture; $env:CRYPTEX_M8_DAEMON_BINARY = $previousDaemonBinary }
    foreach ($name in @('native-result.json','native-private-profile-result.json','native-pinned-package-result.json','native-provision-result.json')) {
        Copy-Item -LiteralPath (Join-Path $nativeFixture $name) -Destination (Join-Path $proof $name)
    }
    Write-Output ('Scoped Windows verification PASS. Report: ' + $proof)
    Write-Output 'This does not complete M8. No Actions, peer pairing or canonical writes were enabled.'
} finally { Pop-Location }
