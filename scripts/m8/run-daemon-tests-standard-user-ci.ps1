param([string]$M14UpdatePackages,[switch]$M13DesktopShell,[switch]$DesktopStartupOnly,[switch]$DesktopSessionOnly,[switch]$TransportPolicyOnly,[switch]$LanSetupOnly,[switch]$LanCutoffOnly,[string]$LanArtifact,[switch]$ExchangeNativeOnly,[string]$ExchangePackage,[switch]$LocalVerificationOnly,[string]$M9HistoricalDaemon,[switch]$M9AttachmentBrowser,[ValidateSet('prepare','restore')][string]$M11CrossBackup)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
if(([int][bool]$M14UpdatePackages+[int][bool]$M13DesktopShell+[int][bool]$DesktopStartupOnly+[int][bool]$DesktopSessionOnly+[int][bool]$TransportPolicyOnly+[int][bool]$LanSetupOnly+[int][bool]$LanCutoffOnly+[int][bool]$ExchangeNativeOnly+[int][bool]$LocalVerificationOnly+[int][bool]$M9HistoricalDaemon+[int][bool]$M9AttachmentBrowser+[int][bool]$M11CrossBackup) -gt 1){throw 'Choose one scoped suite.'}
if($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'Disposable GitHub-hosted Windows runner required.'
}
$workspace=(Resolve-Path -LiteralPath $env:GITHUB_WORKSPACE).Path
$manifest=Join-Path $workspace 'services\cryptexd'
$python=if($M9HistoricalDaemon -or $M13DesktopShell){(Get-Command python -CommandType Application | Select-Object -First 1).Source}else{$null}
$browserNode=if($M9AttachmentBrowser -or $M13DesktopShell){(Get-Command node -CommandType Application | Select-Object -First 1).Source}else{$null}
if($ExchangeNativeOnly -or $M9HistoricalDaemon -or $M13DesktopShell){$ExchangePackage=(Resolve-Path -LiteralPath $ExchangePackage).Path}
if($M14UpdatePackages) {
    $M14UpdatePackages=(Resolve-Path -LiteralPath $M14UpdatePackages).Path
    $python=(Get-Command python -CommandType Application | Select-Object -First 1).Source
    $executables=@(); $expectedSuites=1
} elseif($LanSetupOnly -or $LanCutoffOnly) {
    $LanArtifact=(Resolve-Path -LiteralPath $LanArtifact).Path
    $python=(Get-Command python -CommandType Application | Select-Object -First 1).Source
    $executables=@(); $expectedSuites=1
} else {
$buildArgs=@('test','--locked','-p','cryptexd','--no-run','--message-format=json')
if($DesktopStartupOnly){$buildArgs+=@('--test','desktop_startup')}
if($M13DesktopShell -or $DesktopSessionOnly -or $TransportPolicyOnly -or $ExchangeNativeOnly -or $LocalVerificationOnly -or $M9HistoricalDaemon -or $M9AttachmentBrowser -or $M11CrossBackup){$buildArgs+=@('--lib')}
$artifacts=@(& cargo @buildArgs)
if($LASTEXITCODE -ne 0){throw 'Daemon test build failed.'}
$executables=@($artifacts | ForEach-Object {$_ | ConvertFrom-Json} | Where-Object {
    $_.reason -eq 'compiler-artifact' -and
    $_.target.name -eq $(if($DesktopStartupOnly){'desktop_startup'}else{'cryptexd'}) -and
    $_.profile.test -and $_.executable
} | ForEach-Object {$_.executable} | Sort-Object -Unique)
$expectedArtifacts=if($M13DesktopShell -or $DesktopStartupOnly -or $DesktopSessionOnly -or $TransportPolicyOnly -or $ExchangeNativeOnly -or $LocalVerificationOnly -or $M9HistoricalDaemon -or $M9AttachmentBrowser -or $M11CrossBackup){1}else{2}
$expectedSuites=if($M13DesktopShell){2}elseif($M9HistoricalDaemon){5}elseif($LocalVerificationOnly){1}elseif($ExchangeNativeOnly){5}elseif($TransportPolicyOnly){13}elseif($DesktopSessionOnly){3}else{$expectedArtifacts}
if($executables.Count -ne $expectedArtifacts){throw 'Expected exact daemon test artifacts.'}
foreach($exe in $executables) {
    $full=(Resolve-Path -LiteralPath $exe).Path
    if(-not $full.StartsWith($workspace+'\target\',[StringComparison]::OrdinalIgnoreCase)){throw 'Test artifact outside workspace target.'}
}
}
$nonce=[guid]::NewGuid().ToString('N').Substring(0,12)
$userName='cpxCi'+$nonce
$scratch=Join-Path $env:RUNNER_TEMP ('cryptex-standard-'+$nonce)
New-Item -ItemType Directory -Path $scratch | Out-Null
$user=$null; $child=$null; $failure=$null
try {
    $bytes=New-Object byte[] 32
    $rng=[Security.Cryptography.RandomNumberGenerator]::Create()
    try {$rng.GetBytes($bytes)} finally {$rng.Dispose()}
    $password=ConvertTo-SecureString ([Convert]::ToBase64String($bytes)+'aA1!') -AsPlainText -Force
    $user=New-LocalUser -Name $userName -Password $password -AccountNeverExpires -Description 'Cryptex disposable standard-user tests'
    $usersSid=[Security.Principal.SecurityIdentifier]::new('S-1-5-32-545')
    if(-not (Get-LocalGroupMember -SID $usersSid | Where-Object {$_.SID -eq $user.SID})) {
        Add-LocalGroupMember -SID $usersSid -Member $user
    }
    # Fresh account reads checkout/build artifacts; only scratch is writable.
    $access=@(@{path=$workspace;rights='ReadAndExecute'},@{path=$scratch;rights='FullControl'})
    if($ExchangeNativeOnly -or $M9HistoricalDaemon -or $M13DesktopShell){$access+=@{path=$ExchangePackage;rights='ReadAndExecute'}}
    if($LanSetupOnly -or $LanCutoffOnly){$access+=@{path=$LanArtifact;rights='ReadAndExecute'}}
    if($M14UpdatePackages){$access+=@{path=$M14UpdatePackages;rights='ReadAndExecute'}}
    foreach($entry in $access) {
        $acl=Get-Acl -LiteralPath $entry.path
        $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($user.SID,$entry.rights,'ContainerInherit,ObjectInherit','None','Allow'))
        Set-Acl -LiteralPath $entry.path -AclObject $acl
    }
    if($M11CrossBackup) {
        $exchange=Join-Path $env:RUNNER_TEMP 'exchange'
        foreach($folder in @('public','private','opened','peer-public')) {
            New-Item -ItemType Directory -Path (Join-Path $scratch $folder) | Out-Null
        }
        if($M11CrossBackup -eq 'restore') {
            foreach($member in @('opened/content.zip','opened/inventory.json','peer-public/selection.json')) {
                Copy-Item -LiteralPath (Join-Path $exchange $member) -Destination (Join-Path $scratch $member)
            }
        }
    }
    $payload=Join-Path $scratch 'tests.json'
    @{schema=1;m14_update_packages=$M14UpdatePackages;m13_desktop_shell=[bool]$M13DesktopShell;m13_browser_node=$(if($M13DesktopShell){$browserNode});m11_cross_backup=$M11CrossBackup;m9_attachment_browser_node=$browserNode;m9_historical_daemon=$M9HistoricalDaemon;lan_cutoff_only=[bool]$LanCutoffOnly;local_verification_only=[bool]$LocalVerificationOnly;sid=$user.SID.Value;user_name=$userName;scratch=$scratch;manifest=$manifest;executables=$executables;desktop_startup_only=[bool]$DesktopStartupOnly;desktop_session_only=[bool]$DesktopSessionOnly;transport_policy_only=[bool]$TransportPolicyOnly;lan_setup_only=[bool]$LanSetupOnly;lan_artifact=$LanArtifact;exchange_native_only=[bool]$ExchangeNativeOnly;exchange_package=$ExchangePackage;python=$python;workspace=$workspace} |
        ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $payload -Encoding UTF8
    $stdout=Join-Path $scratch 'stdout.log'; $stderr=Join-Path $scratch 'stderr.log'
    $bootstrap=Join-Path $PSScriptRoot 'standard-user-daemon-child.ps1'
    $credential=[Management.Automation.PSCredential]::new(('.\'+$userName),$password)
    $child=Start-Process -FilePath "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
        -ArgumentList ('-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "{0}" -Payload "{1}"' -f $bootstrap,$payload) `
        -Credential $credential -LoadUserProfile -WindowStyle Hidden -PassThru `
        -Environment @{GITHUB_ACTIONS='true';RUNNER_ENVIRONMENT='github-hosted';RUNNER_OS='Windows'} `
        -WorkingDirectory $manifest -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    if(-not $child.WaitForExit(1200000)){throw 'Standard-user daemon tests timed out.'}
    $child.WaitForExit()
    Get-Content -LiteralPath $stdout
    Get-Content -LiteralPath $stderr
    if($child.ExitCode -ne 0){throw 'Standard-user daemon tests failed.'}
    $report=Get-Content -LiteralPath (Join-Path $scratch 'result.json') -Raw | ConvertFrom-Json
    if($report.sid -ne $user.SID.Value -or $report.elevated -or -not $report.passed -or $report.suites -ne $expectedSuites){throw 'Missing standard-user proof.'}
    if($M13DesktopShell) {
        $display=New-Item -ItemType Directory -Path (Join-Path $env:RUNNER_TEMP 'm14-display') -ErrorAction Stop
        foreach($file in @('display-keyboard.json','light-1x.png','light-1.5x.png','light-2x.png','dark-1x.png','dark-1.5x.png','dark-2x.png')) {
            [IO.File]::Copy((Join-Path $scratch "m14-display/$file"),(Join-Path $display.FullName $file),$false)
        }
    }
    if($M14UpdatePackages) {
        [IO.File]::Copy((Join-Path $scratch 'm14-installed-update.json'),(Join-Path $env:RUNNER_TEMP 'm14-installed-update.json'),$false)
    }
    if($M11CrossBackup -eq 'prepare') {
        foreach($member in @('private/content.zip','public/selection.json')) {
            [IO.File]::Copy((Join-Path $scratch $member),(Join-Path $exchange $member),$false)
        }
    }
} catch {$failure=$_.Exception.Message}
finally {
    if($null -ne $child) {
        if(-not $child.HasExited) {
            & taskkill.exe /PID $child.Id /T /F | Out-Null
            if(-not $child.WaitForExit(10000)){$failure='Test process cleanup failed.'}
        }
        $child.Dispose()
    }
    if($null -ne $user) {
        try {
            $profile=Get-CimInstance Win32_UserProfile | Where-Object SID -eq $user.SID.Value
            $unloadDeadline=[DateTime]::UtcNow.AddSeconds(10)
            while($profile -and $profile.Loaded -and [DateTime]::UtcNow -lt $unloadDeadline) {
                Start-Sleep -Milliseconds 250
                $profile=Get-CimInstance Win32_UserProfile | Where-Object SID -eq $user.SID.Value
            }
            if($profile) {
                $profilePath=[IO.Path]::GetFullPath($profile.LocalPath)
                $usersRoot=[IO.Path]::GetFullPath((Join-Path $env:SystemDrive 'Users')).TrimEnd('\')+'\'
                if($profile.Loaded -or -not $profilePath.StartsWith($usersRoot,[StringComparison]::OrdinalIgnoreCase) -or
                    [IO.Path]::GetFileName($profilePath) -notmatch ('^'+[regex]::Escape($userName)+'(\..*)?$')){throw 'Unexpected/loaded CI profile.'}
                Remove-CimInstance -InputObject $profile
            }
        } catch {$failure='Temporary profile cleanup failed.'}
        try {
            Remove-LocalUser -SID $user.SID
            if(Get-LocalUser -SID $user.SID -ErrorAction SilentlyContinue){throw 'CI account still exists.'}
        } catch {$failure='Temporary account cleanup failed.'}
    }
    if(-not $failure) {
        $full=[IO.Path]::GetFullPath($scratch)
        $parent=[IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\')+'\'
        if(-not $full.StartsWith($parent,[StringComparison]::OrdinalIgnoreCase) -or
            [IO.Path]::GetFileName($full) -cne ('cryptex-standard-'+$nonce) -or
            ((Get-Item -LiteralPath $full).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            $failure='Unexpected CI scratch cleanup target.'
        } else {
            try {Remove-Item -LiteralPath $full -Recurse -Force} catch {$failure='CI scratch cleanup failed.'}
        }
    }
    # Failure diagnostics remain only on the disposable runner; no profile/key archive uploaded.
}
if($failure){throw $failure}
Write-Output 'standard_user_daemon_tests=passed; temporary_account_profile_removed=true'
