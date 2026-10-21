# Physical-host regression controls. Runs with normal Windows policy unchanged.
[CmdletBinding()]
param([string]$Cryptexd = '')

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ([string]::IsNullOrWhiteSpace($Cryptexd)) { $Cryptexd = Join-Path $PSScriptRoot '..\..\..\target\debug\cryptexd.exe' }
$binary = (Resolve-Path -LiteralPath $Cryptexd).Path
$observer = Join-Path $PSScriptRoot 'observe-windows-login.ps1'
$powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
$testRoot = Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\p0-2\observer-controls-' + [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffZ'))
[void][IO.Directory]::CreateDirectory($testRoot)
$utf8 = New-Object Text.UTF8Encoding($false)
$results = @()

function Wait-For([scriptblock]$Predicate, [int]$Seconds = 15) {
    $timer = [Diagnostics.Stopwatch]::StartNew()
    while ($timer.Elapsed.TotalSeconds -lt $Seconds) {
        if (& $Predicate) { return $true }
        Start-Sleep -Milliseconds 50
    }
    return $false
}

foreach ($scenario in @('clean', 'killed', 'alive', 'not_started', 'pid_mismatch')) {
    $root = Join-Path $testRoot $scenario
    [void][IO.Directory]::CreateDirectory($root)
    $child = $null
    $monitor = $null
    try {
        if ($scenario -in @('clean', 'killed', 'alive')) {
            $childArgs = 'lifecycle login-hold --runtime-root "' + $root + '"'
            $child = Start-Process -FilePath $binary -ArgumentList $childArgs -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $root 'daemon.stdout') -RedirectStandardError (Join-Path $root 'daemon.stderr')
            $null = $child.Handle
            if (-not (Wait-For { Test-Path -LiteralPath (Join-Path $root 'ready') })) { throw ($scenario + ': daemon never became ready') }
        } elseif ($scenario -eq 'pid_mismatch') {
            [IO.File]::WriteAllText((Join-Path $root 'ready'), ($PID.ToString() + "`n"), $utf8)
        }
        $monitorArgs = '-NoProfile -NonInteractive -File "' + $observer + '" -RuntimeRoot "' + $root + '" -Cryptexd "' + $binary + '" -MaxSeconds 20 -StartTimeoutSeconds 2'
        if ($scenario -eq 'alive') { $monitorArgs = $monitorArgs.Replace('-MaxSeconds 20', '-MaxSeconds 2') }
        $monitor = Start-Process -FilePath $powershell -ArgumentList $monitorArgs -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $root 'observer.stdout') -RedirectStandardError (Join-Path $root 'observer.stderr')
        $null = $monitor.Handle
        if ($scenario -eq 'clean' -or $scenario -eq 'killed') {
            if (-not (Wait-For {
                $logs = @(Get-ChildItem -LiteralPath (Join-Path $root 'observations') -Filter observer.jsonl -Recurse -ErrorAction SilentlyContinue)
                foreach ($log in $logs) {
                    if ([IO.File]::ReadAllText($log.FullName) -match '"event":"attached"') { return $true }
                }
                return $false
            })) { throw ($scenario + ': observer did not attach') }
            if ($scenario -eq 'clean') { [IO.File]::WriteAllText((Join-Path $root 'stop'), "stop`n", $utf8) }
            else { $child.Kill() }
            if (-not $child.WaitForExit(5000)) { throw ($scenario + ': child did not exit') }
        }
        if (-not $monitor.WaitForExit(25000)) { throw ($scenario + ': observer exceeded its deadline') }
        if ($monitor.ExitCode -ne 0) { throw ($scenario + ': observer error: ' + [IO.File]::ReadAllText((Join-Path $root 'observer.stderr'))) }
        $resultFile = @(Get-ChildItem -LiteralPath (Join-Path $root 'observations') -Filter result.json -Recurse)
        if ($resultFile.Count -ne 1) { throw ($scenario + ': expected one observation result') }
        $result = [IO.File]::ReadAllText($resultFile[0].FullName) | ConvertFrom-Json
        if ($scenario -eq 'clean') {
            if ($result.outcome -ne 'process_exited' -or $result.exit_code -ne 0) { throw 'Clean stop was not recorded as exit=0.' }
            if ([IO.File]::ReadAllText((Join-Path $root 'lifecycle.status')) -notmatch 'clean shutdown released instance lock') { throw 'Clean status missing.' }
        } elseif ($scenario -eq 'killed') {
            if ($result.outcome -ne 'process_exited' -or $result.exit_code -eq 0 -or $null -eq $result.exit_code) { throw 'Forced termination exit code was not captured.' }
            if (Test-Path -LiteralPath (Join-Path $root 'stop')) { throw 'Forced termination should have no stop marker.' }
        } elseif ($scenario -eq 'alive') {
            if ($result.outcome -ne 'alive_at_observation_deadline' -or $null -ne $result.exit_code -or $child.HasExited) { throw 'Live process was not observed at deadline.' }
            if (Test-Path -LiteralPath (Join-Path $root 'stop')) { throw 'Observer must not stop the live daemon.' }
            [IO.File]::WriteAllText((Join-Path $root 'stop'), "stop`n", $utf8)
            if (-not $child.WaitForExit(5000)) { throw 'Live control cleanup did not clean-stop.' }
        } elseif ($scenario -eq 'not_started') {
            if ($result.outcome -ne 'not_attached_before_timeout' -or $null -ne $result.exit_code) { throw 'Missing process incorrectly reported as alive or exited.' }
        } elseif ($scenario -eq 'pid_mismatch') {
            if ($result.outcome -ne 'pid_identity_mismatch' -or $null -ne $result.exit_code) { throw 'Unrelated PID was not rejected.' }
        }
        $results += [ordered]@{ scenario = $scenario; passed = $true; outcome = $result.outcome; exit_code = $result.exit_code; output_root = $result.output_root }
    } finally {
        foreach ($process in @($child, $monitor)) {
            if ($null -ne $process) {
                if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
                $process.Dispose()
            }
        }
    }
}
$summary = [ordered]@{ test_root = $testRoot; scenarios = $results; login_proof = 'NOT_TESTED_BY_CONTROLS' }
[IO.File]::WriteAllText((Join-Path $testRoot 'summary.json'), ($summary | ConvertTo-Json -Depth 6), $utf8)
$summary | ConvertTo-Json -Depth 6
