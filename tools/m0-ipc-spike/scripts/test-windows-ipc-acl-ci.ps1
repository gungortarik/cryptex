[CmdletBinding()]
param([string]$Binary = '')

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
# Account creation is restricted to disposable GitHub-hosted Windows runners.
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'This cross-account test runs only on a disposable GitHub-hosted Windows runner.'
}
if ([string]::IsNullOrWhiteSpace($Binary)) {
    $Binary = Join-Path $PSScriptRoot '..\..\..\target\debug\m0-ipc-spike.exe'
}
$binaryPath = (Resolve-Path -LiteralPath $Binary).Path
$nonce = [Guid]::NewGuid().ToString('N').Substring(0, 12)
$id = 'acl-' + $nonce
$pipeName = 'cryptex-m0-p0-3-' + $id
$userName = 'cpxAcl' + $nonce
$runtimeRoot = Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\p0-3\ipc-acl-ci-' + $nonce)
[void][IO.Directory]::CreateDirectory($runtimeRoot)
$utf8 = New-Object Text.UTF8Encoding($false)
$ownerSid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$server = $null
$testUser = $null
$probeResult = $null
$failure = $null
$cleanupErrors = [Collections.Generic.List[string]]::new()
$accountRemoved = $false
$phase = 'server_start'

try {
    $stdout = Join-Path $runtimeRoot 'server.stdout'
    $stderr = Join-Path $runtimeRoot 'server.stderr'
    $server = Start-Process -FilePath $binaryPath -ArgumentList @('serve', '--id', $id) -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        if ($server.HasExited) { throw 'IPC server exited before readiness.' }
        $ready = (Test-Path -LiteralPath $stdout) -and ((Get-Content -LiteralPath $stdout -Raw) -match 'ready id=')
        if (-not $ready) { Start-Sleep -Milliseconds 100 }
    } while (-not $ready -and [DateTime]::UtcNow -lt $deadline)
    if (-not $ready) { throw 'IPC server did not become ready.' }

    $phase = 'same_user_before'
    $before = & $binaryPath client --id $id --role desktop | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or -not $before.accepted -or $before.daemon_pid -ne $server.Id) {
        throw 'Same-user handshake failed before the cross-account check.'
    }

    $phase = 'create_ephemeral_user'
    $randomBytes = New-Object byte[] 32
    $rng = [Security.Cryptography.RandomNumberGenerator]::Create()
    try { $rng.GetBytes($randomBytes) } finally { $rng.Dispose() }
    $password = ConvertTo-SecureString ([Convert]::ToBase64String($randomBytes) + 'aA1!') -AsPlainText -Force
    $testUser = New-LocalUser -Name $userName -Password $password -AccountNeverExpires -Description 'Cryptex ephemeral CI ACL test'
    $usersSid = [Security.Principal.SecurityIdentifier]::new('S-1-5-32-545')
    $alreadyMember = Get-LocalGroupMember -SID $usersSid | Where-Object { $_.SID -eq $testUser.SID }
    if ($null -eq $alreadyMember) { Add-LocalGroupMember -SID $usersSid -Member $testUser }
    $phase = 'different_user_token'
    Add-Type -Path (Join-Path $PSScriptRoot 'WindowsAclProbe.cs')
    $probeResult = [WindowsAclProbe]::Connect($userName, $password, $testUser.SID.Value, $pipeName)
    if ($probeResult.sid -ne $testUser.SID.Value -or $probeResult.sid -eq $ownerSid) {
        throw 'Client thread did not run as the intended distinct Windows user.'
    }
    if (-not $probeResult.denied -or $probeResult.hresult -ne -2147024891) {
        throw 'The different-account connection did not fail with ERROR_ACCESS_DENIED.'
    }
    if ([Security.Principal.WindowsIdentity]::GetCurrent().User.Value -ne $ownerSid) {
        throw 'Client thread did not restore its owner identity.'
    }

    $phase = 'same_user_after'
    $after = & $binaryPath client --id $id --role desktop | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or -not $after.accepted -or $after.daemon_pid -ne $server.Id) {
        throw 'Same-user handshake failed after the cross-account check.'
    }
    $phase = 'passed'
} catch {
    $failure = $_.Exception.Message
} finally {
    foreach ($process in @($server)) {
        if ($null -ne $process) {
            try {
                if (-not $process.HasExited) {
                    $process.Kill()
                    if (-not $process.WaitForExit(5000)) { throw 'Owned test process did not exit.' }
                }
            } catch { $cleanupErrors.Add($_.Exception.Message) }
            finally { $process.Dispose() }
        }
    }
    if ($null -ne $testUser) {
        try {
            Remove-LocalUser -SID $testUser.SID
            $accountRemoved = $true
        } catch { $cleanupErrors.Add($_.Exception.Message) }
    }
}

$passed = $null -eq $failure -and $cleanupErrors.Count -eq 0 -and $accountRemoved
$summary = [ordered]@{
    experiment = 'm0-p0-3-windows-cross-account-acl'
    environment = 'GITHUB_HOSTED_DISPOSABLE_WINDOWS_RUNNER'
    runtime_root = $runtimeRoot
    endpoint = '\\.\pipe\' + $pipeName
    owner_sid = $ownerSid
    different_user_probe = $probeResult
    same_user_before_and_after = ($phase -eq 'passed')
    temporary_account_removed = $accountRemoved
    phase = $phase
    error = $failure
    cleanup_errors = @($cleanupErrors)
    result = if ($passed) { 'PASS_WINDOWS_CROSS_ACCOUNT_ACL_SPIKE_ONLY' } else { 'FAIL_WINDOWS_CROSS_ACCOUNT_ACL_SPIKE' }
}
$json = $summary | ConvertTo-Json -Depth 8
[IO.File]::WriteAllText((Join-Path $runtimeRoot 'summary.json'), $json, $utf8)
$json
if (-not $passed) { throw 'Cross-account IPC ACL verification failed; inspect the structured result.' }
