param([switch]$NoOpen)
$ErrorActionPreference='Stop'
$work=$null
function Save-ProbeJson($Path,$Value) {
    [IO.File]::WriteAllText($Path,($Value | ConvertTo-Json -Depth 16),(New-Object Text.UTF8Encoding($false)))
}
function Export-ProbeSegment($Root,$Segment,$Destination) {
    if ($Segment.relative_path -notmatch '^[0-9a-f]{72}/[0-9]{20}\.seg$' -or $Segment.sha256 -notmatch '^[A-F0-9]{64}$' -or $Segment.bytes -lt 1 -or $Segment.bytes -gt 8MB) { throw 'invalid_segment_manifest' }
    $source=Join-Path $Root ('canonical\events\' + $Segment.relative_path.Replace('/','\'))
    if ((Get-Item -LiteralPath $source).Length -ne $Segment.bytes -or (Get-FileHash -LiteralPath $source).Hash -ne $Segment.sha256) { throw 'source_changed' }
    $target=Join-Path $Destination $Segment.relative_path.Replace('/','\')
    New-Item -ItemType Directory -Path (Split-Path $target -Parent) -Force | Out-Null
    Copy-Item -LiteralPath $source -Destination $target
    if ((Get-FileHash -LiteralPath $target).Hash -ne $Segment.sha256) { throw 'export_hash_mismatch' }
}
try {
    $work=Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\tp\' + [guid]::NewGuid().ToString('N').Substring(0,16))
    New-Item -ItemType Directory -Path $work | Out-Null
    $sid=[Security.Principal.WindowsIdentity]::GetCurrent().User
    $acl=New-Object Security.AccessControl.DirectorySecurity
    $acl.SetOwner($sid)
    $acl.SetAccessRuleProtection($true,$false)
    $acl.AddAccessRule((New-Object Security.AccessControl.FileSystemAccessRule($sid,'FullControl','ContainerInherit,ObjectInherit','None','Allow')))
    Set-Acl -LiteralPath $work -AclObject $acl
    $configPath=Join-Path $PSScriptRoot 'trusted-probe-settings.json'
    if ((Get-Item -LiteralPath $configPath).Length -gt 4096) { throw 'settings_limit' }
    $config=Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
    if ($config.schema -ne 1 -or $config.source_profile -notmatch '^cx-m8-[0-9a-f]{32}$' -or $config.receiver_profile -notmatch '^cx-m8-[0-9a-f]{32}$' -or
        $config.source_profile -eq $config.receiver_profile -or $config.probe_id -notmatch '^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$') { throw 'invalid_settings' }
    $daemon=Join-Path $PSScriptRoot 'cryptexd.exe'
    if ((Get-FileHash -LiteralPath $daemon).Hash -ne $config.daemon_sha256) { throw 'daemon_hash_mismatch' }
    $source=Join-Path $env:TEMP ($config.source_profile + '\desktop')
    $receiver=Join-Path $env:TEMP ($config.receiver_profile + '\desktop')
    foreach ($pair in @(@{root=$source;device=$config.source_device},@{root=$receiver;device=$config.receiver_device})) {
        $identity=Get-Content -LiteralPath (Join-Path $pair.root 'identity\local_device.json') -Raw | ConvertFrom-Json
        if ($identity.device_id -ne $pair.device -or $identity.brain_id -ne $config.brain_id) { throw 'original_profile_mismatch' }
    }
    $negative=Join-Path $work 'n'
    New-Item -ItemType Directory -Path $negative | Out-Null
    [IO.File]::WriteAllText((Join-Path $negative 'pilot-runtime.marker'),"m0-native-desktop-runtime-v1`n",(New-Object Text.UTF8Encoding($false)))
    Write-Host 'Onceden onaylanmis test cihazlari ve imzali gecmis kontrol ediliyor...'
    $start=New-Object Diagnostics.ProcessStartInfo
    $start.FileName=$daemon
    $start.Arguments='pilot-fixture trusted-import-prepare-v1 --source-root "' + $source + '" --receiver-root "' + $receiver + '" --negative-root "' + $negative + '" --probe-id ' + $config.probe_id + ' --confirm-isolated-pilot'
    $start.UseShellExecute=$false
    $start.CreateNoWindow=$true
    $start.RedirectStandardOutput=$true
    $start.RedirectStandardError=$true
    $process=[Diagnostics.Process]::Start($start)
    $stdout=$process.StandardOutput.ReadToEndAsync()
    $stderr=$process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit(120000)) { $process.Kill(); throw 'prepare_timeout' }
    if ($process.ExitCode -ne 0) {
        if ($stderr.Result -match 'probe_[a-z_]+') { throw $Matches[0] }
        throw 'daemon_preparation_failed'
    }
    $report=$stdout.Result | ConvertFrom-Json
    if ($report.scope -ne 'trusted_import_validation_only' -or $report.probe_id -ne $config.probe_id -or $report.brain_id -ne $config.brain_id -or
        $report.source_device -ne $config.source_device -or $report.receiver_device -ne $config.receiver_device -or
        $report.reciprocal_approval_verified -ne $true -or $report.published -ne $false -or @($report.segments).Count -lt 1 -or @($report.segments).Count -gt 16) { throw 'preparation_binding_mismatch' }
    $export=Join-Path $work 'export'
    New-Item -ItemType Directory -Path $export | Out-Null
    Save-ProbeJson (Join-Path $export 'trusted-probe.json') $report
    foreach ($segment in $report.segments) { Export-ProbeSegment $source $segment (Join-Path $export 'trusted') }
    Export-ProbeSegment $negative $report.unapproved_segment (Join-Path $export 'unapproved')
    $negativePublic=Join-Path $negative 'signed-probe.json'
    if ((Get-FileHash -LiteralPath $negativePublic).Hash -ne $report.unapproved_descriptor_sha256) { throw 'negative_public_descriptor_mismatch' }
    Copy-Item -LiteralPath $negativePublic -Destination (Join-Path $export 'unapproved-public.json')
    $output=Join-Path $work 'GUVEN-HAZIRLIK.zip'
    Compress-Archive -Path (Join-Path $export '*') -DestinationPath $output
    Write-Host 'Hazir. GUVEN-HAZIRLIK.zip dosyasini sohbete ekleyin.'
    Write-Host 'Bu dosya yalnizca sentetik imzali test kayitlarini icerir. Ozel anahtar yoktur.'
    if (-not $NoOpen) { Start-Process explorer.exe -ArgumentList ('/select,"{0}"' -f $output) }
    Write-Host $output
} catch {
    $code=$_.Exception.Message
    if ($code -notmatch '^[a-z_]{1,64}$') { $code='preparation_failed' }
    Write-Host ('Hazirlik durdu: ' + $code)
    if ($work) {
        Save-ProbeJson (Join-Path $work 'error.json') @{schema=1;scope='trusted_import_preparation';error_code=$code}
        $output=Join-Path $work 'GUVEN-HATA.zip'
        Compress-Archive -LiteralPath (Join-Path $work 'error.json') -DestinationPath $output -Force
        if (-not $NoOpen) { Start-Process explorer.exe -ArgumentList ('/select,"{0}"' -f $output) }
        Write-Host $output
    }
    exit 1
}
