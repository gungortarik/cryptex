function Test-TrustedProbePlainPath([string]$Path) {
    $item=Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    while ($null -ne $item) {
        if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { return $false }
        if ($item -is [IO.DirectoryInfo]) { $item=$item.Parent } else { $item=$item.Directory }
    }
    return $true
}
function Get-ProbeBytesHash([byte[]]$Bytes) {
    $hash=[Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($hash.ComputeHash($Bytes))).Replace('-','') } finally { $hash.Dispose() }
}
function Read-ProbeEntry($Entry,[int]$Limit) {
    if ($Entry.Length -gt $Limit) { throw 'bundle_entry_limit' }
    $stream=$Entry.Open()
    $memory=New-Object IO.MemoryStream
    try {
        $buffer=New-Object byte[] 4096
        while (($count=$stream.Read($buffer,0,$buffer.Length)) -gt 0) {
            if ($memory.Length + $count -gt $Limit) { throw 'bundle_entry_limit' }
            $memory.Write($buffer,0,$count)
        }
        return [pscustomobject]@{bytes=$memory.ToArray()}
    } finally { $stream.Dispose();$memory.Dispose() }
}
function Assert-ProbeProperties($Value,[string[]]$Names) {
    if (@($Value.PSObject.Properties).Count -ne $Names.Count) { throw 'bundle_schema_mismatch' }
    foreach ($property in $Value.PSObject.Properties) { if ($Names -cnotcontains $property.Name) { throw 'bundle_schema_mismatch' } }
}
function Get-ProbeDeviceStem([string]$Device) {
    if ($Device -cnotmatch '^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$') { throw 'bundle_invalid_device' }
    return ([BitConverter]::ToString([Text.Encoding]::UTF8.GetBytes($Device))).Replace('-','').ToLowerInvariant()
}
function Read-TrustedImportBundle([string]$Path,[string]$ExpectedProbe,[string]$Destination) {
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip=[IO.Compression.ZipFile]::OpenRead($Path)
    try {
        if ($zip.Entries.Count -gt 20 -or ($zip.Entries | Measure-Object Length -Sum).Sum -gt 128MB) { throw 'bundle_size_limit' }
        $entries=@{}
        foreach ($entry in $zip.Entries) {
            $name=$entry.FullName.Replace('\','/')
            if ($entries.ContainsKey($name)) { throw 'bundle_duplicate_entry' }
            $entries[$name]=$entry
        }
        if (-not $entries.ContainsKey('trusted-probe.json') -or -not $entries.ContainsKey('unapproved-public.json')) { throw 'bundle_metadata_missing' }
        $manifestBytes=(Read-ProbeEntry $entries['trusted-probe.json'] 16384).bytes
        $manifest=[Text.Encoding]::UTF8.GetString($manifestBytes) | ConvertFrom-Json
        Assert-ProbeProperties $manifest @('schema','scope','probe_id','brain_id','source_device','receiver_device','reciprocal_approval_verified','segments','unapproved_segment','unapproved_device','unapproved_descriptor_sha256','published')
        if ($manifest.schema -ne 1 -or $manifest.scope -ne 'trusted_import_validation_only' -or $manifest.probe_id -cne $ExpectedProbe -or
            $manifest.reciprocal_approval_verified -isnot [bool] -or -not $manifest.reciprocal_approval_verified -or
            $manifest.published -isnot [bool] -or $manifest.published -or @($manifest.segments).Count -lt 1 -or @($manifest.segments).Count -gt 16) { throw 'bundle_binding_mismatch' }
        $sourceStem=Get-ProbeDeviceStem $manifest.source_device
        $negativeStem=Get-ProbeDeviceStem $manifest.unapproved_device
        Get-ProbeDeviceStem $manifest.receiver_device | Out-Null
        Get-ProbeDeviceStem $manifest.brain_id | Out-Null
        if ($manifest.unapproved_device -in @($manifest.source_device,$manifest.receiver_device) -or $manifest.source_device -eq $manifest.receiver_device) { throw 'bundle_device_collision' }
        $expected=@{'trusted-probe.json'=$null;'unapproved-public.json'=$null}
        foreach ($segment in $manifest.segments) {
            Assert-ProbeProperties $segment @('relative_path','bytes','sha256')
            if ($segment.relative_path -cnotmatch ('^'+$sourceStem+'/[0-9]{20}\.seg$') -or $segment.bytes -lt 1 -or $segment.bytes -gt 8MB -or $segment.sha256 -cnotmatch '^[A-F0-9]{64}$') { throw 'bundle_segment_invalid' }
            $name='trusted/'+$segment.relative_path
            if ($expected.ContainsKey($name)) { throw 'bundle_duplicate_segment' }
            $expected[$name]=$segment
        }
        $negative=$manifest.unapproved_segment
        Assert-ProbeProperties $negative @('relative_path','bytes','sha256')
        if ($negative.relative_path -cne ($negativeStem+'/00000000000000000000.seg') -or $negative.bytes -lt 1 -or $negative.bytes -gt 8MB -or $negative.sha256 -cnotmatch '^[A-F0-9]{64}$') { throw 'bundle_negative_invalid' }
        $expected['unapproved/'+$negative.relative_path]=$negative
        if ($entries.Count -ne $expected.Count) { throw 'bundle_unexpected_entries' }
        foreach ($name in $entries.Keys) { if (-not $expected.ContainsKey($name)) { throw 'bundle_unexpected_entries' } }
        $negativeBytes=(Read-ProbeEntry $entries['unapproved-public.json'] 4096).bytes
        if ((Get-ProbeBytesHash $negativeBytes) -cne $manifest.unapproved_descriptor_sha256) { throw 'bundle_negative_descriptor_hash' }
        $public=[Text.Encoding]::UTF8.GetString($negativeBytes) | ConvertFrom-Json
        Assert-ProbeProperties $public @('schema','scope','brain_id','device_id','event_id','public_key')
        if ($public.schema -ne 1 -or $public.scope -ne 'synthetic_signature_only_not_trust' -or $public.brain_id -cne $manifest.brain_id -or
            $public.device_id -cne $manifest.unapproved_device -or $public.public_key -cnotmatch '^[0-9a-f]{64}$') { throw 'bundle_negative_descriptor_binding' }
        if (Test-Path -LiteralPath $Destination) { throw 'bundle_fresh_destination_required' }
        if (-not (Test-TrustedProbePlainPath (Split-Path $Destination -Parent))) { throw 'bundle_unsafe_destination' }
        New-Item -ItemType Directory -Path $Destination | Out-Null
        foreach ($name in $expected.Keys) {
            $entry=$entries[$name]
            $bytes=(Read-ProbeEntry $entry $(if ($name -eq 'trusted-probe.json') {16384} elseif ($name -eq 'unapproved-public.json') {4096} else {8MB})).bytes
            $record=$expected[$name]
            if ($record -and ($bytes.Length -ne $record.bytes -or (Get-ProbeBytesHash $bytes) -cne $record.sha256)) { throw 'bundle_segment_hash_mismatch' }
            $target=Join-Path $Destination $name.Replace('/','\')
            New-Item -ItemType Directory -Path (Split-Path $target -Parent) -Force | Out-Null
            [IO.File]::WriteAllBytes($target,$bytes)
        }
        return [pscustomobject]@{manifest=$manifest;manifest_sha256=(Get-ProbeBytesHash $manifestBytes);root=$Destination}
    } finally { $zip.Dispose() }
}
