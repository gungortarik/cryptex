param([ValidateSet('image','wrong-receipt')][string]$Action, [string]$Path, [string]$OutputPath)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'Disposable Windows runner required'
}
$cert = Get-Item -LiteralPath "Cert:\CurrentUser\My\$env:CX_UPDATE_CI_SIGNER"
if ($Action -eq 'image') {
    $signature = Set-AuthenticodeSignature -FilePath $Path -Certificate $cert -HashAlgorithm SHA256
    if ($signature.Status -ne 'Valid') { throw "Fixture signing failed ($($signature.Status))" }
    return
}
$foreign = $null
Add-Type -AssemblyName System.Security.Cryptography.Pkcs
try {
    if ($Action -eq 'wrong-receipt') {
        $foreign = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=Cryptex unrelated CI $([guid]::NewGuid())" `
            -KeyExportPolicy NonExportable -NotAfter (Get-Date).AddDays(1) -CertStoreLocation Cert:\CurrentUser\My
        $cert = $foreign
    }
    $cms = [Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new([IO.File]::ReadAllBytes($Path)))
    $signer = [Security.Cryptography.Pkcs.CmsSigner]::new($cert)
    $signer.DigestAlgorithm = [Security.Cryptography.Oid]::new('2.16.840.1.101.3.4.2.1')
    $cms.ComputeSignature($signer)
    [IO.File]::WriteAllBytes($OutputPath, $cms.Encode())
} finally {
    if ($foreign) { Remove-Item -LiteralPath "Cert:\CurrentUser\My\$($foreign.Thumbprint)" -DeleteKey }
}
