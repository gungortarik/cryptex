param(
    [Parameter(Mandatory = $true)]
    [string]$Root
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# $IsWindows exists in PowerShell Core, but not in Windows PowerShell 5.1.
# Use the long-standing Windows_NT environment marker so this helper works
# in both shells under StrictMode.
if ($env:OS -ne "Windows_NT") {
    throw "measure-windows-allocation.ps1 must be run on Windows"
}

$ResolvedRoot = (Resolve-Path -LiteralPath $Root).Path
if (-not (Test-Path -LiteralPath $ResolvedRoot -PathType Container)) {
    throw "root is not a directory: $ResolvedRoot"
}

if (-not ([System.Management.Automation.PSTypeName]'Cryptex.Win32DiskUsage').Type) {
    Add-Type -TypeDefinition @"
using System;
using System.IO;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

namespace Cryptex
{
    [StructLayout(LayoutKind.Sequential)]
    public struct FileStandardInfo
    {
        public long AllocationSize;
        public long EndOfFile;
        public uint NumberOfLinks;

        [MarshalAs(UnmanagedType.U1)]
        public bool DeletePending;

        [MarshalAs(UnmanagedType.U1)]
        public bool Directory;
    }

    public static class Win32DiskUsage
    {
        private enum FileInfoByHandleClass
        {
            FileBasicInfo = 0,
            FileStandardInfo = 1
        }

        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern bool GetFileInformationByHandleEx(
            SafeFileHandle hFile,
            FileInfoByHandleClass fileInformationClass,
            out FileStandardInfo fileInformation,
            uint bufferSize);

        public static long GetAllocationSize(string path)
        {
            using (FileStream stream = new FileStream(
                path,
                FileMode.Open,
                FileAccess.Read,
                FileShare.ReadWrite | FileShare.Delete))
            {
                FileStandardInfo info;
                uint size = (uint)Marshal.SizeOf(typeof(FileStandardInfo));

                if (!GetFileInformationByHandleEx(
                    stream.SafeFileHandle,
                    FileInfoByHandleClass.FileStandardInfo,
                    out info,
                    size))
                {
                    throw new System.ComponentModel.Win32Exception(
                        Marshal.GetLastWin32Error());
                }

                return info.AllocationSize;
            }
        }
    }
}
"@
}

function Get-AllocatedFileBytes {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $Allocated = [Cryptex.Win32DiskUsage]::GetAllocationSize($Path)
    if ($Allocated -lt 0) {
        throw "negative allocation size returned for '$Path': $Allocated"
    }

    return [uint64]$Allocated
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
Write-Output "measurement_method=GetFileInformationByHandleEx(FileStandardInfo.AllocationSize)"
Write-Output "directory_metadata_included=false"
Write-Output "measurement_scope=file_allocation_only"
