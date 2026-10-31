$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$MarkerPath = Join-Path $PSScriptRoot 'run-dispatched.json'
$record = [ordered]@{
    pid = $PID
    launched_utc = [DateTime]::UtcNow.ToString('o')
    session_id = [Diagnostics.Process]::GetCurrentProcess().SessionId
}
$parent = Split-Path -Parent $MarkerPath
[void][IO.Directory]::CreateDirectory($parent)
$tempPath = $MarkerPath + '.tmp'
[IO.File]::WriteAllText($tempPath, ($record | ConvertTo-Json), (New-Object Text.UTF8Encoding($false)))
Move-Item -LiteralPath $tempPath -Destination $MarkerPath -Force
