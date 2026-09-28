$ErrorActionPreference = "Continue"
Set-StrictMode -Version 2.0

$script:PassCount = 0
$script:WarnCount = 0
$script:FailCount = 0

function Write-Pass { param([string]$Message) Write-Host "[PASS] $Message"; $script:PassCount++ }
function Write-Warn { param([string]$Message) Write-Host "[WARN] $Message"; $script:WarnCount++ }
function Write-Fail { param([string]$Message) Write-Host "[FAIL] $Message"; $script:FailCount++ }
function Write-Info { param([string]$Message) Write-Host "[INFO] $Message" }

function Test-CommandPresent {
    param([string]$Name)
    $cmd = Get-Command $Name -ErrorAction SilentlyContinue
    if ($null -ne $cmd) { Write-Pass "$Name found: $($cmd.Source)"; return $true }
    Write-Fail "$Name not found"; return $false
}

function Test-UninstallEntry {
    param([string]$Pattern)
    $paths = @(
        "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*",
        "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*",
        "HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*"
    )
    foreach ($path in $paths) {
        $entries = Get-ItemProperty $path -ErrorAction SilentlyContinue
        foreach ($entry in $entries) {
            $prop = $entry.PSObject.Properties["DisplayName"]
            if ($null -ne $prop) {
                $name = [string]$prop.Value
                if ($name -like $Pattern) { return $name }
            }
        }
    }
    return $null
}

Write-Host "Cryptex Windows Preflight"
Write-Host "========================="
Write-Host ""

Write-Host "Platform"
Write-Host "--------"
try {
    $os = Get-CimInstance Win32_OperatingSystem -ErrorAction Stop
    $build = [int]$os.BuildNumber
    Write-Info "OS: $($os.Caption)"
    Write-Info "Version: $($os.Version)"
    Write-Info "Build: $build"
    if ($build -ge 22000) { Write-Pass "Windows 11-class build detected" }
    else { Write-Fail "Windows 11 or newer is required by the readiness baseline" }

    $arch = [string]$os.OSArchitecture
    Write-Info "Architecture: $arch"
    if ($arch -match "64") { Write-Pass "Supported 64-bit CPU architecture detected" }
    else { Write-Warn "Unexpected CPU architecture: $arch" }
}
catch { Write-Fail "Unable to query Windows platform information: $($_.Exception.Message)" }

Write-Info "PowerShell: $($PSVersionTable.PSVersion)"

Write-Host ""
Write-Host "Developer toolchain"
Write-Host "-------------------"

$gitOk = Test-CommandPresent "git"
$ghOk = Test-CommandPresent "gh"
$nodeOk = Test-CommandPresent "node"
$pnpmOk = Test-CommandPresent "pnpm"
$rustcOk = Test-CommandPresent "rustc"
$cargoOk = Test-CommandPresent "cargo"
$rustupOk = Test-CommandPresent "rustup"

if ($gitOk)   { Write-Info (& git --version) }
if ($ghOk)    { Write-Info ((& gh --version | Select-Object -First 1)) }
if ($nodeOk)  { Write-Info "Node: $(& node --version)" }
if ($pnpmOk)  { Write-Info "pnpm: $(& pnpm --version)" }
if ($rustcOk) { Write-Info (& rustc --version) }
if ($cargoOk) { Write-Info (& cargo --version) }
if ($rustupOk) {
    $active = & rustup show active-toolchain 2>$null
    Write-Info "Rust toolchain: $active"
}
if ($ghOk) {
    $null = & gh auth status 2>&1
    $ghExit = $LASTEXITCODE
    if ($ghExit -eq 0) { Write-Pass "GitHub CLI is authenticated" }
    else { Write-Warn "GitHub CLI is installed but authentication was not verified" }
}
if ($gitOk) {
    $longPaths = & git config --global --get core.longpaths 2>$null
    if ($longPaths -eq "true") { Write-Pass "Git core.longpaths is enabled" }
    else { Write-Warn "Git core.longpaths is not enabled" }
}

Write-Host ""
Write-Host "Windows build prerequisites"
Write-Host "---------------------------"

$vswhereCandidates = @(
    "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe",
    "$env:ProgramFiles\Microsoft Visual Studio\Installer\vswhere.exe"
)
$vswhere = $vswhereCandidates | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if ($null -ne $vswhere) {
    $vsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($vsPath) { Write-Pass "Visual Studio C++ Build Tools found: $vsPath" }
    else { Write-Fail "Visual Studio installation exists but C++ build tools were not verified" }
}
else { Write-Fail "Visual Studio Build Tools / vswhere not found" }

$webViewVersion = $null
$webViewExe = $null
$webViewRoots = @(
    "${env:ProgramFiles(x86)}\Microsoft\EdgeWebView\Application",
    "$env:ProgramFiles\Microsoft\EdgeWebView\Application",
    "$env:LOCALAPPDATA\Microsoft\EdgeWebView\Application"
)
foreach ($root in $webViewRoots) {
    if ($root -and (Test-Path $root)) {
        $candidate = Get-ChildItem -Path $root -Directory -ErrorAction SilentlyContinue |
            Sort-Object Name -Descending |
            ForEach-Object {
                $exe = Join-Path $_.FullName "msedgewebview2.exe"
                if (Test-Path $exe) {
                    [PSCustomObject]@{ Version = $_.Name; Exe = $exe }
                }
            } | Select-Object -First 1
        if ($null -ne $candidate) {
            $webViewVersion = $candidate.Version
            $webViewExe = $candidate.Exe
            break
        }
    }
}
if ($null -ne $webViewVersion) {
    Write-Pass "WebView2 Runtime found"
    Write-Info "WebView2 version: $webViewVersion"
    Write-Info "WebView2 executable: $webViewExe"
}
else { Write-Fail "WebView2 Runtime was not verified" }

