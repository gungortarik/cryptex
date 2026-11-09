$ErrorActionPreference='Stop'
$root=Join-Path $env:TEMP ('cx-native-runner-check-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
Add-Type -AssemblyName System.IO.Compression.FileSystem
try {
    foreach($name in @('NATIVE-AKTAR.ps1','start-native-controller-relay.ps1')) {
        $tokens=$null; $errors=$null
        $ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $name),[ref]$tokens,[ref]$errors)
        if($errors.Count){throw "parse_failed_$name"}
        if($name -eq 'NATIVE-AKTAR.ps1') {
            foreach($f in $ast.FindAll({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst]},$false)) {
                if($f.Name -in @('Test-NativeSummary','Assert-SourceArchive')){Invoke-Expression $f.Extent.Text}
            }
        }
    }
    if(-not (Test-NativeSummary 0 @('test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 585 filtered out;'))){throw 'valid_summary_rejected'}
    foreach($case in @(
        @{code=1;line='test result: ok. 1 passed; 0 failed; 0 ignored;'},
        @{code=0;line='test result: ok. 0 passed; 0 failed; 0 ignored;'},
        @{code=0;line='test result: ok. 1 passed; 0 failed; 1 ignored;'}
    )) {if(Test-NativeSummary $case.code @($case.line)){throw 'invalid_summary_accepted'}}
    $session='0123456789abcdef0123456789abcdef'
    $peer='XAAPCAD-GA4SF4J-BNQL3FS-T456WKV-FGLXUKO-CDWHVDW-EYWNRIG-SM5GUAZ'
    $receiver='U2SYIGU-YWZX757-2WPZGCR-DHD7MLH-ZKV7HLJ-EYO6CXE-K3DU3DU-SDFXYAE'
    @{schema=1;session=$session;peer_id=$peer;address='tcp://192.168.177.1:23456'} | ConvertTo-Json | Set-Content (Join-Path $root 'relay.json')
    @{native_id=$peer} | ConvertTo-Json | Set-Content (Join-Path $root 'sender.json')
    $relative=('a'*72)+'/00000000000000000000.seg'
    $public=Join-Path $root 'public-request'
    $file=Join-Path $public ('payload/'+$relative)
    New-Item -ItemType Directory -Path ([IO.Path]::GetDirectoryName($file)) -Force | Out-Null
    [IO.File]::WriteAllBytes($file,[byte[]]@(1,2,3))
    $manifest=@{schema=1;session=$session;relay_id=$peer;receiver_id=$receiver;folder_id=('cx-native-'+$session);
        segments=@(@{relative_path=$relative;bytes=3;sha256=(Get-FileHash -LiteralPath $file).Hash.ToLowerInvariant()})}
    $manifest | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $public 'request.json')
    $requestZip=Join-Path $root 'valid.zip'
    Compress-Archive -Path (Join-Path $public '*') -DestinationPath $requestZip
    & (Join-Path $PSScriptRoot 'start-native-controller-relay.ps1') -SessionRoot $root -RequestZip $requestZip -ValidateOnly
    $r=Get-Content (Join-Path $root 'relay-result.json') -Raw | ConvertFrom-Json
    if($r.outcome -ne 'request_validated_only' -or $r.native_launched){throw 'valid_request_failed'}
    foreach($bad in @('../escape.txt','payload/../escape.seg','C:/escape.txt','request.json')) {
        $zipPath=Join-Path $root ([guid]::NewGuid().ToString('N')+'.zip')
        Copy-Item -LiteralPath $requestZip -Destination $zipPath
        $archive=[IO.Compression.ZipFile]::Open($zipPath,[IO.Compression.ZipArchiveMode]::Update)
        try {[void]$archive.CreateEntry($bad)} finally {$archive.Dispose()}
        $refused=$false
        try {& (Join-Path $PSScriptRoot 'start-native-controller-relay.ps1') -SessionRoot $root -RequestZip $zipPath -ValidateOnly} catch {$refused=$true}
        if(-not $refused){throw 'unsafe_request_accepted'}
        # The common source extraction guard also refuses traversal/duplicates.
        $refused=$false
        try {Assert-SourceArchive $zipPath} catch {$refused=$true}
        if(-not $refused){throw 'unsafe_source_accepted'}
    }
    # Metadata must not turn mismatched bytes into success.
    $manifest.segments[0].sha256='0'*64
    $manifest | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $public 'request.json')
    $badHash=Join-Path $root 'bad-hash.zip'
    Compress-Archive -Path (Join-Path $public '*') -DestinationPath $badHash
    $refused=$false
    try {& (Join-Path $PSScriptRoot 'start-native-controller-relay.ps1') -SessionRoot $root -RequestZip $badHash -ValidateOnly} catch {$refused=$true}
    if(-not $refused){throw 'bad_digest_accepted'}
    foreach($address in @('default','tcp://0.0.0.0:23456','tcp://8.8.8.8:23456','tcp://192.168.177.1:0')) {
        @{schema=1;session=$session;peer_id=$peer;address=$address} | ConvertTo-Json | Set-Content (Join-Path $root 'relay.json')
        $refused=$false
        try {& (Join-Path $PSScriptRoot 'start-native-controller-relay.ps1') -SessionRoot $root -RequestZip $requestZip -ValidateOnly} catch {$refused=$true}
        if(-not $refused){throw 'unsafe_relay_address_accepted'}
    }
    Write-Output 'native_controller_runner_checks=passed'
} finally {
    $full=[IO.Path]::GetFullPath($root)
    $parent=[IO.Path]::GetFullPath($env:TEMP).TrimEnd('\')+'\'
    if(-not $full.StartsWith($parent,[StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetFileName($full) -notlike 'cx-native-runner-check-*'){throw 'unsafe_cleanup_target'}
    Remove-Item -LiteralPath $full -Recurse -Force
}
