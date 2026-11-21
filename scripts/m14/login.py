"""Arm and verify the product's login registration in a disposable OS account."""
import argparse
import getpass
import json
import os
from pathlib import Path
import plistlib
import subprocess
import sys
import time

WINDOWS = os.name == "nt"
BASE = Path(os.environ["LOCALAPPDATA"]) if WINDOWS else Path.home() / "Library/Application Support"
STATE = BASE / "Cryptex-M14-Login"
PLIST = Path.home() / "Library/LaunchAgents/Cryptex.plist"
FLAG = "--cryptex-background"


def registration():
    if WINDOWS:
        import winreg
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Run") as key:
                value, kind = winreg.QueryValueEx(key, "Cryptex")
                if kind != winreg.REG_SZ:
                    raise ValueError("Unexpected startup value type")
                return value
        except FileNotFoundError:
            return None
    if not PLIST.exists():
        return None
    with PLIST.open("rb") as source:
        return plistlib.load(source)


def check_entry(entry, binary):
    if WINDOWS:
        # Accept Windows paths with or without surrounding quotes. Validate the
        # entire value before passing it unchanged to CreateProcess in CI.
        if entry not in (f"{binary} {FLAG}", f'"{binary}" {FLAG}'):
            raise ValueError("Unexpected executable or arguments in the startup entry")
    elif not (isinstance(entry, dict) and entry.get("Label") == "Cryptex"
              and entry.get("ProgramArguments") == [str(binary), FLAG]
              and entry.get("RunAtLoad") is True and not entry.get("KeepAlive")):
        raise ValueError("Unexpected login agent or arguments")


def prepare(binary, hosted):
    if registration() is not None:
        raise RuntimeError("This account already has a Cryptex startup entry")
    STATE.mkdir(parents=True, exist_ok=False)
    (STATE / "executable").write_text(str(binary), encoding="utf-8")
    if hosted:
        (STATE / "hosted").touch()
    subprocess.run([binary, "--m14-arm"], check=True, timeout=60)
    entry = registration()
    check_entry(entry, binary)
    return entry


def verify(real_login):
    until = time.monotonic() + 160
    while not (STATE / "disabled").exists():
        if time.monotonic() >= until:
            raise TimeoutError("Cryptex did not complete startup and removal in time")
        time.sleep(1)
    armed = (STATE / "armed-session").read_text().strip()
    ready = (STATE / "ready-session").read_text().strip()
    attempts = [p.read_text().strip() for p in STATE.glob("launch-*")]
    if not ready or not armed or attempts != [ready]:
        raise RuntimeError(f"Expected one launch in the observed session, saw {len(attempts)}")
    if real_login and armed == ready:
        raise RuntimeError("A fresh login session is required")
    if registration() is not None:
        raise RuntimeError("The native startup entry still exists")
    result = {"platform": sys.platform, "check": "new-login" if real_login else "registered-launch",
              "launches": len(attempts), "startup_entry_removed": True,
              "distinct_login_session": ready != armed, "observation_seconds": 60}
    (STATE / "result.json").write_text(json.dumps(result, indent=2), encoding="utf-8")
    return result


def disarm_owned(binary):
    entry = registration()
    if entry is not None:
        check_entry(entry, binary)
        subprocess.run([binary, "--m14-disarm"], check=True, timeout=60)
        if registration() is not None:
            raise RuntimeError("Startup entry cleanup failed")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--ci", action="store_true")
    parser.add_argument("--binary", type=Path)
    args = parser.parse_args()
    user = getpass.getuser().lower()
    if args.ci:
        if not (os.environ.get("GITHUB_ACTIONS") == "true"
                and os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted"
                and user in ("runner", "runneradmin")):
            raise RuntimeError("Hosted disposable account required")
    elif user != "cryptexm14":
        raise RuntimeError("Bu paketi cryptexm14 isimli gecici kullanicida acin.")
    binary = (args.binary or Path(__file__).with_name(
        "cryptex-desktop.exe" if WINDOWS else "cryptex-desktop")).resolve(strict=True)
    if not args.ci and STATE.exists():
        if (STATE / "executable").read_text(encoding="utf-8") != str(binary):
            raise RuntimeError("Use the original test folder")
        armed = (STATE / "armed-session").read_text().strip()
        current = subprocess.check_output([binary, "--m14-session"], text=True, timeout=15).strip()
        if current == armed:
            print("Test hazir. Gecici kullanicidan cikis yapip tekrar girin, sonra KONTROL dosyasini acin.")
            return
        try:
            result = verify(real_login=True)
        finally:
            disarm_owned(binary)
        report = Path(__file__).with_name(f"M14-OTURUM-SONUC-{sys.platform}.json")
        report.write_text(json.dumps(result, indent=2), encoding="utf-8")
        print(f"Sonuc: {report}")
        return
    if STATE.exists() or registration() is not None:
        raise RuntimeError("Use a fresh temporary OS account")
    try:
        entry = prepare(binary, args.ci)
        if not args.ci:
            print("Hazir. Gecici hesaptan cikis yapip tekrar girin. Ayni KONTROL dosyasini acin.")
            return
        if WINDOWS:
            subprocess.run(entry, check=True, timeout=110)
        else:
            domain = f"gui/{os.getuid()}"
            subprocess.run(["/bin/launchctl", "bootstrap", domain, str(PLIST)], check=True)
        try:
            print(json.dumps(verify(real_login=False)), flush=True)
        finally:
            if not WINDOWS:
                subprocess.run(["/bin/launchctl", "bootout", f"{domain}/Cryptex"], check=True)
    except Exception:
        disarm_owned(binary)
        raise


if __name__ == "__main__":
    main()
