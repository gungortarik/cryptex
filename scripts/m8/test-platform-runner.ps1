$ErrorActionPreference='Stop'
$tokens=$null; $parseErrors=$null
$ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'WIN-TEST.ps1'),[ref]$tokens,[ref]$parseErrors)
if($parseErrors.Count -ne 0){throw 'Runner parse failure'}
# Load only helpers: never execute the package runner or access owner profiles.
foreach($name in @('Test-GroupSummary','Get-TestFileSystem','Invoke-TestProcess')){
    $definition=$ast.Find({param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq $name},$true)
    if($null -eq $definition){throw ('Missing helper: '+$name)}
    . ([scriptblock]::Create($definition.Extent.Text))
}
$good='test result: ok. 5 passed; 0 failed; 0 ignored; 578 filtered out; finished in 0.10s'
if(-not (Test-GroupSummary 0 @($good) 5)){throw 'Valid summary rejected'}
foreach($case in @(
    @{code=7;lines=@($good);count=5},
    @{code=0;lines=@();count=5},
    @{code=0;lines=@($good,$good);count=5},
    @{code=0;lines=@($good);count=6},
    @{code=0;lines=@('test result: ok. 0 passed; 0 failed; 0 ignored;');count=5},
    @{code=0;lines=@('test result: ok. 5 passed; 0 failed; 1 ignored;');count=5},
    @{code=0;lines=@('test result: FAILED. 5 passed; 1 failed; 0 ignored;');count=5}
)){
    if(Test-GroupSummary $case.code $case.lines $case.count){throw 'False PASS accepted'}
}
$work=Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\platform-runner-test\'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
$utf8=New-Object Text.UTF8Encoding($false)
$exe=Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
$log=Join-Path $work 'process.log'
$code=Invoke-TestProcess $exe '-NoProfile -Command "[Console]::Out.WriteLine(''out''); [Console]::Error.WriteLine(''err''); exit 7"' $log $work
if($code -ne 7 -or (Get-Content $log -Raw) -notmatch '(?s)out.*err'){throw 'Process exit/log capture failed'}
$failed=$false
try { Invoke-TestProcess (Join-Path $work 'missing.exe') '' $log $work | Out-Null } catch { $failed=$true }
if(-not $failed){throw 'Launch failure was swallowed'}
if((Get-TestFileSystem $work) -notmatch '^[A-Za-z0-9]+$'){throw 'Filesystem observation malformed'}
Write-Output 'PASS: exact counts, zero/ignored/duplicate/missing/nonzero failures, process logs and launch failure'
