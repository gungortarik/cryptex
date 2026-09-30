param(
    [Parameter(Mandatory = $true)]
    [string]$Root
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not $IsWindows) {
    throw "measure-windows-allocation.ps1 must be run on Windows"
}

$ResolvedRoot = (Resolve-Path -LiteralPath $Root).Path
if (-not (Test-Path -LiteralPath $ResolvedRoot -PathType Container)) {
    throw "root is not a directory: $ResolvedRoot"
}

if (-not ([System.Management.Automation.PSTypeName]'Cryptex.Win32DiskUsage').Type) {
    Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

namespace Cryptex
{
    public static class Win32DiskUsage
    {
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern uint GetCompressedFileSizeW(
            string lpFileName,
            out uint lpFileSizeHigh);
    }
}
"@
}

function Get-AllocatedFileBytes {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    [uint32]$High = 0
    [uint32]$Low = [Cryptex.Win32DiskUsage]::GetCompressedFileSizeW($Path, [ref]$High)
    $ErrorCode = [Runtime.InteropServices.Marshal]::GetLastWin32Error()

    if (($Low -eq [uint32]::MaxValue) -and ($ErrorCode -ne 0)) {
        throw "GetCompressedFileSizeW failed for '$Path' with Win32 error $ErrorCode"
    }

    return ([uint64]$High * [uint64]4294967296) + [uint64]$Low
}

[uint64]$FileCount = 0
[uint64]$DirectoryCount = 1
[uint64]$LogicalBytes = 0
[uint64]$AllocatedFileBytes = 0

foreach ($Directory in [System.IO.Directory]::EnumerateDirectories(
    $ResolvedRoot,
    "*",
    [System.IO.SearchOption]::AllDirectories)) {
    $DirectoryCount++
}

foreach ($Path in [System.IO.Directory]::EnumerateFiles(
    $ResolvedRoot,
    "*",
    [System.IO.SearchOption]::AllDirectories)) {
    $Info = [System.IO.FileInfo]::new($Path)
    $FileCount++
    $LogicalBytes += [uint64]$Info.Length
    $AllocatedFileBytes += Get-AllocatedFileBytes -Path $Path
}

$AllocationDeltaBytes = [decimal]$AllocatedFileBytes - [decimal]$LogicalBytes
$AllocationRatio = if ($LogicalBytes -eq 0) {
    0
}
else {
    [math]::Round(([double]$AllocatedFileBytes / [double]$LogicalBytes), 6)
}

Write-Output "root=$ResolvedRoot"
Write-Output "file_count=$FileCount"
Write-Output "directory_count=$DirectoryCount"
Write-Output "logical_bytes=$LogicalBytes"
Write-Output "allocated_file_bytes=$AllocatedFileBytes"
Write-Output "file_allocation_delta_bytes=$AllocationDeltaBytes"
Write-Output "file_allocation_ratio=$AllocationRatio"
Write-Output "measurement_method=GetCompressedFileSizeW"
Write-Output "directory_metadata_included=false"
Write-Output "measurement_scope=file_allocation_only"
