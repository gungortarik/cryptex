param([Parameter(Mandatory=$true)][string]$Payload)
# Parent runs PowerShell7; this credential child is Windows PowerShell5.1.
# Do not inherit incompatible PS7 module paths into native PS5.1 autoload.
$env:PSModulePath="$PSHOME\Modules"
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
if($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows'){throw 'Hosted CI required.'}
$p=Get-Content -LiteralPath $Payload -Raw | ConvertFrom-Json
$identity=[Security.Principal.WindowsIdentity]::GetCurrent()
$principal=[Security.Principal.WindowsPrincipal]::new($identity)
if($identity.User.Value -ne $p.sid -or $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Standard-user token required.'}
$env:TEMP=Join-Path $p.scratch 'private-temp'
$env:TMP=$env:TEMP
New-Item -ItemType Directory -Path $env:TEMP | Out-Null
$owner=(Get-Acl -LiteralPath $env:TEMP).GetOwner([Security.Principal.SecurityIdentifier]).Value
if($owner -ne $p.sid){throw 'Temporary directory owner mismatch.'}
# Environment/GetFolderPath can retain the launching runner's profile. Resolve
# the credential process profile from its actual TOKEN_QUERY handle instead.
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class CryptexCiProfile {
    [DllImport("userenv.dll", EntryPoint="GetUserProfileDirectoryW", CharSet=CharSet.Unicode, SetLastError=true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool ReadProfile(IntPtr token, StringBuilder path, ref uint size);
    public static string Read(IntPtr token) {
        uint size=0; ReadProfile(token, null, ref size);
        if(size<2 || size>32768) throw new InvalidOperationException("Profile size unavailable");
        var path=new StringBuilder((int)size);
        if(!ReadProfile(token, path, ref size)) throw new Win32Exception(Marshal.GetLastWin32Error());
        return path.ToString();
    }
}
'@
$profile=[CryptexCiProfile]::Read($identity.Token)
if(-not (Test-Path -LiteralPath $profile -PathType Container) -or
    [IO.Path]::GetFileName($profile) -notmatch ('^'+[regex]::Escape($p.user_name)+'(\..*)?$')){throw 'Fresh credential profile required.'}
$env:USERPROFILE=$profile
$env:LOCALAPPDATA=Join-Path $profile 'AppData\Local'
$env:APPDATA=Join-Path $profile 'AppData\Roaming'
$env:CARGO_MANIFEST_DIR=$p.manifest
Set-Location -LiteralPath $p.manifest
function Invoke-TestBinary([string]$Exe,[string]$Arguments) {
    $start=New-Object Diagnostics.ProcessStartInfo
    $start.FileName=$Exe; $start.Arguments=$Arguments; $start.WorkingDirectory=$p.manifest
    $start.UseShellExecute=$false; $start.CreateNoWindow=$true
    $start.RedirectStandardOutput=$true; $start.RedirectStandardError=$true
    $process=[Diagnostics.Process]::Start($start)
    try {
        $stdout=$process.StandardOutput.ReadToEndAsync(); $stderr=$process.StandardError.ReadToEndAsync()
        if(-not $process.WaitForExit(1080000)) {
            & taskkill.exe /PID $process.Id /T /F | Out-Null
            $process.WaitForExit(10000) | Out-Null
            throw 'Daemon executable timeout.'
        }
        return @{code=$process.ExitCode;stdout=$stdout.Result;stderr=$stderr.Result}
    } finally {$process.Dispose()}
}
function Assert-CompleteSuite([int]$Code,[string]$Stdout,[int]$Count,[bool]$NoIgnored=$false,[int]$Filtered=0) {
    $summary=@($Stdout -split '\r?\n' | Where-Object {$_ -cmatch '^test result:'})
    # Process-interruption tests may print nested harness summaries. The final
    # summary must cover this executable's complete pre-enumerated inventory.
    if($Code -ne 0 -or $Count -lt 1 -or $Filtered -lt 0 -or $summary.Count -eq 0 -or $summary[-1] -cnotmatch '^test result: ok\. ([1-9][0-9]*) passed; 0 failed; ([0-9]+) ignored; 0 measured; ([0-9]+) filtered out;'){throw 'Daemon test suite failed or incomplete.'}
    if(([int]$Matches[1]+[int]$Matches[2]) -ne $Count){throw 'Incomplete daemon inventory coverage.'}
    if([int]$Matches[3] -ne $Filtered){throw 'Unexpected excluded test inventory.'}
    if($NoIgnored -and [int]$Matches[2] -ne 0){throw 'Startup suite must execute every test, including the endpoint fixture.'}
}
function Set-NativeTestPackage {
    $nativePackage=Join-Path $env:TEMP 'exchange-native-package'
    New-Item -ItemType Directory -Path $nativePackage | Out-Null
    foreach($leaf in @('syncthing.exe','LICENSE.txt','AUTHORS.txt')) {
        Copy-Item -LiteralPath (Join-Path $p.exchange_package $leaf) -Destination $nativePackage
    }
    $env:CRYPTEX_EXCHANGE_NATIVE_PACKAGE=$nativePackage
}
if($p.PSObject.Properties['m9_attachment_browser_node'] -and $p.m9_attachment_browser_node) {
    $env:CRYPTEX_ATTACHMENT_TEST_BINARY=$p.executables[0]
    $script=Join-Path $p.workspace 'apps\desktop\scripts\attachment-flow.test.mjs'
    $result=Invoke-TestBinary $p.m9_attachment_browser_node ('--test --test-reporter=tap "'+$script+'"')
    $result.stdout | Write-Output
    $result.stderr | Write-Output
    if($result.code -ne 0 -or $result.stdout -notmatch '(?m)^# pass 4\r?$' -or $result.stdout -notmatch '(?m)^ok \d+ - purge preserves a shared attachment until its last note is deleted\r?$' -or $result.stdout -notmatch '(?m)^ok \d+ - .*disposable controls physical\r?$' -or $result.stdout -notmatch '(?m)^# fail 0\r?$'){throw 'Isolated attachment browser suite incomplete.'}
    @{sid=$identity.User.Value;elevated=$false;passed=$true;suites=1} | ConvertTo-Json |
        Set-Content -LiteralPath (Join-Path $p.scratch 'result.json') -Encoding UTF8
    exit 0
}
if($p.PSObject.Properties['m9_historical_daemon'] -and $p.m9_historical_daemon) {
    $env:CRYPTEX_M9_PREVIOUS_DAEMON=$p.m9_historical_daemon
    $env:CRYPTEX_M9_CANDIDATE_DAEMON=Join-Path $p.workspace 'target\debug\cryptexd.exe'
    $suite='attachment_blobs::tests::signed_link::audit_pre_m9_profile_admission'
    $result=Invoke-TestBinary $p.executables[0] "$suite --exact --ignored --nocapture"
    $result.stdout | Write-Output
    $result.stderr | Write-Output
    if($result.code -ne 0 -or $result.stdout -notmatch '1 passed; 0 failed; 0 ignored' -or
        $result.stdout -notmatch 'historical_profile_lock=denied'){throw 'Historical M8 denial was not verified.'}
    $env:CRYPTEX_M9_READER_ATTEMPT=[guid]::NewGuid().ToString('N')
    $cutoff=Invoke-TestBinary $p.executables[0] 'knowledge_materialization::pairing_recovery_tests::shared_attachment::cutoff_preserves_the_other_notes_shared_attachment --exact --nocapture'
    $cutoff.stdout | Write-Output
    $cutoff.stderr | Write-Output
    if($cutoff.code -ne 0 -or $cutoff.stdout -notmatch 'shared_attachment_cutoff=verified'){throw 'Shared attachment cutoff check incomplete.'}
    $readerUnits=Invoke-TestBinary $p.executables[0] 'managed_profile::sealed_export::reader::'
    $readerUnits.stdout | Write-Output
    if($readerUnits.code -ne 0 -or $readerUnits.stdout -notmatch '3 passed; 0 failed; 0 ignored'){throw 'Reader unit checks incomplete.'}
    foreach($phase in @('write','reopen')) {
        $env:CRYPTEX_M9_OBSERVATION_PHASE=$phase
        $nativeReceipt=Invoke-TestBinary $p.executables[0] 'managed_profile::native_observation_survives_process_exit --exact --ignored'
        $nativeReceipt.stdout | Write-Output
        if($nativeReceipt.code -ne 0 -or $nativeReceipt.stdout -notmatch '1 passed; 0 failed; 0 ignored'){throw "Native reader evidence $phase failed."}
    }
    $preflight=Invoke-TestBinary $p.executables[0] 'managed_profile::sealed_export::tests::reader_preflight:: --nocapture'
    $preflight.stdout | Write-Output
    $preflight.stderr | Write-Output
    if($preflight.code -ne 0 -or $preflight.stdout -notmatch '5 passed; 0 failed; 0 ignored'){throw 'Paired reader checks incomplete.'}
    Set-NativeTestPackage
    & $p.python -B (Join-Path $p.workspace 'scripts/m9/test_paired_attachment.py') --native (Join-Path $env:CRYPTEX_EXCHANGE_NATIVE_PACKAGE 'syncthing.exe') --test-binary $p.executables[0]
    if($LASTEXITCODE -ne 0){throw 'Paired M9 file transfer failed.'}
    $suite='managed_profile::native_product_receive_test::reader_preflight::normal_owners_verify_readers_over_the_private_network'
    $env:CRYPTEX_M9_READER_ATTEMPT=[guid]::NewGuid().ToString('N')
    $live=Invoke-TestBinary $p.executables[0] "$suite --ignored --exact --nocapture"
    $live.stdout | Write-Output
    $live.stderr | Write-Output
    $proofs=@($live.stderr -split '\r?\n' | Where-Object {$_ -ceq 'component=m9_reader_check status=verified peers=1'})
    if($live.code -ne 0 -or $live.stdout -notmatch '1 passed; 0 failed; 0 ignored' -or $proofs.Count -lt 2){throw 'Live paired reader verification incomplete.'}
    @{sid=$identity.User.Value;elevated=$false;passed=$true;suites=5} | ConvertTo-Json |
        Set-Content -LiteralPath (Join-Path $p.scratch 'result.json') -Encoding UTF8
    exit 0
}
$cutoffOnly=$p.PSObject.Properties['lan_cutoff_only'] -and $p.lan_cutoff_only
if($cutoffOnly -or ($p.PSObject.Properties['lan_setup_only'] -and $p.lan_setup_only)) {
    if($cutoffOnly) {
        & $p.python -B (Join-Path $p.workspace 'scripts/m8/test_lan_cutoff_pair.py') --test-windows $p.lan_artifact
    } else {
        & $p.python -B (Join-Path $p.workspace 'scripts/m8/windows_lan_setup_ci.py') $p.lan_artifact
    }
    if($LASTEXITCODE -ne 0){throw 'Windows LAN setup CI failed.'}
    @{sid=$identity.User.Value;elevated=$false;passed=$true;suites=1} | ConvertTo-Json |
        Set-Content -LiteralPath (Join-Path $p.scratch 'result.json') -Encoding UTF8
    exit 0
}
if($p.PSObject.Properties['exchange_native_only'] -and $p.exchange_native_only) {
    Set-NativeTestPackage
    $exe=$p.executables[0]
    $full=Invoke-TestBinary $exe '--list'
    $fullCount=@($full.stdout -split '\r?\n' | Where-Object {$_ -cmatch ': test$'}).Count
    if($full.code -ne 0 -or $fullCount -lt 1){throw 'Missing native test inventory.'}
    $diagnosticFilter='managed_profile::worker::diagnostics::tests::'
    $diagnostics=Invoke-TestBinary $exe "$diagnosticFilter --test-threads=1"
    $diagnostics.stdout | Write-Output
    $diagnostics.stderr | Write-Output
    Assert-CompleteSuite $diagnostics.code $diagnostics.stdout 3 $true ($fullCount-3)
    $filter='managed_profile::native_product_receive_test::restriction::self_retirement_reaches_offline_peer_through_native_relay'
    $result=Invoke-TestBinary $exe "$filter --ignored --exact --test-threads=1"
    $result.stdout | Write-Output
    $result.stderr | Write-Output
    Assert-CompleteSuite $result.code $result.stdout 1 $true ($fullCount-1)
    $filter='managed_profile::native_xml::exchange::tests::supervised_exchange_install_restart_and_revocation'
    $result=Invoke-TestBinary $exe "$filter --ignored --exact --test-threads=1"
    $result.stdout | Write-Output
    $result.stderr | Write-Output
    Assert-CompleteSuite $result.code $result.stdout 1 $true ($fullCount-1)
    $filter='managed_profile::native_product_receive_test::two_normal_owners_exchange_offline_notes_and_replay_after_restart'
    $result=Invoke-TestBinary $exe "$filter --ignored --exact --test-threads=1"
    $result.stdout | Write-Output
    $result.stderr | Write-Output
    Assert-CompleteSuite $result.code $result.stdout 1 $true ($fullCount-1)
    $filter='managed_profile::native_product_receive_test::restriction::restriction_propagates_to_remaining_peer_and_survives_restart'
    $result=Invoke-TestBinary $exe "$filter --ignored --exact --test-threads=1"
    $result.stdout | Write-Output
    $result.stderr | Write-Output
    Assert-CompleteSuite $result.code $result.stdout 1 $true ($fullCount-1)
    @{sid=$identity.User.Value;elevated=$false;passed=$true;suites=5} | ConvertTo-Json |
        Set-Content -LiteralPath (Join-Path $p.scratch 'result.json') -Encoding UTF8
    exit 0
}
$suites=0
$testArguments=''
$startupOnly=[bool]($p.PSObject.Properties['desktop_startup_only'] -and $p.desktop_startup_only)
$sessionOnly=[bool]($p.PSObject.Properties['desktop_session_only'] -and $p.desktop_session_only)
$verificationOnly=[bool]($p.PSObject.Properties['local_verification_only'] -and $p.local_verification_only)
$transportOnly=[bool]($p.PSObject.Properties['transport_policy_only'] -and $p.transport_policy_only)
if($startupOnly) {
    $env:CRYPTEX_STARTUP_DISPOSABLE_ACCOUNT='1'
    $testArguments='--include-ignored --test-threads=1'
}
foreach($exe in $p.executables) {
    $fullList=Invoke-TestBinary $exe '--list'
    $fullCount=@($fullList.stdout -split '\r?\n' | Where-Object {$_ -cmatch ': test$'}).Count
    if($fullList.code -ne 0 -or $fullCount -eq 0){throw 'Missing full daemon test inventory.'}
    $filters=if($verificationOnly){@('knowledge_materialization::local_receipt::tests::')}elseif($transportOnly){@('canonical_writer::tests::','first_capture::','managed_profile::enrollment::','managed_profile::sealed_export::','managed_profile::connection_settings::','managed_profile::exchange_policy::','windows_recovery_reopens_the_enrolled_journal_with_exact_path_spelling','real_import_and_ipc_share_instance_admission_through_commit_cleanup_and_rebuild','managed_profile::exchange_start::','managed_profile::incoming::tests::','import_staging::windows_inbox::tests::','offline_restricted_peer_tail_is_refused_after_owner_confirmed_cutoff','incoming_publication::tests::')}elseif($sessionOnly){@('approval_session::tests::','ipc::tests::','m8_owner_channel::')}else{@('')}
    foreach($filter in $filters) {
        $list=Invoke-TestBinary $exe ("$filter --list".Trim())
        $count=@($list.stdout -split '\r?\n' | Where-Object {$_ -cmatch ': test$'}).Count
        if($list.code -ne 0 -or $count -eq 0 -or $count -gt $fullCount){throw 'Missing daemon test inventory.'}
        $arguments=if($sessionOnly -or $transportOnly -or $verificationOnly){"$filter --test-threads=1"}else{$testArguments}
        $run=Invoke-TestBinary $exe $arguments
        $run.stdout | Write-Output
        $run.stderr | Write-Output
        Assert-CompleteSuite $run.code $run.stdout $count ($startupOnly -or $sessionOnly -or $transportOnly -or $verificationOnly) ($fullCount-$count)
        $suites++
    }
}
@{sid=$identity.User.Value;elevated=$false;passed=$true;suites=$suites} | ConvertTo-Json |
    Set-Content -LiteralPath (Join-Path $p.scratch 'result.json') -Encoding UTF8
