param(
    [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-f]{12}$')][string]$Run,
    [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-f]{32}$')][string]$Session
)
$ErrorActionPreference='Stop'
$root=Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\native-transfer\'+$Run)
if(-not (Test-Path -LiteralPath $root -PathType Container)){throw 'synthetic_run_missing'}
if((Get-Item -LiteralPath $root).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'run_link_refused'}
$relay=Get-Content -LiteralPath (Join-Path $root 'relay.json') -Raw | ConvertFrom-Json
if($relay.schema -ne 1 -or $relay.session -cne $Session){throw 'session_mismatch'}
$out=Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\native-diagnostic\'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $out | Out-Null
$inventory=New-Object Collections.Generic.List[object]
$pending=New-Object 'Collections.Generic.Stack[string]'
$pending.Push($root)
$count=0; $logs=0
while($pending.Count) {
    $dir=$pending.Pop()
    foreach($entry in Get-ChildItem -LiteralPath $dir -Force) {
        $count++; if($count -gt 4000){throw 'inventory_limit'}
        $relative=$entry.FullName.Substring($root.Length+1)
        if($entry.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'entry_link_refused'}
        if($entry.PSIsContainer) {
            if($dir -eq $root -and $entry.Name -in @('source','target','native-package','public-request')){continue}
            $pending.Push($entry.FullName)
        } else {
            $row=[ordered]@{path=$relative;bytes=$entry.Length}
            if($entry.Extension -in @('.seg','.open') -and $entry.Length -le 8MB) {
                $row.sha256=(Get-FileHash -LiteralPath $entry.FullName).Hash
            }
            $inventory.Add([pscustomobject]$row)
            # Native logs contain transport diagnostics. Never copy config, key,
            # certificate, database, canonical segment or import manifest bodies.
            if($entry.Name -ceq 'native.log') {
                $logs++; if($logs -gt 4 -or $entry.Length -gt 1MB){throw 'native_log_limit'}
                Copy-Item -LiteralPath $entry.FullName -Destination (Join-Path $out ('native-'+$logs+'.log'))
            }
        }
    }
}
$report=[ordered]@{schema=1;run=$Run;session=$Session;scope='read_only_synthetic_run_diagnostic';
    native_log_count=$logs;inventory=@($inventory.ToArray())}
[IO.File]::WriteAllText((Join-Path $out 'diagnostic.json'),($report|ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
foreach($name in @('runner-result.json','checkpoint.txt','native-test.log')) {
    $path=Join-Path $root $name
    if(Test-Path -LiteralPath $path -PathType Leaf) {
        if((Get-Item -LiteralPath $path).Length -gt 1MB){throw 'runner_log_limit'}
        Copy-Item -LiteralPath $path -Destination $out
    }
}
$zip=Join-Path $env:USERPROFILE ('Downloads\DENETLEYICI-TANI-'+$Run+'-'+[guid]::NewGuid().ToString('N').Substring(0,8)+'.zip')
Compress-Archive -Path (Join-Path $out '*') -DestinationPath $zip
Write-Host "TANI HAZIR: $zip"
Write-Host 'Bu ZIP dosyasini sohbete ekleyin. Test yeniden calistirilmadi.'
