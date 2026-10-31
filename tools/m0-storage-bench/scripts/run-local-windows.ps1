param(
    [Parameter(Mandatory = $true)]
    [ValidateSet(10000, 100000, 500000)]
    [int]$Events,

    [Parameter(Mandatory = $true)]
    [ValidateRange(1, 2147483647)]
    [int]$PayloadBytes,

    [Parameter(Mandatory = $true)]
    [ValidateRange(1, 2147483647)]
    [int]$SegmentEvents
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")).Path
$RuntimeRoot = Join-Path $env:LOCALAPPDATA "Cryptex-Dev\benchmarks\p0-1"
$Stamp = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
$RunRoot = Join-Path $RuntimeRoot "windows-$Events-$Stamp"
$ResultsDir = Join-Path $RunRoot "results"
$DataDir = Join-Path $RunRoot "data"

New-Item -ItemType Directory -Force -Path $ResultsDir | Out-Null
New-Item -ItemType Directory -Force -Path $DataDir | Out-Null

Push-Location $RepoRoot
try {
    $Dirty = @(git status --porcelain)
    if ($Dirty.Count -ne 0) {
        throw "benchmark requires a clean Git worktree"
    }

    & cargo build --locked --release -p m0-storage-bench
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }

    $Bench = Join-Path $RepoRoot "target\release\m0-storage-bench.exe"
    if (-not (Test-Path -LiteralPath $Bench -PathType Leaf)) {
        throw "benchmark binary not found: $Bench"
    }

    try {
        $SyncthingVersion = (& syncthing --version 2>$null | Select-Object -First 1)
        if ([string]::IsNullOrWhiteSpace($SyncthingVersion)) {
            $SyncthingVersion = "unavailable"
        }
    }
    catch {
        $SyncthingVersion = "unavailable"
    }

    @(
        "platform=windows"
        "events=$Events"
        "payload_bytes=$PayloadBytes"
        "segment_events=$SegmentEvents"
        "git_commit=$(git rev-parse HEAD)"
        "git_branch=$(git rev-parse --abbrev-ref HEAD)"
        "rustc=$(rustc --version)"
        "cargo=$(cargo --version)"
        "os_version=$([Environment]::OSVersion.VersionString)"
        "arch=$env:PROCESSOR_ARCHITECTURE"
        "syncthing_version=$SyncthingVersion"
        "run_root=$RunRoot"
    ) | Set-Content -LiteralPath (Join-Path $ResultsDir "metadata.txt") -Encoding utf8

    function Invoke-Bench {
        param(
            [Parameter(Mandatory = $true)]
            [string[]]$BenchArgs,

            [Parameter(Mandatory = $true)]
            [string]$OutputPath
        )

        $Output = & $Bench @BenchArgs 2>&1
        $ExitCode = $LASTEXITCODE
        $Output | Tee-Object -FilePath $OutputPath
        if ($ExitCode -ne 0) {
            throw "m0-storage-bench failed with exit code $ExitCode"
        }
    }

    foreach ($Layout in @("files", "segments")) {
        $Root = Join-Path $DataDir "$Layout-$Events"
        $OutputPath = Join-Path $ResultsDir "$Layout-$Events.txt"

        if ($Layout -eq "files") {
            Invoke-Bench -BenchArgs @("generate", "files", "$Events", "$PayloadBytes", $Root) -OutputPath $OutputPath
        }
        else {
            Invoke-Bench -BenchArgs @("generate", "segments", "$Events", "$PayloadBytes", $Root, "$SegmentEvents") -OutputPath $OutputPath
        }

        Invoke-Bench -BenchArgs @("scan", $Layout, $Root) -OutputPath (Join-Path $ResultsDir "$Layout-$Events-rescan.txt")
    }

    Invoke-Bench -BenchArgs @("crash-probe", "files", "$PayloadBytes", (Join-Path $DataDir "crash-files")) -OutputPath (Join-Path $ResultsDir "crash-files.txt")
    Invoke-Bench -BenchArgs @("crash-probe", "segments", "$PayloadBytes", (Join-Path $DataDir "crash-segments")) -OutputPath (Join-Path $ResultsDir "crash-segments.txt")

    Write-Host "P0-1 local Windows run complete."
    Write-Host "Results: $ResultsDir"
    Write-Host "Data:    $DataDir"
    Write-Host ""
    Write-Host "Do not delete the data yet if it will be used for the Syncthing phase."
    Write-Host "This run is evidence only; it does not select the production layout."
}
finally {
    Pop-Location
}
