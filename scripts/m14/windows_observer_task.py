"""Manage one expiring interactive logon observer in the disposable test account."""
import os
from pathlib import Path
import re
import subprocess
import sys


# Use the current interactive token. Task Scheduler starts pythonw directly,
# so an interpreter console cannot contaminate the observation.
TASK = r"""
$ErrorActionPreference = 'Stop'
$service = New-Object -ComObject Schedule.Service
$service.Connect()
$root = $service.GetFolder('\')
$sid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$name = $env:M14_OBSERVER_NAME
if ($env:M14_OBSERVER_OP -eq 'create') {
    $d = $service.NewTask(0)
    $d.RegistrationInfo.Description = 'Cryptex M14 temporary sign-in console measurement'
    $d.Principal.UserId = $sid
    $d.Principal.LogonType = 3
    $d.Principal.RunLevel = 0
    $d.Settings.DisallowStartIfOnBatteries = $false
    $d.Settings.StopIfGoingOnBatteries = $false
    $d.Settings.ExecutionTimeLimit = 'PT4M'
    $d.Settings.DeleteExpiredTaskAfter = 'PT1M'
    $t = $d.Triggers.Create(9)
    $t.UserId = $sid
    $t.Delay = 'PT0S'
    $t.EndBoundary = (Get-Date).AddDays(1).ToString('yyyy-MM-ddTHH:mm:ss')
    $a = $d.Actions.Create(0)
    $a.Path = $env:M14_OBSERVER_EXE
    $a.Arguments = $env:M14_OBSERVER_ARGS
    $null = $root.RegisterTaskDefinition($name, $d, 2, $sid, $null, 3)
} else {
    $task = $root.GetTask($name)
    $action = $task.Definition.Actions.Item(1)
    if ($task.Definition.Actions.Count -ne 1 -or $action.Path -ne $env:M14_OBSERVER_EXE -or
        $action.Arguments -cne $env:M14_OBSERVER_ARGS) {
        throw 'Observer registration changed.'
    }
    if ($env:M14_OBSERVER_OP -eq 'preflight') {
        $null = $task.Run($null)
    } elseif ($env:M14_OBSERVER_OP -eq 'status') {
        Write-Output ('Task state={0}, last result=0x{1:X8}' -f [int]$task.State, [int]$task.LastTaskResult)
    } else {
        $root.DeleteTask($name, 0)
    }
}
"""


def observer_task(operation, token, script):
    if operation not in ("create", "preflight", "status", "remove") or not re.fullmatch("[0-9a-f]{32}", token):
        raise ValueError("Invalid observer operation or test ID")
    if os.environ.get("USERNAME", "").lower() != "cryptexm14":
        raise RuntimeError("A temporary cryptexm14 account is required")
    exe = Path(sys.executable).with_name("pythonw.exe").resolve(strict=True)
    args = subprocess.list2cmdline(["-B", str(script.resolve(strict=True)), "observe", token])
    env = dict(os.environ, M14_OBSERVER_NAME="Cryptex-M14-Console-" + token,
               M14_OBSERVER_OP=operation, M14_OBSERVER_EXE=str(exe), M14_OBSERVER_ARGS=args)
    powershell = Path(os.environ["SystemRoot"]) / "System32/WindowsPowerShell/v1.0/powershell.exe"
    action = "try {\n" + TASK + "\n} catch { [Console]::Error.WriteLine($_.Exception.Message); exit 1 }"
    result = subprocess.run([powershell, "-NoProfile", "-NonInteractive", "-Command", action],
                            env=env, capture_output=True, timeout=30,
                            creationflags=subprocess.CREATE_NO_WINDOW)
    if result.returncode:
        raise RuntimeError(f"Task Scheduler ({operation}): " + result.stderr.decode(errors="replace").strip())
    return result.stdout.decode(errors="replace").strip()
