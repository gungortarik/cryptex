$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSVersion.Major -ne 5) { throw 'Run this regression with Windows PowerShell 5.1' }
$tokens = $null
$parseErrors = $null
$ast = [Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'verify-managed-profile-windows.ps1'), [ref]$tokens, [ref]$parseErrors)
if ($parseErrors.Count -ne 0) { throw 'Runner parser failure' }
$definition = $ast.Find({param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Invoke-CargoStep'}, $true)
if ($null -eq $definition) { throw 'Runner function missing' }
# Extract only the actual command helper; never execute the runner's batch or
# source-manifest/setup code in this regression. Mock only executable lookup.
. ([ScriptBlock]::Create($definition.Extent.Text))
$proof = Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\m8-verification\ps51-runner-regression-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $proof | Out-Null
$utf8 = New-Object Text.UTF8Encoding($false)
$results = New-Object 'System.Collections.Generic.List[object]'
$script:mockCargo = Join-Path $env:SYSTEMROOT 'System32\WindowsPowerShell\v1.0\powershell.exe'
function Get-Command { param($Name, $CommandType, $ErrorAction) [pscustomobject]@{ Source=$script:mockCargo } }
Invoke-CargoStep 'zero-stderr' @('-NoProfile','-Command',"[Console]::Error.WriteLine('Downloading synthetic crates ...'); exit 0")
if ($ErrorActionPreference -ne 'Stop') { throw 'Preference not restored after success' }
$rejected = $false
try {
    Invoke-CargoStep 'nonzero-stderr' @('-NoProfile','-Command',"[Console]::Error.WriteLine('Synthetic failure'); exit 7")
} catch {
    if ($_.Exception.Message -notlike 'Verification stopped at nonzero-stderr*') { throw }
    $rejected = $true
}
if (-not $rejected -or $results[0].exit -ne 0 -or $results[1].exit -ne 7) { throw 'Exit status not preserved' }
if ($ErrorActionPreference -ne 'Stop') { throw 'Preference not restored after nonzero exit' }
$script:mockCargo = Join-Path $proof 'does-not-exist.exe'
$rejected = $false
try { Invoke-CargoStep 'launch-failure' @('--version') } catch { $rejected = $true }
if (-not $rejected) { throw 'Launch failure became PASS' }
if ($results.Count -gt 2 -and $results[2].exit -eq 0) { throw 'Launch failure recorded success' }
if ($ErrorActionPreference -ne 'Stop') { throw 'Preference not restored after launch failure' }
if ((Get-Content -Raw (Join-Path $proof 'zero-stderr.log')) -notmatch 'Downloading synthetic crates') { throw 'Stderr log missing' }
Write-Output ('PASS 3/3: stderr with exit 0; actual exit 7; launch failure. Proof: ' + $proof)
