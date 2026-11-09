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
$profile=[Environment]::GetFolderPath([Environment+SpecialFolder]::UserProfile)
if(-not $profile -or -not (Test-Path -LiteralPath $profile -PathType Container)){throw 'Loaded user profile required.'}
if((Get-Acl -LiteralPath $profile).GetOwner([Security.Principal.SecurityIdentifier]).Value -ne $p.sid){throw 'Loaded profile owner mismatch.'}
$env:USERPROFILE=$profile
$env:LOCALAPPDATA=Join-Path $profile 'AppData\Local'
$env:APPDATA=Join-Path $profile 'AppData\Roaming'
$env:TEMP=Join-Path $p.scratch 'private-temp'
$env:TMP=$env:TEMP
New-Item -ItemType Directory -Path $env:TEMP | Out-Null
$owner=(Get-Acl -LiteralPath $env:TEMP).GetOwner([Security.Principal.SecurityIdentifier]).Value
if($owner -ne $p.sid){throw 'Temporary directory owner mismatch.'}
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
function Assert-CompleteSuite([int]$Code,[string]$Stdout,[int]$Count) {
    $summary=@($Stdout -split '\r?\n' | Where-Object {$_ -cmatch '^test result:'})
    # Process-interruption tests may print nested harness summaries. The final
    # summary must cover this executable's complete pre-enumerated inventory.
    if($Code -ne 0 -or $Count -lt 1 -or $summary.Count -eq 0 -or $summary[-1] -cnotmatch '^test result: ok\. ([1-9][0-9]*) passed; 0 failed; ([0-9]+) ignored; 0 measured; 0 filtered out;'){throw 'Daemon test suite failed or incomplete.'}
    if(([int]$Matches[1]+[int]$Matches[2]) -ne $Count){throw 'Incomplete daemon inventory coverage.'}
}
$suites=0
foreach($exe in $p.executables) {
    $list=Invoke-TestBinary $exe '--list'
    $count=@($list.stdout -split '\r?\n' | Where-Object {$_ -cmatch ': test$'}).Count
    if($list.code -ne 0 -or $count -eq 0){throw 'Missing daemon test inventory.'}
    $run=Invoke-TestBinary $exe ''
    $run.stdout | Write-Output
    $run.stderr | Write-Output
    Assert-CompleteSuite $run.code $run.stdout $count
    $suites++
}
@{sid=$identity.User.Value;elevated=$false;passed=$true;suites=$suites} | ConvertTo-Json |
    Set-Content -LiteralPath (Join-Path $p.scratch 'result.json') -Encoding UTF8
