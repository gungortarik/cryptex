"""Observe the installed GUI during Windows alternate-shell logon processing."""
import ctypes as c
from ctypes import wintypes as w
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
import winreg

from login import FLAG, check_entry, registration

USER = c.WinDLL("user32", use_last_error=True)
KERNEL = c.WinDLL("kernel32", use_last_error=True)
ADV = c.WinDLL("advapi32", use_last_error=True)
CALLBACK = c.WINFUNCTYPE(None, w.HANDLE, w.DWORD, w.HWND, w.LONG, w.LONG, w.DWORD, w.DWORD)
SHOW, CONSOLE_START = 0x8002, 0x4006
RUN = r"Software\Microsoft\Windows\CurrentVersion\Run"


def api(dll, name, result, *args):
    fn = getattr(dll, name)
    fn.argtypes, fn.restype = args, result
    return fn


hook_event = api(USER, "SetWinEventHook", w.HANDLE, w.DWORD, w.DWORD, w.HMODULE, CALLBACK, w.DWORD, w.DWORD, w.DWORD)
unhook = api(USER, "UnhookWinEvent", w.BOOL, w.HANDLE)
get_class = api(USER, "GetClassNameW", c.c_int, w.HWND, w.LPWSTR, c.c_int)
peek = api(USER, "PeekMessageW", w.BOOL, c.POINTER(w.MSG), w.HWND, w.UINT, w.UINT, w.UINT)
dispatch = api(USER, "DispatchMessageW", c.c_ssize_t, c.POINTER(w.MSG))
open_process = api(KERNEL, "OpenProcess", w.HANDLE, w.DWORD, w.BOOL, w.DWORD)
close = api(KERNEL, "CloseHandle", w.BOOL, w.HANDLE)
query_image = api(KERNEL, "QueryFullProcessImageNameW", w.BOOL, w.HANDLE, w.DWORD, w.LPWSTR, c.POINTER(w.DWORD))
open_token = api(ADV, "OpenProcessToken", w.BOOL, w.HANDLE, w.DWORD, c.POINTER(w.HANDLE))
token_info = api(ADV, "GetTokenInformation", w.BOOL, w.HANDLE, c.c_int, c.c_void_p, w.DWORD, c.POINTER(w.DWORD))


