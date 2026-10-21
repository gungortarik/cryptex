param(
    [Parameter(Mandatory = $true)]
    [string]$Root
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ($env:OS -ne "Windows_NT") {
    throw "measure-windows-directory-allocation.ps1 must be run on Windows"
}

$ResolvedRoot = (Resolve-Path -LiteralPath $Root).Path
if (-not (Test-Path -LiteralPath $ResolvedRoot -PathType Container)) {
    throw "root is not a directory: $ResolvedRoot"
}

if (-not ([System.Management.Automation.PSTypeName]'Cryptex.Win32DirectoryUsageV1').Type) {
    Add-Type -TypeDefinition @"
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

namespace Cryptex
{
    [StructLayout(LayoutKind.Sequential)]
    public struct DirectoryFileStandardInfoV1
    {
        public long AllocationSize;
        public long EndOfFile;
        public uint NumberOfLinks;

        [MarshalAs(UnmanagedType.U1)]
        public bool DeletePending;

        [MarshalAs(UnmanagedType.U1)]
        public bool Directory;
    }

    public static class Win32DirectoryUsageV1
    {
        private const uint FILE_READ_ATTRIBUTES = 0x0080;
        private const uint FILE_SHARE_READ = 0x00000001;
        private const uint FILE_SHARE_WRITE = 0x00000002;
        private const uint FILE_SHARE_DELETE = 0x00000004;
        private const uint OPEN_EXISTING = 3;
        private const uint FILE_FLAG_BACKUP_SEMANTICS = 0x02000000;

        private enum FileInfoByHandleClass
        {
            FileBasicInfo = 0,
            FileStandardInfo = 1
        }

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern SafeFileHandle CreateFileW(
            string lpFileName,
            uint dwDesiredAccess,
            uint dwShareMode,
            IntPtr lpSecurityAttributes,
            uint dwCreationDisposition,
            uint dwFlagsAndAttributes,
            IntPtr hTemplateFile);

        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern bool GetFileInformationByHandleEx(
            SafeFileHandle hFile,
            FileInfoByHandleClass fileInformationClass,
            out DirectoryFileStandardInfoV1 fileInformation,
            uint bufferSize);

        public static long GetDirectoryAllocationSize(string path)
        {
            using (SafeFileHandle handle = CreateFileW(
                path,
                FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                IntPtr.Zero,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                IntPtr.Zero))
            {
                if (handle.IsInvalid)
                {
                    throw new Win32Exception(Marshal.GetLastWin32Error());
                }

                DirectoryFileStandardInfoV1 info;
                uint size = (uint)Marshal.SizeOf(typeof(DirectoryFileStandardInfoV1));

                if (!GetFileInformationByHandleEx(
                    handle,
                    FileInfoByHandleClass.FileStandardInfo,
                    out info,
                    size))
                {
                    throw new Win32Exception(Marshal.GetLastWin32Error());
                }

                if (!info.Directory)
                {
                    throw new InvalidOperationException("path is not reported as a directory: " + path);
                }

                return info.AllocationSize;
            }
        }
    }
}
"@
}

[uint64]$DirectoryCount = 0
[uint64]$AllocatedDirectoryBytes = 0

$Directories = @($ResolvedRoot)
$Directories += [System.IO.Directory]::EnumerateDirectories(
    $ResolvedRoot,
    "*",
    [System.IO.SearchOption]::AllDirectories)

foreach ($Directory in $Directories) {
    $Allocated = [Cryptex.Win32DirectoryUsageV1]::GetDirectoryAllocationSize($Directory)
    if ($Allocated -lt 0) {
        throw "negative directory allocation size returned for '$Directory': $Allocated"
    }

    $DirectoryCount++
    $AllocatedDirectoryBytes += [uint64]$Allocated
}

Write-Output "root=$ResolvedRoot"
Write-Output "directory_count=$DirectoryCount"
Write-Output "allocated_directory_bytes=$AllocatedDirectoryBytes"
Write-Output "measurement_method=CreateFileW(FILE_FLAG_BACKUP_SEMANTICS)+GetFileInformationByHandleEx(FileStandardInfo.AllocationSize)"
Write-Output "filesystem_global_metadata_included=false"
Write-Output "measurement_scope=directory_handle_allocation_only"
