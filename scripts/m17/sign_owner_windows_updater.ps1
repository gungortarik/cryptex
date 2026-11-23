param(
    [Parameter(Mandatory)][string]$Updater,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{64}$')][string]$UnsignedSha256,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{64}$')][string]$CertificateSha256
)
$ErrorActionPreference = 'Stop'
if ((Get-FileHash -LiteralPath $Updater).Hash -ine $UnsignedSha256) { throw 'Different unsigned updater' }
$identities = @(Get-ChildItem Cert:\CurrentUser\My | Where-Object {
    $_.HasPrivateKey -and $_.GetCertHashString('SHA256') -ieq $CertificateSha256
})
if ($identities.Count -ne 1) { throw 'Expected one signing identity with the selected certificate SHA-256' }
$null = Set-AuthenticodeSignature -FilePath $Updater -Certificate $identities[0] -HashAlgorithm SHA256
& "$PSScriptRoot/verify_first_install_windows.ps1" $Updater $CertificateSha256 (Get-FileHash -LiteralPath $Updater).Hash
