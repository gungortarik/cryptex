# Run the baseline, candidate and rollback under a separate standard account.
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'Disposable Windows runner required'
}
$packages = Join-Path $env:RUNNER_TEMP 'm14-packages'
$public = Join-Path $env:RUNNER_TEMP (([guid]::NewGuid()).ToString() + '.cer')
$cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=Cryptex CI update $([guid]::NewGuid())" `
    -KeyExportPolicy NonExportable -HashAlgorithm SHA256 -NotAfter (Get-Date).AddDays(1) -CertStoreLocation Cert:\CurrentUser\My
$root = "Cert:\LocalMachine\Root\$($cert.Thumbprint)"
$ownedRoot = $false
try {
    Export-Certificate -Cert $cert -FilePath $public | Out-Null
    if (Test-Path -LiteralPath $root) { throw 'Unexpected certificate collision' }
    $ownedRoot = $true
    Import-Certificate -CertStoreLocation Cert:\LocalMachine\Root -FilePath $public | Out-Null
    $env:CRYPTEX_UPDATE_SIGNER_DER = $public
    $env:CX_UPDATE_CI_SIGNER = $cert.Thumbprint
    python -B scripts/m14/installed_update.py build $packages
    if ($LASTEXITCODE -ne 0) { throw 'Could not build signed update packages' }
    Remove-Item -LiteralPath "Cert:\CurrentUser\My\$($cert.Thumbprint)" -DeleteKey
    Remove-Item Env:\CX_UPDATE_CI_SIGNER,Env:\CRYPTEX_UPDATE_SIGNER_DER
    & "$PSScriptRoot/../m8/run-daemon-tests-standard-user-ci.ps1" -M14UpdatePackages $packages
    if ($LASTEXITCODE -ne 0) { throw 'Standard account update failed' }
} finally {
    if ($ownedRoot -and (Test-Path -LiteralPath $root)) { Remove-Item -LiteralPath $root }
    if (Test-Path -LiteralPath "Cert:\CurrentUser\My\$($cert.Thumbprint)") {
        Remove-Item -LiteralPath "Cert:\CurrentUser\My\$($cert.Thumbprint)" -DeleteKey
    }
}
