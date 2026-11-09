param(
    [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-f]{12}$')][string]$Run,
    [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-f]{32}$')][string]$Session
)
$ErrorActionPreference='Stop'
# Cancel only the known synthetic runner, through its existing cancellation flag.
# Never stop arbitrary processes, delete profiles, or alter signed messages.
$root=Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\native-transfer\'+$Run)
if(-not (Test-Path -LiteralPath $root -PathType Container)){exit 0}
if((Get-Item -LiteralPath $root).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'unexpected_run_link'}
$relay=Get-Content -LiteralPath (Join-Path $root 'relay.json') -Raw | ConvertFrom-Json
if($relay.schema -ne 1 -or $relay.session -cne $Session){throw 'previous_run_session_mismatch'}
$result=Join-Path $root 'runner-result.json'
if(Test-Path -LiteralPath $result -PathType Leaf){exit 0}
$checkpoint=(Get-Content -LiteralPath (Join-Path $root 'checkpoint.txt') -Raw).Trim()
if($checkpoint -cne 'waiting_for_host_delivery'){throw 'previous_run_not_waiting'}
[IO.File]::WriteAllText((Join-Path $root 'cancel.request'),'cancel',[Text.UTF8Encoding]::new($false))
Write-Host 'Onceki testin kontrollu kapanmasi bekleniyor...'
$deadline=[DateTime]::UtcNow.AddSeconds(55)
while([DateTime]::UtcNow -lt $deadline) {
    if(Test-Path -LiteralPath $result -PathType Leaf){exit 0}
    Start-Sleep -Milliseconds 250
}
throw 'previous_run_cleanup_not_observed'