Write-Host ""
Write-Host "Desktop prerequisites"
Write-Host "---------------------"

$cursorCandidates = @(
    "$env:LOCALAPPDATA\Programs\cursor\Cursor.exe",
    "$env:LOCALAPPDATA\Programs\Cursor\Cursor.exe",
    "$env:ProgramFiles\Cursor\Cursor.exe"
)
$cursorFound = $cursorCandidates | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if ($cursorFound -or (Get-Command cursor -ErrorAction SilentlyContinue)) { Write-Pass "Cursor found" }
else { Write-Fail "Cursor not found" }

$obsidianCandidates = @(
    "$env:LOCALAPPDATA\Obsidian\Obsidian.exe",
    "$env:LOCALAPPDATA\Programs\Obsidian\Obsidian.exe"
)
$obsidianFound = $obsidianCandidates | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if ($obsidianFound -or (Test-UninstallEntry "Obsidian*")) { Write-Pass "Obsidian found" }
else { Write-Fail "Obsidian not found" }

$syncthingCommand = Get-Command syncthing -ErrorAction SilentlyContinue
$syncthingCandidates = @(
    "$env:LOCALAPPDATA\Microsoft\WinGet\Links\syncthing.exe",
    "$env:ProgramFiles\Syncthing\syncthing.exe",
    "${env:ProgramFiles(x86)}\Syncthing\syncthing.exe",
    "$env:LOCALAPPDATA\Programs\Syncthing\syncthing.exe"
)
$syncthingExe = $syncthingCandidates | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
$syncthingInstalled = Test-UninstallEntry "Syncthing*"
if ($syncthingCommand) {
    Write-Pass "Syncthing found: $($syncthingCommand.Source)"
    Write-Info ((& syncthing --version 2>$null | Select-Object -First 1))
}
elseif ($syncthingExe) { Write-Pass "Syncthing executable found: $syncthingExe" }
elseif ($syncthingInstalled) { Write-Pass "Syncthing installation found: $syncthingInstalled" }
else { Write-Fail "Syncthing baseline not found" }

Write-Host ""
Write-Host "Device protection"
Write-Host "-----------------"

if (Get-Command Get-BitLockerVolume -ErrorAction SilentlyContinue) {
    try {
        $bitlocker = Get-BitLockerVolume -MountPoint "C:" -ErrorAction Stop
        Write-Info "BitLocker protection status: $($bitlocker.ProtectionStatus)"
        if ($bitlocker.ProtectionStatus -eq "On") { Write-Pass "BitLocker protection is enabled on C:" }
        else { Write-Warn "BitLocker protection is not enabled on C:" }
    }
    catch { Write-Warn "Unable to query BitLocker status" }
}
else { Write-Warn "BitLocker status command unavailable" }

Write-Host ""
Write-Host "Filesystem"
Write-Host "----------"

if ($env:LOCALAPPDATA -and (Test-Path $env:LOCALAPPDATA)) {
    $testFile = Join-Path $env:LOCALAPPDATA "cryptex-preflight-$PID.tmp"
    try {
        Set-Content -Path $testFile -Value "ok" -Encoding ASCII -ErrorAction Stop
        Remove-Item $testFile -Force -ErrorAction Stop
        Write-Pass "LOCALAPPDATA is writable"
    }
    catch { Write-Fail "LOCALAPPDATA write test failed" }
}
else { Write-Fail "LOCALAPPDATA was not found" }

Write-Host ""
Write-Host "Repository"
Write-Host "----------"

if ($gitOk) {
    & git rev-parse --is-inside-work-tree *> $null
    if ($LASTEXITCODE -eq 0) {
        $repoRoot = & git rev-parse --show-toplevel
        Write-Pass "Git repository detected: $repoRoot"

        if ((Test-Path (Join-Path $repoRoot "AGENTS.md")) -and
            (Test-Path (Join-Path $repoRoot "docs\00_MASTER_SPEC.md"))) {
            Write-Pass "Cryptex authority files found"
        }
        else { Write-Fail "Expected Cryptex authority files not found" }

        $origin = & git remote get-url origin 2>$null
        if ($origin -like "*gungortarik/cryptex*") { Write-Pass "Expected Git origin configured" }
        else { Write-Warn "Unexpected or missing Git origin: $origin" }
    }
    else { Write-Warn "Script was not run from inside a Git repository" }
}

Write-Host ""
Write-Host "Summary"
Write-Host "-------"
Write-Host "PASS: $script:PassCount"
Write-Host "WARN: $script:WarnCount"
Write-Host "FAIL: $script:FailCount"

if ($script:FailCount -gt 0) { exit 1 }
exit 0
