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
    $staged = @(Get-ChildItem -LiteralPath "$repo/apps/desktop/src-tauri/binaries" -Filter 'cryptexd-*.exe')
    if ($staged.Count -ne 1) { throw 'Expected one staged daemon' }
    $desktop = Join-Path $repo 'target/release/cryptex-desktop.exe'
    $desktopHash = (Get-FileHash -LiteralPath $desktop).Hash
    $daemonHash = (Get-FileHash -LiteralPath $staged[0].FullName).Hash
    $installer = Join-Path $fixture.FullName 'owner-setup.exe'
    $package = Join-Path $PSScriptRoot 'build_owner_windows_installer.ps1'
    foreach ($case in @(
        @{ pin = $pin; daemon = ('0' * 64); reason = 'Unsigned build differs from the approved image hashes' },
        @{ pin = ('0' * 64); daemon = $daemonHash; reason = 'Expected one Windows signing identity with the approved certificate SHA-256' }
    )) {
        try {
            & $package $case.pin $desktopHash $case.daemon $installer
            throw 'Unapproved Windows packaging accepted'
        } catch {
            if ($_.Exception.Message -ne $case.reason) { throw }
        }
        if (Test-Path -LiteralPath $installer) { throw 'Refused packaging published an installer' }
    }
    $callerTarget = [Environment]::GetEnvironmentVariable('CARGO_TARGET_DIR')
    $provenance = Join-Path $repo 'apps/desktop/src-tauri/binaries/build-provenance.json'
    $original = [IO.File]::ReadAllBytes($provenance)
    try {
        foreach ($field in 'revision','dirty','target') {
            $record = [Text.Encoding]::UTF8.GetString($original) | ConvertFrom-Json
            switch ($field) {
                revision { $record.source.commit = '0' * 40 }
                dirty { $record.source.clean = $false }
                target { $record.target = 'aarch64-apple-darwin' }
            }
            $record | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $provenance
            try {
                & $package $pin $desktopHash $daemonHash $installer
                throw "Owner packaging accepted unexpected provenance ($field)"
            } catch {
                if ($_.Exception.Message -ne 'Windows build provenance differs from the clean reviewed revision or target') { throw }
            }
            if (Test-Path -LiteralPath $installer) { throw 'Rejected provenance published an installer' }
            if ($callerTarget -cne [Environment]::GetEnvironmentVariable('CARGO_TARGET_DIR')) {
                throw 'Rejected packaging changed the Cargo target directory'
            }
        }
    } finally {
        [IO.File]::WriteAllBytes($provenance, $original)
    }
    $report = & $package $pin $desktopHash $daemonHash $installer
    if ($callerTarget -cne [Environment]::GetEnvironmentVariable('CARGO_TARGET_DIR')) {
        throw 'Successful packaging changed the Cargo target directory'
    }
    if ($report.Installer -ne $installer -or (Get-FileHash -LiteralPath $desktop).Hash -ne $desktopHash -or
        (Get-FileHash -LiteralPath $staged[0].FullName).Hash -ne $daemonHash) { throw 'Approved build inputs changed' }
    $release = $report.ReleaseSha256
    $verifyInstaller = Join-Path $PSScriptRoot 'verify_first_install_windows.ps1'
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
    python -B "$PSScriptRoot/owner_transport.py" installed (Join-Path $destination transport) x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Installed official transport verification failed' }
    $info = [Diagnostics.FileVersionInfo]::GetVersionInfo((Join-Path $destination 'cryptex-desktop.exe'))
    $installedVersion = '{0}.{1}.{2}' -f $info.ProductMajorPart,$info.ProductMinorPart,$info.ProductBuildPart
    if ($report.Source -ne $env:GITHUB_SHA -or $report.Platform -ne 'windows' -or
        $report.Version -cne '1.0.2' -or $installedVersion -cne $report.Version -or $info.ProductPrivatePart -ne 0) {
        throw "Owner candidate version mismatch: installed $installedVersion, package $($report.Version)"
    }
    Write-Output "Verified Windows installed candidate $installedVersion and signed installer $release."
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
    $previousDaemon = Join-Path $env:RUNNER_TEMP 'm17-previous/cryptexd.exe'
    Set-AuthenticodeSignature -FilePath $previousDaemon -Certificate $cert -HashAlgorithm SHA256 | Out-Null
    Assert-ImageSigner $previousDaemon $pin
    $handoff = New-Item -ItemType Directory -Path (Join-Path $fixture.FullName 'paired-update')
    $oldImage = New-Item -ItemType Directory -Path (Join-Path $handoff.FullName 'working')
    Copy-Item -LiteralPath $previousDaemon -Destination (Join-Path $oldImage.FullName 'cryptexd.exe')
    $oldDesktop = Join-Path $oldImage.FullName 'cryptex-desktop.exe'
    Copy-Item -LiteralPath (Join-Path $env:RUNNER_TEMP 'm17-previous/cryptex-desktop.exe') -Destination $oldDesktop
    Set-AuthenticodeSignature -FilePath $oldDesktop -Certificate $cert -HashAlgorithm SHA256 | Out-Null
    Assert-ImageSigner $oldDesktop $pin
    Copy-Item -LiteralPath (Join-Path $env:RUNNER_TEMP 'm17-previous/seed-pair.exe') -Destination $handoff.FullName
    $env:CRYPTEX_UPDATE_SIGNER_DER = $publicPath
    cargo build --release --locked -p cryptex-updater --features owner-transport-update
    if ($LASTEXITCODE -ne 0) { throw 'Could not build the enrolled-profile updater' }
    $tool = Join-Path $handoff.FullName 'cryptex-updater.exe'
    python -B "$PSScriptRoot/update_fixture.py" (Join-Path $repo 'target/release/cryptex-updater.exe') $handoff.FullName
    if ($LASTEXITCODE -ne 0) { throw 'Could not verify the signed updater delivery' }
    Assert-ImageSigner $tool $pin
    foreach ($slot in @(@{name='working';dir=$oldImage.FullName}, @{name='candidate';dir=$destination})) {
        $approved = & $tool fingerprint $slot.dir
        if ($LASTEXITCODE -ne 0) { throw 'Could not fingerprint installed images' }
        python -B "$PSScriptRoot/sign_owner_update.py" $tool $slot.dir $approved ($pin.ToLowerInvariant()) (Join-Path $handoff.FullName ($slot.name + '.cms'))
        if ($LASTEXITCODE -ne 0) { throw 'Could not sign installed update receipt' }
    }
    $trial = @{
        PreviousDaemon = $previousDaemon
        PreviousDaemonHash = (Get-FileHash -LiteralPath $previousDaemon).Hash
        PairedUpdate = $handoff.FullName
        Installer = $installer; Certificate = $pin; Setup = $release
        Desktop = (Get-FileHash -LiteralPath (Join-Path $destination 'cryptex-desktop.exe')).Hash
        Daemon = (Get-FileHash -LiteralPath (Join-Path $destination 'cryptexd.exe')).Hash
    }
    & "$repo/scripts/m8/run-daemon-tests-standard-user-ci.ps1" -M14ReleaseSmoke -M17SignedSmoke $trial
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
