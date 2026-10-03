# Read-only package identity diagnostic. Dot-source before startup experiments.
if (-not ('CryptexWindowsProcessContext' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
public static class CryptexWindowsProcessContext {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetCurrentPackageFullName(ref uint length, StringBuilder name);
    public static string PackageName() {
        uint length = 0;
        int result = GetCurrentPackageFullName(ref length, null);
        if (result == 15700) return null; // APPMODEL_ERROR_NO_PACKAGE
        if (result != 122) throw new Win32Exception(result, "Cannot query process package identity.");
        StringBuilder name = new StringBuilder(checked((int)length));
        result = GetCurrentPackageFullName(ref length, name);
        if (result != 0) throw new Win32Exception(result, "Cannot read process package identity.");
        return name.ToString();
    }
}
'@
}
function Get-WindowsProcessContext {
    $packageName = [CryptexWindowsProcessContext]::PackageName()
    $ancestors = @()
    $walkPid = $PID
    for ($index = 0; $index -lt 6 -and $walkPid -gt 0; $index++) {
        $process = Get-CimInstance Win32_Process -Filter "ProcessId=$walkPid" -ErrorAction Stop
        if ($null -eq $process) { break }
        $ancestors += [ordered]@{
            name = $process.Name; pid = $process.ProcessId
            parent_pid = $process.ParentProcessId; executable = $process.ExecutablePath
        }
        $walkPid = $process.ParentProcessId
    }
    [ordered]@{
        package_full_name = $packageName
        packaged_process = ($null -ne $packageName)
        ancestors = $ancestors
    }
}
