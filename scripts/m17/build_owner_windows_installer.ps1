param(
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{64}$')][string]$CertificateSha256,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{64}$')][string]$DesktopSha256,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{64}$')][string]$DaemonSha256,
    [Parameter(Mandatory)][string]$OutputInstaller
)
# Run in PowerShell 7 on the signing machine after the reviewed release build.
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path -LiteralPath "$PSScriptRoot/../..").Path
$desktop = Join-Path $repo 'target/release/cryptex-desktop.exe'
$sidecars = @(Get-ChildItem -LiteralPath "$repo/apps/desktop/src-tauri/binaries" -Filter 'cryptexd-*.exe' -File)
if ($sidecars.Count -ne 1) { throw 'Expected one staged daemon for the release build' }
$output = [IO.Path]::GetFullPath($OutputInstaller)
if ([IO.Path]::GetExtension($output) -ine '.exe' -or (Test-Path -LiteralPath $output)) {
    throw 'Choose a new installer executable path'
}
$parent = (Get-Item -LiteralPath ([IO.Path]::GetDirectoryName($output))).FullName
if ((Get-FileHash -LiteralPath $desktop).Hash -ine $DesktopSha256 -or
    (Get-FileHash -LiteralPath $sidecars[0].FullName).Hash -ine $DaemonSha256) {
    throw 'Unsigned build differs from the approved image hashes'
}
$identities = @(Get-ChildItem Cert:\CurrentUser\My | Where-Object {
    $_.HasPrivateKey -and $_.GetCertHashString('SHA256') -ieq $CertificateSha256
})
if ($identities.Count -ne 1) { throw 'Expected one Windows signing identity with the approved certificate SHA-256' }
$cert = $identities[0]
$tempBase = (Get-Item -LiteralPath ([IO.Path]::GetTempPath())).FullName.TrimEnd('\')
$scratch = [IO.Directory]::CreateTempSubdirectory('cryptex-owner-package-').FullName
$targetBefore = [Environment]::GetEnvironmentVariable('CARGO_TARGET_DIR')
$sidecarStage = $null
$copiedDaemon = $null
$provenanceLock = $null
$transportLocks = @()
try {
    $provenanceFile = Join-Path $repo 'apps/desktop/src-tauri/binaries/build-provenance.json'
    # NSIS reads this resource later. Keep the verified file read-only until bundling finishes.
    $provenanceLock = [IO.File]::Open($provenanceFile, 'Open', 'Read', 'Read')
    $record = Get-Content -Raw -LiteralPath $provenanceFile | ConvertFrom-Json
    $source = git -C $repo rev-parse --verify HEAD
    if ($LASTEXITCODE -ne 0 -or $source -cnotmatch '^[0-9a-f]{40}$' -or
        $record.target -cne 'x86_64-pc-windows-msvc' -or $record.source.commit -cne $source -or
        $record.source.clean -isnot [bool] -or !$record.source.clean) {
        throw 'Windows build provenance differs from the clean reviewed revision or target'
    }
    $changes = git -C $repo status --porcelain=v1 --untracked-files=normal
    if ($LASTEXITCODE -ne 0 -or $changes) { throw 'Windows packaging requires a clean release checkout' }
    $transportSource = Join-Path $repo 'apps/desktop/src-tauri/binaries/transport'
    python -B "$PSScriptRoot/owner_transport.py" verify $transportSource x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Invalid transport packaging input' }
    $env:CARGO_TARGET_DIR = Join-Path $scratch 'target'
    $release = [IO.Directory]::CreateDirectory((Join-Path $env:CARGO_TARGET_DIR 'release')).FullName
    $copiedDesktop = Join-Path $release 'cryptex-desktop.exe'
    # Tauri normalizes externalBin paths without the Windows drive prefix.
    # Keep the signed copy at a unique path relative to the Tauri build manifest.
    $relativeStage = 'binaries/owner-' + [guid]::NewGuid().ToString('N')
    $sidecarStage = New-Item -ItemType Directory -Path (Join-Path "$repo/apps/desktop/src-tauri" $relativeStage)
    $copiedDaemon = Join-Path $sidecarStage.FullName $sidecars[0].Name
    Copy-Item -LiteralPath $desktop -Destination $copiedDesktop
    Copy-Item -LiteralPath $sidecars[0].FullName -Destination $copiedDaemon
    if ((Get-FileHash -LiteralPath $copiedDesktop).Hash -ine $DesktopSha256 -or
        (Get-FileHash -LiteralPath $copiedDaemon).Hash -ine $DaemonSha256) {
        throw 'An approved image changed during staging'
    }
    $signed = Set-AuthenticodeSignature -FilePath $copiedDaemon -Certificate $cert -HashAlgorithm SHA256
    if ($signed.Status -ne 'Valid' -or $signed.SignerCertificate.GetCertHashString('SHA256') -ine $CertificateSha256) {
        throw 'Staged daemon signature verification failed'
    }
    # NSIS must read only the four reviewed files. Use explicit mappings and hold
    # their handles without write/delete sharing through installer generation.
    $transportMap = @{ 'binaries/transport' = $null }
    foreach ($member in 'syncthing.exe','LICENSE.txt','AUTHORS.txt','NOTICES.txt') {
        $transportLocks += [IO.File]::Open((Join-Path $transportSource $member), 'Open', 'Read', 'Read')
        $transportMap["binaries/transport/$member"] = "transport/$member"
    }
    python -B "$PSScriptRoot/owner_transport.py" verify $transportSource x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Transport changed during package staging' }
    $config = Join-Path $scratch 'signing.json'
    @{ bundle = @{
        resources = $transportMap
        externalBin = @("$relativeStage/cryptexd")
        windows = @{ certificateThumbprint = $cert.Thumbprint; digestAlgorithm = 'sha256' }
    }} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $config
    Push-Location -LiteralPath "$repo/apps/desktop"
    try {
        pnpm exec tauri bundle --ci --bundles nsis --config src-tauri/tauri.bundle.conf.json --config $config | Out-Host
        if ($LASTEXITCODE -ne 0) { throw 'Signed NSIS packaging failed' }
    } finally { Pop-Location }
    $installers = @(Get-ChildItem -LiteralPath (Join-Path $release 'bundle/nsis') -Filter '*-setup.exe' -File)
    if ($installers.Count -ne 1) { throw 'Expected one new signed installer' }
    $version = $installers[0].VersionInfo.ProductVersion
    if ($version -notmatch '^([0-9]+\.[0-9]+\.[0-9]+)(?:\.0)?$') { throw 'Unexpected NSIS product version' }
    $version = $Matches[1]
    $hash = (Get-FileHash -LiteralPath $installers[0].FullName).Hash
    $pending = Join-Path $parent ([guid]::NewGuid().ToString() + '.exe')
    [IO.File]::Copy($installers[0].FullName, $pending, $false)
    try {
        & "$PSScriptRoot/verify_first_install_windows.ps1" $pending $CertificateSha256 $hash | Out-Host
        [IO.File]::Move($pending, $output)
    } finally {
        if ([IO.File]::Exists($pending)) { [IO.File]::Delete($pending) }
    }
    [pscustomobject]@{
        Installer = $output
        Version = $version
        Platform = 'windows'
        Source = $source
        ReleaseSha256 = $hash
        CertificateSha256 = $CertificateSha256
    }
} finally {
    foreach ($handle in $transportLocks) { $handle.Dispose() }
    if ($provenanceLock) { $provenanceLock.Dispose() }
    $env:CARGO_TARGET_DIR = $targetBefore
    if ($copiedDaemon -and [IO.File]::Exists($copiedDaemon)) { [IO.File]::Delete($copiedDaemon) }
    if ($sidecarStage) { [IO.Directory]::Delete($sidecarStage.FullName, $false) }
    $resolved = (Resolve-Path -LiteralPath $scratch).Path
    if ([IO.Path]::GetDirectoryName($resolved) -ieq $tempBase -and
        [IO.Path]::GetFileName($resolved).StartsWith('cryptex-owner-package-') -and $resolved -ieq $scratch) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