def identity(pid):
    process = open_process(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
    if not process:
        raise c.WinError(c.get_last_error())
    token = w.HANDLE()
    try:
        name = c.create_unicode_buffer(32768)
        size = w.DWORD(len(name))
        if not query_image(process, 0, name, c.byref(size)) or not open_token(process, 8, c.byref(token)):
            raise c.WinError(c.get_last_error())
        stats = c.create_string_buffer(128)
        if not token_info(token, 10, stats, len(stats), c.byref(size)):  # TokenStatistics
            raise c.WinError(c.get_last_error())
        # TOKEN_STATISTICS begins with TokenId and AuthenticationId, both LUIDs.
        return Path(name.value), stats.raw[8:16].hex()
    finally:
        if token:
            close(token)
        close(process)


class ConsoleObserver:
    def __init__(self):
        self.windows = []
        self.starts = []
        self.hooks = []
        self.callback = CALLBACK(self.on_event)

    def on_event(self, hook, event, hwnd, obj, child, thread, timestamp):
        if event == CONSOLE_START:
            self.starts.append(obj)
        elif hwnd and obj == 0 and child == 0:
            name = c.create_unicode_buffer(256)
            get_class(hwnd, name, len(name))
            if name.value in ("ConsoleWindowClass", "CASCADIA_HOSTING_WINDOW_CLASS"):
                self.windows.append(hwnd)

    def pump(self):
        msg = w.MSG()
        while peek(c.byref(msg), None, 0, 0, 1):
            dispatch(c.byref(msg))

    def __enter__(self):
        for event in (SHOW, CONSOLE_START):
            hook = hook_event(event, event, None, self.callback, 0, 0, 0)
            if not hook:
                self.__exit__()
                raise c.WinError(c.get_last_error())
            self.hooks.append(hook)
        return self

    def __exit__(self, *args):
        for hook in self.hooks:
            unhook(hook)

    def calibrate(self):
        # CREATE_NEW_CONSOLE with SW_SHOWNORMAL is intentional only in this
        # positive control. The OS launches Cryptex independently of this process.
        info = subprocess.STARTUPINFO()
        info.dwFlags = subprocess.STARTF_USESHOWWINDOW
        info.wShowWindow = 1
        with subprocess.Popen([sys.executable, "-c", "import time; time.sleep(1)"],
                              creationflags=subprocess.CREATE_NEW_CONSOLE, startupinfo=info) as control:
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                self.pump()
                if control.pid in self.starts and self.windows:
                    break
                time.sleep(.01)
            else:
                raise RuntimeError(f"Console calibration failed: window shows={len(self.windows)}, "
                                   f"control start observed={control.pid in self.starts}")
        self.pump()
        self.windows.clear()
        self.starts.clear()


def observe(desktop, runtime, session):
    with ConsoleObserver() as observer:
        observer.calibrate()
        shell = Path(os.environ["SystemRoot"]) / "System32/runonce.exe"
        startup = subprocess.Popen([shell, "/AlternateShellStartup"])
        try:
            deadline = time.monotonic() + 90
            stable_since = None
            pids = None
            while time.monotonic() < deadline:
                observer.pump()
                if observer.windows or observer.starts:
                    raise RuntimeError(f"Console detected during logon: window shows={len(observer.windows)}, "
                                       f"console starts={len(observer.starts)}")
                if startup.poll() not in (0, None):
                    raise RuntimeError(f"Windows shell startup exited with {startup.returncode}")
                snapshot = (runtime / "gui-proof.snapshot").read_text(encoding="utf-8")
                fields = dict(line.split("=", 1) for line in snapshot.splitlines() if "=" in line)
                if fields.get("note") == "desktop_launched" and fields.get("daemon_pid_observation") == "live":
                    pair = (int(fields["desktop_pid"]), int(fields["daemon_pid"]))
                    if stable_since is None:
                        pids = pair
                        stable_since = time.monotonic()
                    if pair != pids:
                        raise RuntimeError("Desktop or daemon changed during startup")
                    if time.monotonic() - stable_since >= 10:
                        if identity(pair[0]) != (desktop, session) or identity(pair[1]) != (desktop.with_name("cryptexd.exe"), session):
                            raise RuntimeError("Installed image or logon identity mismatch")
                        observer.pump()
                        if observer.windows or observer.starts:
                            raise RuntimeError("Console observed at the end of startup")
                        return {"desktop_pid": pair[0], "daemon_pid": pair[1], "console_show_events": 0,
                                "console_process_events": 0, "positive_control_seen": True,
                                "observation_after_readiness_seconds": 10}
                time.sleep(.01)
            raise TimeoutError("Windows logon processing did not start the installed Desktop")
        finally:
            try:
                startup.wait(timeout=5)
            except subprocess.TimeoutExpired:
                startup.terminate()
                startup.wait(timeout=5)


def main():
    if (os.environ.get("GITHUB_ACTIONS") != "true" or os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted"
            or not re.fullmatch(r"cpxci[0-9a-f]{12}", os.environ["USERNAME"].lower())):
        raise RuntimeError("Use the disposable standard CI account")
    base = Path(os.environ["LOCALAPPDATA"])
    desktop = (base / "Programs/Cryptex/cryptex-desktop.exe").resolve(strict=True)
    runtime = base / "Cryptex-Dev/runtime/p0-2"
    scratch = Path(os.environ["RUNNER_TEMP"])
    session = identity(os.getpid())[1]
    if sys.argv[1:] == ["arm"]:
        if registration() is not None:
            raise RuntimeError("Existing startup registration in temporary account")
        with winreg.CreateKey(winreg.HKEY_CURRENT_USER, RUN) as key:
            winreg.SetValueEx(key, "Cryptex", 0, winreg.REG_SZ, f'"{desktop}" {FLAG}')
        check_entry(registration(), desktop)
        (scratch / "armed-logon").write_text(session, encoding="ascii")
        return
    if sys.argv[1:] != ["observe"] or (scratch / "armed-logon").read_text() == session:
        raise RuntimeError("Expected a fresh Windows logon after preparation")
    check_entry(registration(), desktop)
    try:
        report = observe(desktop, runtime, session)
    finally:
        check_entry(registration(), desktop)
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, RUN, 0, winreg.KEY_SET_VALUE) as key:
            winreg.DeleteValue(key, "Cryptex")
        (runtime / "quit.request").touch()
    if registration() is not None:
        raise RuntimeError("Test startup registration remains")
    deadline = time.monotonic() + 15
    while "note=clean_quit_via_request_file" not in (runtime / "gui-proof.snapshot").read_text():
        if time.monotonic() >= deadline:
            raise TimeoutError("Installed Desktop did not exit")
        time.sleep(.1)
    report.update(distinct_logon=True, startup_entry_removed=True, activation="windows-alternate-shell")
    print(json.dumps(report), flush=True)
    (scratch / "m14-windows-console-login.json").write_text(json.dumps(report, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
