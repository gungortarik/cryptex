param(
    [Parameter(Mandatory)][string]$Installer,
    [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$SignerSha256,
    [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ReleaseSha256
)
$ErrorActionPreference = 'Stop'
$file = Get-Item -LiteralPath $Installer
if ($file -isnot [System.IO.FileInfo] -or $file.Extension -ine '.exe' -or
    ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Expected a local installer executable' }

# Retain a read handle across both checks. Take expected hashes from the signing machine.
$stream = [IO.File]::Open($file.FullName, 'Open', 'Read', 'Read')
$sha = [Security.Cryptography.SHA256]::Create()
try {
    $release = [BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-', '')
    if ($release -ine $ReleaseSha256) { throw 'Different Cryptex release' }
    $signature = Get-AuthenticodeSignature -LiteralPath $file.FullName
    if ($signature.Status -ne 'Valid' -or $signature.SignatureType -ne 'Authenticode') {
        throw "Invalid installer signature ($($signature.Status))"
    }
    $signer = [BitConverter]::ToString($sha.ComputeHash($signature.SignerCertificate.RawData)).Replace('-', '')
    if ($signer -ine $SignerSha256) { throw 'Unexpected Cryptex signer' }
} finally {
    $sha.Dispose()
    $stream.Dispose()
}
Write-Output 'Cryptex installer and owner release verified.'
