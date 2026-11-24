param([Parameter(Mandatory)][string]$Launcher)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'Requires a disposable hosted Windows VM'
}
$account = 'cryptexm14'
if (Get-LocalUser -Name $account -ErrorAction SilentlyContinue) { throw 'Existing account' }
$password = ConvertTo-SecureString (([guid]::NewGuid().ToString('N')) + 'Aa!1') -AsPlainText -Force
$user = New-LocalUser -Name $account -Password $password
$cred = [Management.Automation.PSCredential]::new((".\" + $account), $password)
$scratch = (New-Item -ItemType Directory -Path (Join-Path $env:PUBLIC ('cryptex-trial-' + [guid]::NewGuid().ToString('N')))).FullName
$exe = Join-Path $scratch 'KUR.exe'
Copy-Item -LiteralPath $Launcher -Destination $exe
& icacls.exe $scratch /grant ("*{0}:(OI)(CI)M" -f $user.SID.Value) | Out-Null
if ($LASTEXITCODE) { throw 'Fixture ACL failed' }
$root = $null
function Invoke-TrialUser($program, $arguments, $label) {
    $options = @{
        FilePath = $program; Credential = $cred; LoadUserProfile = $true; WorkingDirectory = $scratch
        RedirectStandardOutput = "$scratch/$label.out"; RedirectStandardError = "$scratch/$label.err"
        PassThru = $true; WindowStyle = 'Hidden'
    }
    if ($arguments) { $options.ArgumentList = $arguments }
    $child = Start-Process @options
    try {
        if (!$child.WaitForExit(240000)) { $child.Kill($true); throw "$label timed out" }
        $child.WaitForExit()
        Get-Content -LiteralPath "$scratch/$label.out","$scratch/$label.err"
        if ($child.ExitCode -ne 0) { throw "$label exited with $($child.ExitCode)" }
    } finally { $child.Dispose() }
}
try {
    if (!(Get-LocalGroupMember -SID 'S-1-5-32-545' | Where-Object { $_.SID -eq $user.SID })) {
        Add-LocalGroupMember -SID 'S-1-5-32-545' -Member $user
    }
    # Exercise the shipped key creation routine in the target user's own key store.
    # Pretrust only its public leaf on the disposable runner so the CI service
    # need not answer the interactive Windows trust dialog on the owner's behalf.
    $prepare = @'
$ErrorActionPreference = 'Stop'
$type = [Reflection.Assembly]::LoadFile((Join-Path (Get-Location).Path 'KUR.exe')).GetType('Install')
$id = [Security.Principal.WindowsIdentity]::GetCurrent()
$method = $type.GetMethod('Identity', [Reflection.BindingFlags]'NonPublic,Static')
$cert = $method.Invoke($null, @($id.User.Value))
[IO.File]::WriteAllBytes((Join-Path (Get-Location).Path 'public.cer'), $cert.RawData)
'@
    $script = Join-Path $scratch 'prepare.ps1'
    Set-Content -LiteralPath $script -Value $prepare
    Invoke-TrialUser "$env:WINDIR/System32/WindowsPowerShell/v1.0/powershell.exe" "-NoProfile -NonInteractive -ExecutionPolicy RemoteSigned -File `"$script`"" 'prepare'
    $cert = [Security.Cryptography.X509Certificates.X509Certificate2]::new("$scratch/public.cer")
    if ($cert.Subject -ne ('CN=Cryptex M17 Trial ' + $user.SID.Value)) { throw 'Unexpected fixture identity' }
    $root = "Cert:\LocalMachine\Root\$($cert.Thumbprint)"
    if (Test-Path $root) { $root = $null; throw 'Preexisting fixture trust' }
    Import-Certificate -FilePath "$scratch/public.cer" -CertStoreLocation Cert:\LocalMachine\Root | Out-Null
    $first = $null
    foreach ($attempt in 'first', 'repeat') {
        Invoke-TrialUser $exe $null $attempt
        $result = Get-Content -LiteralPath "$scratch/M17-IMZA-SONUC.json" -Raw | ConvertFrom-Json
        if ($result.passed -ne $true -or $result.revision -ne $env:GITHUB_SHA -or
            $result.certificate_sha256 -ne (Get-FileHash "$scratch/public.cer").Hash -or
            $result.installed_desktop_sha256 -notmatch '^[A-F0-9]{64}$' -or
            $result.installed_daemon_sha256 -notmatch '^[A-F0-9]{64}$') { throw 'Incomplete installed receipt' }
        if ($first -and ($first.installed_desktop_sha256 -ne $result.installed_desktop_sha256 -or
                         $first.installed_daemon_sha256 -ne $result.installed_daemon_sha256)) { throw 'Repeat replaced installed files' }
        $first = $result
    }
    Write-Output 'The standard user installed and verified the owner-signed trial twice.'
} finally {
    if ($root -and (Test-Path $root)) { Remove-Item -LiteralPath $root }
    Remove-LocalUser -SID $user.SID
}
