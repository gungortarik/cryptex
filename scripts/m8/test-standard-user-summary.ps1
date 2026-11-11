$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$errors=$null; $tokens=$null
$ast=[Management.Automation.Language.Parser]::ParseFile(
    (Join-Path $PSScriptRoot 'standard-user-daemon-child.ps1'),[ref]$tokens,[ref]$errors)
if($errors.Count){throw 'Child script syntax invalid.'}
$definitions=@($ast.FindAll({param($node)
    $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Assert-CompleteSuite'
},$true))
if($definitions.Count -ne 1){throw 'Expected one suite validator.'}
. ([ScriptBlock]::Create($definitions[0].Extent.Text))
$full='test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1s'
$scoped='test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 600 filtered out; finished in 1s'
Assert-CompleteSuite 0 $full 9 $true
Assert-CompleteSuite 0 $scoped 3 $true 600
Assert-CompleteSuite 0 'test result: ok. 8 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out;' 9 $false
$refusals=@(
    @{code=0; text=$scoped; count=3; noIgnored=$true; filtered=599},
    @{code=0; text=$scoped; count=4; noIgnored=$true; filtered=600},
    @{code=1; text=$scoped; count=3; noIgnored=$true; filtered=600},
    @{code=0; text='test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 600 filtered out;'; count=3; noIgnored=$true; filtered=600},
    @{code=0; text='test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 603 filtered out;'; count=0; noIgnored=$true; filtered=603},
    @{code=0; text='test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 600 filtered out;'; count=3; noIgnored=$true; filtered=600}
)
foreach($case in $refusals) {
    $rejected=$false
    try {Assert-CompleteSuite $case.code $case.text $case.count $case.noIgnored $case.filtered}
    catch {$rejected=$true}
    if(-not $rejected){throw 'Invalid suite summary was accepted.'}
}
'Suite validator: 3 positive and 6 refusal cases passed.'
