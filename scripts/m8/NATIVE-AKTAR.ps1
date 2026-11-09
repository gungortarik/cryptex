$ErrorActionPreference='Stop'
$work=Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\native-transfer\'+[guid]::NewGuid().ToString('N').Substring(0,12))
New-Item -ItemType Directory -Path $work | Out-Null
$utf8=New-Object Text.UTF8Encoding($false)
$report=[ordered]@{schema=1; scope='synthetic_real_native_worker_publication'; outcome='not_completed';
    production_profiles_accessed=$false; power_cut_tested=$false; security_settings_changed=$false}
$exitCode=1
$session='unknown'
$test=$null
$testName='managed_profile::native_transfer_test::real_delivery_worker_publication_crash_restart_and_replay'

function Assert-SourceArchive([string]$Path) {
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip=[IO.Compression.ZipFile]::OpenRead($Path)
    try {
        $total=0L; $names=@{}
        if($zip.Entries.Count -gt 5000){throw 'source_entry_limit'}
        foreach($entry in $zip.Entries){
            $parts=$entry.FullName.Split('/')
            if([IO.Path]::IsPathRooted($entry.FullName) -or $entry.FullName.Contains('\') -or
                $entry.FullName.Contains(':') -or $parts -contains '..' -or
                $names.ContainsKey($entry.FullName) -or (($entry.ExternalAttributes -shr 16) -band 0xF000) -eq 0xA000){throw 'unsafe_source_entry'}
            $names[$entry.FullName]=$true; $total+=$entry.Length
            if($total -gt 128MB){throw 'source_expansion_limit'}
        }
    } finally {$zip.Dispose()}
}
function Test-NativeSummary([int]$Code,[string[]]$Summary) {
    return ($Code -eq 0 -and $Summary.Count -eq 1 -and
        $Summary[0] -cmatch '^test result: ok\. 1 passed; 0 failed; 0 ignored;')
}
function Start-RunnerProcess([string]$Executable,[string]$Arguments,[string]$Source) {
    $start=New-Object Diagnostics.ProcessStartInfo
    $start.FileName=$Executable; $start.Arguments=$Arguments; $start.WorkingDirectory=$Source
    $start.UseShellExecute=$false; $start.CreateNoWindow=$true
    $start.RedirectStandardOutput=$true; $start.RedirectStandardError=$true
    $start.EnvironmentVariables['CARGO_TARGET_DIR']=Join-Path $work 'target'
    $start.EnvironmentVariables['CRYPTEX_NATIVE_TRANSFER_TEST']=$work
    $start.EnvironmentVariables['TEMP']=Join-Path $work 'test-temp'
    $start.EnvironmentVariables['TMP']=Join-Path $work 'test-temp'
    $process=[Diagnostics.Process]::Start($start)
    return @{process=$process; stdout=$process.StandardOutput.ReadToEndAsync(); stderr=$process.StandardError.ReadToEndAsync()}
}
function Finish-RunnerProcess($Run,[string]$Name) {
    $Run.process.WaitForExit()
    [IO.File]::WriteAllText((Join-Path $work $Name),$Run.stdout.Result+"`n"+$Run.stderr.Result,$utf8)
    $code=$Run.process.ExitCode
    $Run.process.Dispose()
    return $code
}
try {
    $principal=[Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
    if($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'run_without_administrator'}
    # Protect only the freshly created test root, before any secrets are created.
    $sid=[Security.Principal.WindowsIdentity]::GetCurrent().User
    $acl=New-Object Security.AccessControl.DirectorySecurity
    $acl.SetOwner($sid); $acl.SetAccessRuleProtection($true,$false)
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($sid,'FullControl','ContainerInherit,ObjectInherit','None','Allow'))
    Set-Acl -LiteralPath $work -AclObject $acl
    New-Item -ItemType Directory -Path (Join-Path $work 'test-temp') | Out-Null
    $commit=(Get-Content -LiteralPath (Join-Path $PSScriptRoot 'SOURCE-COMMIT.txt') -Raw).Trim()
    $expected=(Get-Content -LiteralPath (Join-Path $PSScriptRoot 'SOURCE-SHA256.txt') -Raw).Trim()
    if($commit -cnotmatch '^[0-9a-f]{40}$' -or $expected -cnotmatch '^[0-9a-f]{64}$'){throw 'invalid_source_binding'}
    $sourceZip=Join-Path $PSScriptRoot 'source.zip'
    if((Get-FileHash -LiteralPath $sourceZip).Hash.ToLowerInvariant() -cne $expected){throw 'source_hash_mismatch'}
    Assert-SourceArchive $sourceZip
    $source=Join-Path $work 'source'
    Expand-Archive -LiteralPath $sourceZip -DestinationPath $source
    $report.source_commit=$commit; $report.source_sha256=$expected
    $relayPath=Join-Path $PSScriptRoot 'relay.json'
    if((Get-Item -LiteralPath $relayPath).Length -gt 2048){throw 'invalid_relay'}
    $relay=Get-Content -LiteralPath $relayPath -Raw | ConvertFrom-Json
    if($relay.schema -ne 1 -or $relay.session -cnotmatch '^[0-9a-f]{32}$'){throw 'invalid_relay'}
    $session=$relay.session
    Copy-Item -LiteralPath $relayPath -Destination (Join-Path $work 'relay.json')
    New-Item -ItemType Directory -Path (Join-Path $work 'native-package') | Out-Null
    foreach($name in @('syncthing.exe','LICENSE.txt','AUTHORS.txt')) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination (Join-Path $work 'native-package')
    }
    $report.os=[Environment]::OSVersion.Version.ToString()
    $report.filesystem=([IO.DriveInfo]::new([IO.Path]::GetPathRoot($work))).DriveFormat
    $rustup=Get-Command rustup.exe -ErrorAction SilentlyContinue
    $rustupPath=if($rustup){$rustup.Source}else{Join-Path $env:USERPROFILE '.cargo\bin\rustup.exe'}
    if(-not (Test-Path -LiteralPath $rustupPath -PathType Leaf)){throw 'rustup_missing'}
    Write-Host 'Test derleniyor. Ilk derleme uzun surebilir; pencereyi acik tutun.'
    $build=Start-RunnerProcess $rustupPath 'run 1.98.1 cargo test --locked --color never -p cryptexd --lib --no-run' $source
    $report.build_exit=Finish-RunnerProcess $build 'build.log'
    if($report.build_exit -ne 0){throw 'build_failed'}
    $test=Start-RunnerProcess $rustupPath ('run 1.98.1 cargo test --offline --locked --color never -p cryptexd --lib '+$testName+' -- --exact --ignored --nocapture --test-threads=1') $source
    $exported=$false
    while(-not $test.process.WaitForExit(500)) {
        if(-not $exported -and (Test-Path -LiteralPath (Join-Path $work 'request.ready'))) {
            $requestZip=Join-Path $env:USERPROFILE ('Downloads\DENETLEYICI-ISTEK-'+$session.Substring(0,8)+'-'+[IO.Path]::GetFileName($work)+'.zip')
            Compress-Archive -Path (Join-Path $work 'public-request\*') -DestinationPath $requestZip
            $exported=$true
            Write-Host "`nISTEK HAZIR: $requestZip" -ForegroundColor Green
            Write-Host "Bu ZIP dosyasini sohbete ekleyin. Bu pencereyi ve VM'i acik tutun (30 dakika bekler)."
        }
    }
    $report.test_exit=Finish-RunnerProcess $test 'native-test.log'
    $test=$null
    $summary=@(Get-Content -LiteralPath (Join-Path $work 'native-test.log') | Where-Object {$_ -cmatch '^test result:'})
    $report.summary=$summary -join ''
    if(-not (Test-NativeSummary $report.test_exit $summary)){throw 'native_test_failed'}
    $native=Get-Content -LiteralPath (Join-Path $work 'result.json') -Raw | ConvertFrom-Json
    if(-not $native.passed -or $native.session -cne $session){throw 'native_receipt_mismatch'}
    $report.outcome='native_controller_pilot_passed'; $exitCode=0
} catch {
    $code=$_.Exception.Message
    if($code -cnotmatch '^[a-z_]{1,64}$'){$code='runner_operation_failed'}
    $report.error_code=$code; $report.outcome='failed'
    Write-Host ('Kontrol tamamlanamadi: '+$code) -ForegroundColor Yellow
    if($null -ne $test -and -not $test.process.HasExited) {
        [IO.File]::WriteAllText((Join-Path $work 'cancel.request'),'cancel',$utf8)
        $report.cancel_cleanup_observed=$test.process.WaitForExit(50000)
        if($report.cancel_cleanup_observed){$report.test_exit=Finish-RunnerProcess $test 'native-test.log'}
    }
} finally {
    $report.session=$session; $report.runner_exit=$exitCode
    [IO.File]::WriteAllText((Join-Path $work 'runner-result.json'),($report|ConvertTo-Json -Depth 8),$utf8)
    $files=@('runner-result.json','result.json','build.log','native-test.log','checkpoint.txt','relay.json') |
        ForEach-Object {Join-Path $work $_} | Where-Object {Test-Path -LiteralPath $_ -PathType Leaf}
    $out=Join-Path $work 'DENETLEYICI-SONUC.zip'
    Compress-Archive -LiteralPath $files -DestinationPath $out
    $download=Join-Path $env:USERPROFILE ('Downloads\DENETLEYICI-SONUC-'+$session.Substring(0,[Math]::Min(8,$session.Length))+'-'+[IO.Path]::GetFileName($work)+'.zip')
    Copy-Item -LiteralPath $out -Destination $download
    Write-Host "`nSONUC: $download"
    Write-Host 'Bu sonuc ZIP dosyasini sohbete ekleyin.'
}
exit $exitCode
