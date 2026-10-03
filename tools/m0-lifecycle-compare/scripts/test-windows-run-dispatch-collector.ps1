$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$collector = Join-Path $PSScriptRoot 'collect-windows-run-dispatch.ps1'
$powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
$fixture = Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\p0-2\dispatch-collector-test-' + [Guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($fixture)
$utf8 = [Text.UTF8Encoding]::new($false)
$script = Join-Path $fixture 'write-marker.ps1'
$marker = Join-Path $fixture 'run-dispatched.json'
$manifestPath = Join-Path $fixture 'run-dispatch.json'
[IO.File]::WriteAllText($script, '# Non-executed synthetic fixture', $utf8)
$manifest = [ordered]@{
    session_root = $fixture; marker_script = $script; marker_path = $marker
    run_value = 'CryptexDevP02RunDispatch'; run_command = 'synthetic fixture; never registered'
    marker_script_sha256 = (Get-FileHash -LiteralPath $script -Algorithm SHA256).Hash
    prepared_utc = [DateTime]::UtcNow.ToString('o')
}
[IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json), $utf8)
$key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Run', $false)
try {
    $before = $key.GetValue('CryptexDevP02RunDispatch', $null)
    $result = & $powershell -NoProfile -NonInteractive -File $collector -SessionRoot $fixture | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or $result.marker_present -or $result.classification -ne 'DISPATCH_NOT_OBSERVED_CAUSE_OPEN') {
        throw 'Absent marker was misclassified.'
    }
    $report = Get-Content -LiteralPath $result.evidence_path -Raw | ConvertFrom-Json
    if (-not $report.marker_script_hash_matches -or $report.registry.Count -ne 2 -or $report.observer_user_sid -ne [Security.Principal.WindowsIdentity]::GetCurrent().User.Value) {
        throw 'Identity, hash or registry-view evidence is incorrect.'
    }
    [IO.File]::WriteAllText($marker, '{"pid":1,"session_id":0,"launched_utc":"2000-01-01T00:00:00Z"}', $utf8)
    [IO.File]::WriteAllText($script, '# Changed synthetic fixture', $utf8)
    $second = & $powershell -NoProfile -NonInteractive -File $collector -SessionRoot $fixture | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or -not $second.marker_present -or $second.classification -ne 'MARKER_PRESENT_REQUIRES_LOGIN_CORRELATION' -or $second.evidence_path -eq $result.evidence_path) {
        throw 'Marker presence must require login correlation and preserve earlier observations.'
    }
    $secondReport = Get-Content -LiteralPath $second.evidence_path -Raw | ConvertFrom-Json
    if ($secondReport.marker_script_hash_matches) { throw 'Modified marker script was not detected.' }
    $manifest.marker_script = Join-Path $fixture 'unrelated.ps1'
    [IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json), $utf8)
    try {
        $ErrorActionPreference = 'Continue'
        $ignored = & $powershell -NoProfile -NonInteractive -File $collector -SessionRoot $fixture 2>&1
    } finally { $ErrorActionPreference = 'Stop' }
    if ($LASTEXITCODE -eq 0) { throw 'Mismatched manifest was accepted.' }
    if (@(Get-ChildItem -LiteralPath (Join-Path $fixture 'dispatch-observations') -Filter '*.json').Count -ne 2) {
        throw 'Invalid manifest produced an observation.'
    }
    try {
        $ErrorActionPreference = 'Continue'
        $ignored = & $powershell -NoProfile -NonInteractive -File $collector -SessionRoot $env:TEMP 2>&1
    } finally { $ErrorActionPreference = 'Stop' }
    if ($LASTEXITCODE -eq 0) { throw 'Outside runtime root was accepted.' }
    if ($key.GetValue('CryptexDevP02RunDispatch', $null) -ne $before) { throw 'Collector changed startup registration.' }
} finally { $key.Dispose() }
[ordered]@{ result = 'PASS_DISPATCH_COLLECTOR_CONTROLS'; runtime_root = $fixture; controls = 5; startup_registration_unchanged = $true } | ConvertTo-Json
