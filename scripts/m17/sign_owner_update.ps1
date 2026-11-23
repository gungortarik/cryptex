param(
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{64}$')][string]$CertificateSha256,
    [Parameter(Mandatory)][string]$Statement,
    [Parameter(Mandatory)][string]$Receipt
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Security.Cryptography.Pkcs
$matchesPin = @(Get-ChildItem Cert:\CurrentUser\My | Where-Object {
    $_.HasPrivateKey -and $_.GetCertHashString('SHA256') -eq $CertificateSha256
})
if ($matchesPin.Count -ne 1) { throw 'Expected one signing identity with the selected certificate SHA-256' }
$cms = [Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new([IO.File]::ReadAllBytes($Statement)))
$signer = [Security.Cryptography.Pkcs.CmsSigner]::new($matchesPin[0])
$signer.DigestAlgorithm = [Security.Cryptography.Oid]::new('2.16.840.1.101.3.4.2.1')
$signer.IncludeOption = [Security.Cryptography.X509Certificates.X509IncludeOption]::EndCertOnly
$cms.ComputeSignature($signer)
$cms.CheckSignature($true)
$bytes = $cms.Encode()
$stream = [IO.File]::Open($Receipt, 'CreateNew', 'Write', 'None')
try { $stream.Write($bytes, 0, $bytes.Length) } finally { $stream.Dispose() }
