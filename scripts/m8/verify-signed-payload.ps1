function Invoke-ProbeVerifier($Daemon, $Segment, $Descriptor) {
    $start=New-Object Diagnostics.ProcessStartInfo
    $start.FileName=$Daemon
    $start.Arguments='pilot-fixture verify-signed-transfer-v1 --segment "' + $Segment + '" --descriptor "' + $Descriptor + '" --confirm-isolated-pilot'
    $start.UseShellExecute=$false
    $start.CreateNoWindow=$true
    $start.RedirectStandardOutput=$true
    $start.RedirectStandardError=$true
    $process=[Diagnostics.Process]::Start($start)
    $stdout=$process.StandardOutput.ReadToEndAsync()
    $stderr=$process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit(30000)) { $process.Kill(); throw 'signature_verifier_timeout' }
    return [pscustomobject]@{exit=$process.ExitCode; stdout=$stdout.Result; stderr=$stderr.Result}
}
function Test-SignedPayload($Settings, $Target, $StateRoot) {
    $daemon=Join-Path $PSScriptRoot 'cryptexd.exe'
    $descriptor=Join-Path $PSScriptRoot 'signed-probe.json'
    if ((Get-FileHash -LiteralPath $daemon).Hash -ne $Settings.daemon_sha256 -or
        (Get-FileHash -LiteralPath $descriptor).Hash -ne $Settings.descriptor_sha256) { throw 'verifier_package_hash_mismatch' }
    $valid=Invoke-ProbeVerifier $daemon $Target $descriptor
    if ($valid.exit -ne 0) { throw 'valid_signature_rejected' }
    $result=$valid.stdout | ConvertFrom-Json
    if ($result.scope -ne 'synthetic_signature_only_not_trust' -or $result.outcome -ne 'signature_verified' -or
        $result.event_count -ne 1 -or $result.trust_activated -ne $false -or $result.published -ne $false) { throw 'unexpected_verification_report' }
    $bytes=[IO.File]::ReadAllBytes($Target)
    # Skip the binary frame header before calculating a UTF-8 body offset.
    $text=[Text.Encoding]::UTF8.GetString($bytes,10,$bytes.Length-10)
    $index=$text.IndexOf('Synthetic signed transfer probe.',[StringComparison]::Ordinal)
    if ($index -lt 0) { throw 'negative_fixture_missing' }
    $offset=10+[Text.Encoding]::UTF8.GetByteCount($text.Substring(0,$index))
    $bytes[$offset]=$bytes[$offset] -bxor 1
    $tampered=Join-Path $StateRoot ('tampered-' + $Settings.probe_id + '.seg')
    [IO.File]::WriteAllBytes($tampered,$bytes)
    $invalid=Invoke-ProbeVerifier $daemon $tampered $descriptor
    if ($invalid.exit -eq 0 -or $invalid.stderr -notmatch 'probe_integrity_failed') { throw 'tampered_signature_not_rejected' }
    if ((Get-FileHash -LiteralPath $Target).Hash -ne $Settings.sha256) { throw 'received_source_changed' }
    return [pscustomobject]@{signature_verified=$true; tampered_copy_rejected=$true; trust_activated=$false; published=$false}
}
