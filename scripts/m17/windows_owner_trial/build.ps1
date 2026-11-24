param([Parameter(Mandatory)][string]$InputFolder, [Parameter(Mandatory)][string]$Destination)
$ErrorActionPreference = 'Stop'
$source = Get-Item -LiteralPath (Join-Path $InputFolder 'setup.exe')
$manifest = Get-Item -LiteralPath (Join-Path $InputFolder 'build.json')
$build = Get-Content -LiteralPath $manifest.FullName -Raw | ConvertFrom-Json
if ($build.revision -cnotmatch '^[a-f0-9]{40}$' -or $build.installer -cne (Get-FileHash $source.FullName).Hash -or
    $build.desktop -cnotmatch '^[A-F0-9]{64}$' -or $build.daemon -cnotmatch '^[A-F0-9]{64}$') {
    throw 'Trial input manifest mismatch'
}
$out = [IO.Path]::GetFullPath($Destination)
if (Test-Path -LiteralPath $out) { throw 'Choose a new launcher output' }
$ref = @(Get-ChildItem "$env:WINDIR/Microsoft.NET/assembly/GAC_MSIL/System.Management.Automation/*/System.Management.Automation.dll")
if ($ref.Count -ne 1) { throw 'Expected one Windows PowerShell assembly' }
$compiler = "$env:WINDIR/Microsoft.NET/Framework64/v4.0.30319/csc.exe"
& $compiler /nologo /target:exe /platform:x64 /optimize+ "/r:$($ref[0].FullName)" /r:System.Web.Extensions.dll "/out:$out" `
    "/resource:$($manifest.FullName),build.json" "/resource:$($source.FullName),setup" (Join-Path $PSScriptRoot 'Install.cs')
if ($LASTEXITCODE -or !(Test-Path -LiteralPath $out)) { throw 'Could not build owner launcher' }
