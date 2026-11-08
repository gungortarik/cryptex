. (Join-Path $PSScriptRoot 'trusted-import-bundle.ps1')
. (Join-Path $PSScriptRoot 'verify-signed-payload.ps1')
function Invoke-IncomingProbe($Daemon,$Root,[string[]]$Paths) {
    if ($Paths.Count -lt 1 -or $Paths.Count -gt 16) { throw 'validation_path_count' }
    foreach ($path in $Paths) { if ($path -cnotmatch '^[0-9a-f]{72}/[0-9]{20}\.seg$') { throw 'validation_path_invalid' } }
    $start=New-Object Diagnostics.ProcessStartInfo
    $start.FileName=$Daemon
    $start.Arguments='replication validate-incoming --runtime-root "' + $Root + '" --environment dev ' + (($Paths | ForEach-Object {'"'+$_+'"'}) -join ' ')
    $start.UseShellExecute=$false
    $start.CreateNoWindow=$true
    $start.RedirectStandardOutput=$true
    $start.RedirectStandardError=$true
    $process=[Diagnostics.Process]::Start($start)
    $stdout=$process.StandardOutput.ReadToEndAsync()
    $stderr=$process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit(30000)) { $process.Kill(); throw 'incoming_validation_timeout' }
    return [pscustomobject]@{exit=$process.ExitCode;stdout=$stdout.Result;stderr=$stderr.Result}
}
function Get-CanonicalProbeHashes($Root) {
    $events=Join-Path $Root 'canonical\events'
    if (-not (Test-TrustedProbePlainPath $events)) { throw 'canonical_path_unsafe' }
    $results=@()
    foreach ($device in @(Get-ChildItem -LiteralPath $events -Directory -Force)) {
        if (-not (Test-TrustedProbePlainPath $device.FullName)) { throw 'canonical_path_unsafe' }
        foreach ($file in @(Get-ChildItem -LiteralPath $device.FullName -File -Force)) {
            if ($file.Extension -in @('.open','.seg')) {
                if (-not (Test-TrustedProbePlainPath $file.FullName)) { throw 'canonical_path_unsafe' }
                $results += [pscustomobject]@{path=($device.Name+'/'+$file.Name);hash=(Get-FileHash -LiteralPath $file.FullName).Hash}
            }
        }
    }
    return @($results | Sort-Object path)
}
function Copy-ProbeIncoming($Receiver,$Source,$Segment) {
    $target=Join-Path $Receiver ('transport\incoming\'+$Segment.relative_path.Replace('/','\'))
    $parent=Split-Path $target -Parent
    $existing=$parent
    while (-not (Test-Path -LiteralPath $existing)) { $existing=Split-Path $existing -Parent }
    if (-not (Test-TrustedProbePlainPath $existing)) { throw 'incoming_path_unsafe' }
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
    if (Test-Path -LiteralPath $target) {
        if (-not (Test-TrustedProbePlainPath $target) -or (Get-FileHash -LiteralPath $target).Hash -cne $Segment.sha256) { throw 'incoming_destination_conflict' }
    } else {
        $inputBytes=[IO.File]::ReadAllBytes($Source)
        if ((Get-ProbeBytesHash $inputBytes) -cne $Segment.sha256) { throw 'incoming_source_hash_mismatch' }
        $file=[IO.File]::Open($target,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
        try { $file.Write($inputBytes,0,$inputBytes.Length) } finally { $file.Dispose() }
    }
    return $target
}
function Read-AcceptedImportResult($Result,[int]$SegmentCount) {
    if ($Result.exit -ne 0) { throw 'trusted_import_rejected' }
    $report=$Result.stdout | ConvertFrom-Json
    if ($report.outcome -ne 'validated_only' -or $report.published -ne $false -or $report.segment_count -ne $SegmentCount -or $report.event_count -lt 1) { throw 'unexpected_import_report' }
    return $report
}
function Test-TrustedImportPayload($Settings,$Target,$StateRoot) {
    $daemon=Join-Path $PSScriptRoot 'cryptexd.exe'
    if ((Get-FileHash -LiteralPath $daemon).Hash -cne $Settings.daemon_sha256) { throw 'daemon_hash_mismatch' }
    $work=Join-Path $StateRoot ('trusted-attempt-'+[guid]::NewGuid().ToString('N').Substring(0,8))
    $bundle=Read-TrustedImportBundle $Target $Settings.trusted_probe_id $work
    $manifest=$bundle.manifest
    if ($bundle.manifest_sha256 -cne $Settings.manifest_sha256 -or $manifest.brain_id -cne $Settings.brain_id -or
        $manifest.source_device -cne $Settings.source_device -or $manifest.receiver_device -cne $Settings.receiver_device) { throw 'trusted_bundle_binding_mismatch' }
    $receiver=Join-Path $env:TEMP ($Settings.receiver_profile+'\desktop')
    if (-not (Test-TrustedProbePlainPath $receiver)) { throw 'receiver_path_unsafe' }
    if ((Get-Content -LiteralPath (Join-Path $receiver 'pilot-runtime.marker') -Raw).Trim() -ne 'm0-native-desktop-runtime-v1') { throw 'receiver_marker_mismatch' }
    $identity=Get-Content -LiteralPath (Join-Path $receiver 'identity\local_device.json') -Raw | ConvertFrom-Json
    if ($identity.device_id -cne $manifest.receiver_device -or $identity.brain_id -cne $manifest.brain_id) { throw 'receiver_identity_mismatch' }
    $before=@(Get-CanonicalProbeHashes $receiver)
    if ($before.Count -lt 1) { throw 'canonical_baseline_missing' }
    $journal=Join-Path $receiver 'bootstrap\pending.json'
    $journalBefore=(Get-FileHash -LiteralPath $journal).Hash
    $paths=@($manifest.segments | ForEach-Object relative_path)
    foreach ($segment in $manifest.segments) {
        Copy-ProbeIncoming $receiver (Join-Path $work ('trusted\'+$segment.relative_path.Replace('/','\'))) $segment | Out-Null
    }
    Write-Host 'Guvenilir kayit daemonun tam ice aktarma kurallariyla dogrulaniyor...'
    $positive=Read-AcceptedImportResult (Invoke-IncomingProbe $daemon $receiver $paths) $paths.Count
    $negative=$manifest.unapproved_segment
    $negativeFile=Join-Path $work ('unapproved\'+$negative.relative_path.Replace('/','\'))
    $signature=Invoke-ProbeVerifier $daemon $negativeFile (Join-Path $work 'unapproved-public.json')
    if ($signature.exit -ne 0) { throw 'unapproved_fixture_signature_invalid' }
    $negativeSignature=$signature.stdout | ConvertFrom-Json
    if ($negativeSignature.outcome -ne 'signature_verified' -or $negativeSignature.trust_activated -ne $false -or $negativeSignature.published -ne $false) { throw 'unapproved_fixture_scope_invalid' }
    Copy-ProbeIncoming $receiver $negativeFile $negative | Out-Null
    $unknown=Invoke-IncomingProbe $daemon $receiver @($negative.relative_path)
    if ($unknown.exit -eq 0 -or $unknown.stderr -notmatch 'retrieval safety gate is held: Some\(CanonicalVerificationUnavailable\)') { throw 'unapproved_rejection_unexpected' }
    $unknownCanonical=Join-Path $receiver ('canonical\events\'+(Get-ProbeDeviceStem $manifest.unapproved_device))
    if (Test-Path -LiteralPath $unknownCanonical) { throw 'unapproved_canonical_directory_created' }
    # A separate fresh incoming inode is used for corruption. The original
    # incoming file is renamed aside, never modified (including hardlink aliases).
    $mutated=$false
    foreach ($segment in $manifest.segments) {
        $bytes=[IO.File]::ReadAllBytes((Join-Path $work ('trusted\'+$segment.relative_path.Replace('/','\'))))
        $offset=[Text.Encoding]::GetEncoding(28591).GetString($bytes).IndexOf('Synthetic trusted import probe.',[StringComparison]::Ordinal)
        if ($offset -lt 0) { continue }
        $bytes[$offset]=$bytes[$offset] -bxor 1
        $badHash=Get-ProbeBytesHash $bytes
        $incoming=Join-Path $receiver ('transport\incoming\'+$segment.relative_path.Replace('/','\'))
        $backup=$incoming+'.probe-backup-'+[guid]::NewGuid().ToString('N')
        Move-Item -LiteralPath $incoming -Destination $backup
        try {
            $file=[IO.File]::Open($incoming,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
            try { $file.Write($bytes,0,$bytes.Length) } finally { $file.Dispose() }
            $bad=Invoke-IncomingProbe $daemon $receiver $paths
            if ($bad.exit -eq 0 -or $bad.stderr -notmatch '\bintegrity_failure\b') { throw 'tampered_rejection_unexpected' }
            $mutated=$true
        } finally {
            if (Test-Path -LiteralPath $incoming) {
                if (-not (Test-TrustedProbePlainPath $incoming) -or (Get-FileHash -LiteralPath $incoming).Hash -cne $badHash) { throw 'mutant_restore_conflict' }
                Remove-Item -LiteralPath $incoming
            }
            Move-Item -LiteralPath $backup -Destination $incoming
        }
        break
    }
    if (-not $mutated) { throw 'tamper_test_note_missing' }
    $repeat=Read-AcceptedImportResult (Invoke-IncomingProbe $daemon $receiver $paths) $paths.Count
    $after=@(Get-CanonicalProbeHashes $receiver)
    if (@(Compare-Object $before $after -Property path,hash).Count -ne 0) { throw 'canonical_events_changed' }
    if ((Get-FileHash -LiteralPath $journal).Hash -cne $journalBefore) { throw 'approval_journal_changed' }
    if ((Get-FileHash -LiteralPath $Target).Hash -cne $Settings.sha256) { throw 'received_bundle_changed' }
    return [pscustomobject]@{trusted_accepted=$true;repeat_accepted=$true;unapproved_signature_verified=$true;unapproved_rejected=$true;
        tampered_rejected=$true;canonical_event_hashes_unchanged=$true;approval_journal_unchanged=$true;
        unapproved_canonical_directory_absent=$true;published=$false;positive=$positive;repeat=$repeat;trusted_probe_id=$manifest.probe_id}
}
