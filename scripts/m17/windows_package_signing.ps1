# Only a disposable hosted account may create and trust the one-day fixture.
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or
    $env:RUNNER_OS -ne 'Windows') { throw 'Disposable hosted Windows required' }

function Assert-ImageSigner([string]$ImagePath, [string]$ExpectedPin) {
    $signature = Get-AuthenticodeSignature -LiteralPath $ImagePath
    if ($signature.Status -ne 'Valid') { throw "Invalid Authenticode signature on $ImagePath ($($signature.Status))" }
    if ($signature.SignerCertificate.GetCertHashString('SHA256') -cne $ExpectedPin) {
        throw 'Unexpected Authenticode signer'
    }
}

$repo = (Resolve-Path -LiteralPath "$PSScriptRoot/../..").Path
$fixture = New-Item -ItemType Directory -Path (Join-Path $env:RUNNER_TEMP ([guid]::NewGuid()))
$cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=Cryptex CI $($fixture.Name)" `
    -KeyExportPolicy NonExportable -HashAlgorithm SHA256 -NotAfter (Get-Date).AddDays(1) `
    -CertStoreLocation Cert:\CurrentUser\My
$publicPath = Join-Path $fixture.FullName 'signer.cer'
# The hosted runner is disposable. CurrentUser\Root requests an interactive
# confirmation; install only the public code-signing leaf in the VM trust store.
$rootPath = "Cert:\LocalMachine\Root\$($cert.Thumbprint)"
$ownsTrustEntry = $false
try {
    Export-Certificate -Cert $cert -FilePath $publicPath | Out-Null
    $pin = (Get-FileHash -LiteralPath $publicPath -Algorithm SHA256).Hash
    if (Test-Path -LiteralPath $rootPath) { throw 'Unexpected existing fixture trust' }
    $ownsTrustEntry = $true
    Write-Output 'Installing the public CI signing certificate in the disposable VM'
    Import-Certificate -CertStoreLocation Cert:\LocalMachine\Root -FilePath $publicPath | Out-Null
    $config = Join-Path $fixture.FullName 'signing.json'
    @{bundle=@{windows=@{certificateThumbprint=$cert.Thumbprint; digestAlgorithm='sha256'}}} |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $config
    $staged = @(Get-ChildItem -LiteralPath "$repo/apps/desktop/src-tauri/binaries" -Filter 'cryptexd-*.exe')
    if ($staged.Count -ne 1) { throw 'Expected one staged daemon' }
    $null = Set-AuthenticodeSignature -FilePath $staged[0].FullName -Certificate $cert -HashAlgorithm SHA256
    Assert-ImageSigner $staged[0].FullName $pin
    Write-Output 'Staged daemon signature verified; building NSIS'
    Push-Location -LiteralPath "$repo/apps/desktop"
    try {
        pnpm exec tauri bundle --ci --bundles nsis --config src-tauri/tauri.bundle.conf.json --config $config
        if ($LASTEXITCODE -ne 0) { throw 'Signed NSIS packaging failed' }
    } finally { Pop-Location }
    $installers = @(Get-ChildItem -LiteralPath "$repo/target/release/bundle/nsis" -Filter '*-setup.exe')
    if ($installers.Count -ne 1) { throw 'Expected one signed NSIS installer' }
    $installer = $installers[0].FullName
    Assert-ImageSigner $installer $pin
    $verifyInstaller = Join-Path $PSScriptRoot 'verify_first_install_windows.ps1'
    $release = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
    & $verifyInstaller $installer $pin $release
    foreach ($case in @(
        @{ signer = ('0' * 64); release = $release; reason = 'Unexpected Cryptex signer' },
        @{ signer = $pin; release = ('0' * 64); reason = 'Different Cryptex release' }
    )) {
        try {
            & $verifyInstaller $installer $case.signer $case.release
            throw 'First-install verification accepted unexpected owner or release'
        } catch {
            if ($_.Exception.Message -ne $case.reason) { throw }
        }
    }
    $destination = Join-Path $fixture.FullName 'Applications'
    $proc = Start-Process -FilePath $installer -ArgumentList @('/S', "/D=$destination") -PassThru -WindowStyle Hidden
    if (-not $proc.WaitForExit(180000)) { $proc.Kill($true); throw 'Installer timed out' }
    if ($proc.ExitCode -ne 0) { throw "Installer exited with $($proc.ExitCode)" }
    python -B "$PSScriptRoot/check_installed_provenance.py" (Join-Path $destination 'build-provenance.json')
    if ($LASTEXITCODE -ne 0) { throw 'Installed package provenance verification failed' }
    $notices = Get-FileHash -LiteralPath (Join-Path $destination 'notices/rust.json')
    $sourceNotices = Get-FileHash -LiteralPath "$repo/apps/desktop/src-tauri/binaries/rust-notices.json"
    if ($notices.Hash -ne $sourceNotices.Hash) { throw 'Installed Rust inventory differs from the input' }
    foreach ($file in 'cryptex-desktop.exe','cryptexd.exe') {
        $image = Join-Path $destination $file
        Assert-ImageSigner $image $pin
        try {
            Assert-ImageSigner $image ('0' * 64)
            throw 'Wrong signer accepted'
        } catch {
            if ($_.Exception.Message -ne 'Unexpected Authenticode signer') { throw }
        }
    }
    $edited = Join-Path $fixture.FullName 'changed.exe'
    Copy-Item -LiteralPath (Join-Path $destination 'cryptexd.exe') -Destination $edited
    # The standard DOS stub is covered by the Authenticode digest.
    $bytes = [IO.File]::ReadAllBytes($edited)
    $bytes[80] = $bytes[80] -bxor 1
    [IO.File]::WriteAllBytes($edited, $bytes)
    if ((Get-AuthenticodeSignature -LiteralPath $edited).Status -ne 'HashMismatch') {
        throw 'Expected native hash mismatch on modified daemon'
    }
    try {
        & $verifyInstaller $edited $pin (Get-FileHash -LiteralPath $edited).Hash
        throw 'First-install verification accepted modified signed code'
    } catch {
        if ($_.Exception.Message -ne 'Invalid installer signature (HashMismatch)') { throw }
    }
    # M14 verifies creation of this exact per-user registration at login.
    # Exercise removal by the ordinary signed installer with its real product name.
    $startup = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Software\Microsoft\Windows\CurrentVersion\Run')
    $neighbor = "Cryptex-CI-$($fixture.Name)"
    $entry = '"' + (Join-Path $destination 'cryptex-desktop.exe') + '" --cryptex-background'
    $ownsStartup = $false
    $ownsNeighbor = $false
    try {
        if ($startup.GetValueNames() -contains 'Cryptex' -or $startup.GetValueNames() -contains $neighbor) {
            throw 'Unexpected existing startup registration in disposable account'
        }
        $startup.SetValue('Cryptex', $entry)
        $ownsStartup = $true
        $startup.SetValue($neighbor, '')
        $ownsNeighbor = $true
        $uninstaller = Join-Path $destination 'uninstall.exe'
        $remove = Start-Process -FilePath $uninstaller -ArgumentList '/S' -PassThru -WindowStyle Hidden
        if (-not $remove.WaitForExit(60000)) { throw 'Uninstaller timed out' }
        if ($remove.ExitCode -ne 0) { throw "Uninstaller exited with $($remove.ExitCode)" }
        # NSIS can hand off to a temporary copy and return before its child finishes.
        $limit = [DateTime]::UtcNow.AddSeconds(45)
        do {
            $pending = ($startup.GetValueNames() -contains 'Cryptex') -or
                (Test-Path -LiteralPath (Join-Path $destination 'cryptex-desktop.exe')) -or
                (Test-Path -LiteralPath (Join-Path $destination 'cryptexd.exe'))
            if ($pending) { Start-Sleep -Milliseconds 200 }
        } while ($pending -and [DateTime]::UtcNow -lt $limit)
        if ($pending -or $startup.GetValueNames() -notcontains $neighbor) {
            throw 'Uninstall must remove its startup registration and binaries and preserve unrelated entries'
        }
        Write-Output 'Verified ordinary NSIS uninstall and per-user startup removal.'
    } finally {
        # The worker account is discarded. Remove only fixture values we created.
        if ($ownsNeighbor) { $startup.DeleteValue($neighbor, $false) }
        if ($ownsStartup -and $startup.GetValue('Cryptex') -ceq $entry) { $startup.DeleteValue('Cryptex', $false) }
        $startup.Dispose()
    }
    Write-Output 'Signed NSIS, installed desktop and daemon matched the fixture pin. Wrong signer and changed file refused.'
} finally {
    if ($ownsTrustEntry -and (Test-Path -LiteralPath $rootPath)) { Remove-Item -LiteralPath $rootPath }
    Remove-Item -LiteralPath "Cert:\CurrentUser\My\$($cert.Thumbprint)" -DeleteKey
}
