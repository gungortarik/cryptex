$ErrorActionPreference='Stop'
$work=Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\win-test\'+[guid]::NewGuid().ToString('N').Substring(0,12))
New-Item -ItemType Directory -Path $work | Out-Null
$utf8=New-Object Text.UTF8Encoding($false)
$report=[ordered]@{
    schema=1; scope='isolated_windows_platform_tests'; probe_id='m8-windows-platform-20261108-v1';
    collected_utc=[DateTime]::UtcNow.ToString('o'); os=[Environment]::OSVersion.Version.ToString();
    publication_enabled=$false; production_profiles_accessed=$false; security_settings_changed=$false;
    outcome='not_completed'; checks=@()
}
$logs=@()
$exitCode=1

function Invoke-TestProcess([string]$Executable,[string]$Arguments,[string]$Log,[string]$WorkingDirectory) {
    $start=New-Object Diagnostics.ProcessStartInfo
    $start.FileName=$Executable
    $start.Arguments=$Arguments
    $start.WorkingDirectory=$WorkingDirectory
    $start.UseShellExecute=$false
    $start.CreateNoWindow=$true
    $start.RedirectStandardOutput=$true
    $start.RedirectStandardError=$true
    $start.EnvironmentVariables['CARGO_TARGET_DIR']=Join-Path $work 'target'
    $process=[Diagnostics.Process]::Start($start)
    try {
        $stdout=$process.StandardOutput.ReadToEndAsync()
        $stderr=$process.StandardError.ReadToEndAsync()
        $process.WaitForExit()
        [IO.File]::WriteAllText($Log, $stdout.Result+"`n"+$stderr.Result, $utf8)
        return $process.ExitCode
    } finally { $process.Dispose() }
}

try {
    $commit=(Get-Content -LiteralPath (Join-Path $PSScriptRoot 'SOURCE-COMMIT.txt') -Raw).Trim()
    $expected=(Get-Content -LiteralPath (Join-Path $PSScriptRoot 'SOURCE-SHA256.txt') -Raw).Trim()
    if($commit -cnotmatch '^[0-9a-f]{40}$' -or $expected -cnotmatch '^[0-9a-f]{64}$'){throw 'invalid_source_binding'}
    $sourceZip=Join-Path $PSScriptRoot 'source.zip'
    $actual=(Get-FileHash -LiteralPath $sourceZip -Algorithm SHA256).Hash.ToLowerInvariant()
    if($actual -cne $expected){throw 'source_hash_mismatch'}
    $report.source_commit=$commit
    $report.source_sha256=$actual
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive=[IO.Compression.ZipFile]::OpenRead($sourceZip)
    try {
        $total=0L; $names=@{}
        if($archive.Entries.Count -gt 5000){throw 'source_entry_limit'}
        foreach($entry in $archive.Entries){
            $parts=$entry.FullName.Split('/')
            if([IO.Path]::IsPathRooted($entry.FullName) -or $entry.FullName.Contains('\') -or
                $entry.FullName.Contains(':') -or $parts -contains '..' -or
                $names.ContainsKey($entry.FullName) -or (($entry.ExternalAttributes -shr 16) -band 0xF000) -eq 0xA000){throw 'unsafe_source_entry'}
            $names[$entry.FullName]=$true; $total+=$entry.Length
            if($total -gt 128MB){throw 'source_expansion_limit'}
        }
    } finally { $archive.Dispose() }
    $source=Join-Path $work 'source'
    Expand-Archive -LiteralPath $sourceZip -DestinationPath $source
    $powershell=Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
    $probe=Join-Path $source 'tools\m8\windows-tcp-state-probe.ps1'
    Write-Host 'Yerel TCP durum kontrolu...'
    foreach($item in @(
        @{name='tcp_summary';args='-Iterations 10 -Summary';file='tcp-summary.log'},
        @{name='tcp_parser';args='-SelfTest';file='tcp-parser.log'}
    )){
        $log=Join-Path $work $item.file; $logs+=$log
        $code=Invoke-TestProcess $powershell ('-NoProfile -ExecutionPolicy Bypass -File "'+$probe+'" '+$item.args) $log $source
        $report.checks+=@{name=$item.name;exit_code=$code;passed=($code -eq 0)}
    }
    $rustup=Get-Command rustup.exe -ErrorAction SilentlyContinue
    $rustupPath=if($rustup){$rustup.Source}else{Join-Path $env:USERPROFILE '.cargo\bin\rustup.exe'}
    if(-not (Test-Path -LiteralPath $rustupPath -PathType Leaf)){throw 'rustup_missing'}
    Write-Host 'Sabitlenmis Rust 1.98.1 ile derleniyor. Ilk derleme uzun surebilir...'
    $buildLog=Join-Path $work 'build.log'; $logs+=$buildLog
    $code=Invoke-TestProcess $rustupPath 'run 1.98.1 cargo test --locked --color never -p cryptexd --lib --no-run' $buildLog $source
    $report.build_exit_code=$code
    if($code -ne 0){throw 'build_failed'}
    foreach($group in @(
        @{name='instance';filter='instance::tests::';expected_count=9},
        @{name='publication';filter='incoming_publication::tests::';expected_count=9},
        @{name='owned_http';filter='owned_process::tests::http::';expected_count=8}
    )){
        Write-Host ('Test: '+$group.name)
        $log=Join-Path $work ($group.name+'.log'); $logs+=$log
        $code=Invoke-TestProcess $rustupPath ('run 1.98.1 cargo test --offline --locked --color never -p cryptexd --lib '+$group.filter+' -- --test-threads=1') $log $source
        $summary=@(Get-Content -LiteralPath $log | Where-Object { $_ -cmatch '^test result:' } | Select-Object -Last 1)
        $expectedSummary='^test result: ok\. '+$group.expected_count+' passed; 0 failed; 0 ignored;'
        $passed=($code -eq 0 -and $summary.Count -eq 1 -and $summary[0] -cmatch $expectedSummary)
        $report.checks+=@{name=$group.name;exit_code=$code;passed=$passed;summary=($summary -join '')}
    }
    if(@($report.checks | Where-Object { -not $_.passed }).Count -eq 0){
        $report.outcome='selected_windows_checks_passed'; $exitCode=0
    }else{$report.outcome='selected_windows_checks_failed'}
} catch {
    $errorCode=$_.Exception.Message
    if($errorCode -cnotmatch '^[a-z_]{1,64}$'){$errorCode='operation_failed'}
    $report.error_code=$errorCode
    $report.outcome='runner_failed'
} finally {
    $report.runner_exit=$exitCode
    $result=Join-Path $work 'result.json'
    [IO.File]::WriteAllText($result,($report | ConvertTo-Json -Depth 8).Replace("`r`n","`n")+"`n",$utf8)
    $output=Join-Path $work 'WIN-TEST-SONUC.zip'
    $files=@($result)+@($logs | Where-Object {Test-Path -LiteralPath $_ -PathType Leaf})
    Compress-Archive -LiteralPath $files -DestinationPath $output
    Write-Host 'WIN-TEST-SONUC.zip hazir. Sonuc basarili olsa da hata olsa da bu dosyayi sohbete ekleyin.'
    Start-Process -FilePath explorer.exe -ArgumentList ('/select,"{0}"' -f $output)
}
exit $exitCode
