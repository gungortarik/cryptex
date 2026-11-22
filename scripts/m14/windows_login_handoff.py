"""One-click Windows sign-in measurement, limited to the cryptexm14 account."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import winreg

from windows_console_login import FLAG, RUN, check_entry, identity, observe, registration

HERE = Path(__file__).resolve().parent
BASE = Path(os.environ["LOCALAPPDATA"])
DESKTOP = BASE / "Programs/Cryptex/cryptex-desktop.exe"
RUNTIME = BASE / "Cryptex-Dev/runtime/p0-2"
STATE = BASE / "Cryptex-M14-Console-Login"
RESULT = HERE / "M14-KONSOL-SONUC.json"
ONCE = RUN + "Once"
NAME = "CryptexM14ConsoleObserver"


def hash_file(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def watcher():
    return subprocess.list2cmdline([str(Path(sys.executable).with_name("pythonw.exe")),
                                    str(Path(__file__).resolve()), "observe"])


def remove_owned(key_name, name, expected):
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key_name, 0,
                            winreg.KEY_QUERY_VALUE | winreg.KEY_SET_VALUE) as key:
            value, kind = winreg.QueryValueEx(key, name)
            if (value, kind) != (expected, winreg.REG_SZ):
                raise RuntimeError("Baslangic kaydi degismis. Oldugu gibi birakildi.")
            winreg.DeleteValue(key, name)
    except FileNotFoundError:
        pass


def add_new(key_name, name, value):
    if len(value) >= 260:
        raise RuntimeError("Paketi daha kisa bir klasor yoluna cikarin.")
    with winreg.CreateKeyEx(winreg.HKEY_CURRENT_USER, key_name, 0,
                            winreg.KEY_SET_VALUE | winreg.KEY_QUERY_VALUE) as key:
        try:
            winreg.QueryValueEx(key, name)
        except FileNotFoundError:
            winreg.SetValueEx(key, name, 0, winreg.REG_SZ, value)
            return
        raise RuntimeError("Gecici hesapta mevcut baslangic kaydi var.")


def installed(build, *, allow_install):
    if not DESKTOP.is_file() and allow_install:
        setup = HERE / "Cryptex-setup.exe"
        if DESKTOP.parent.exists() or hash_file(setup) != build["installer_sha256"]:
            raise RuntimeError("Kurulum klasoru veya paket beklenen durumda degil.")
        subprocess.run([setup, "/S", "/D=" + str(DESKTOP.parent)], check=True, timeout=180)
    if (hash_file(DESKTOP), hash_file(DESKTOP.with_name("cryptexd.exe"))) != (
            build["desktop_sha256"], build["daemon_sha256"]):
        raise RuntimeError("Kurulu Cryptex surumu bu paketle eslesmiyor.")


def cleanup(entry):
    remove_owned(RUN, "Cryptex", entry)
    remove_owned(ONCE, NAME, watcher())


def main():
    if os.environ.get("USERNAME", "").lower() != "cryptexm14" or os.environ.get("CRYPTEXD_BIN"):
        raise RuntimeError("Bu paketi cryptexm14 gecici hesabinda acin.")
    session = identity(os.getpid())[1]
    build = json.loads((HERE / "package.json").read_bytes())
    entry = f'"{DESKTOP}" {FLAG}'
    arm = STATE / "armed.json"
    if sys.argv[1:] == ["observe"]:
        prior = json.loads(arm.read_bytes())
        if prior != {"session": prior["session"], "source": build["source"], "folder": str(HERE)}:
            raise RuntimeError("Hazirlik kaydi bu pakete ait degil.")
        report = {"platform": "win32", "activation": "interactive-windows-sign-in", "source": build["source"]}
        try:
            if session == prior["session"]:
                raise RuntimeError("Yeni oturum gerekli.")
            check_entry(registration(), DESKTOP)
            report.update(observe(DESKTOP, RUNTIME, session, windows_logon=True))
            installed(build, allow_install=False)
            report.update(distinct_logon=True, desktop_sha256=build["desktop_sha256"])
        except Exception as error:
            report["error"] = str(error)
        try:
            cleanup(entry)
            report["startup_entry_removed"] = registration() is None
        except Exception as error:
            report["cleanup_error"] = str(error)
        report["passed"] = report.get("startup_entry_removed", False) and "error" not in report and "cleanup_error" not in report
        pending = RESULT.with_suffix(".tmp")
        pending.write_text(json.dumps(report, indent=2), encoding="utf-8")
        pending.replace(RESULT)
        arm.unlink()
        return
    if sys.argv[1:]:
        raise ValueError("Bilinmeyen islem")
    if RESULT.exists():
        print(RESULT, flush=True)
        return
    if arm.exists():
        if json.loads(arm.read_bytes())["session"] == session:
            print("Hazir. Windows oturumunu kapatip cryptexm14 hesabina yeniden girin.")
        else:
            print("Olcum devam ediyor. Yaklasik iki dakika sonra bu klasorde sonucu kontrol edin.")
        return
    if registration() is not None:
        raise RuntimeError("Hesabin mevcut Cryptex baslangic kaydi korunuyor.")
    installed(build, allow_install=True)
    STATE.mkdir(exist_ok=True)
    with arm.open("x", encoding="utf-8") as out:
        json.dump({"source": build["source"], "session": session, "folder": str(HERE)}, out)
    try:
        add_new(ONCE, NAME, watcher())
        try:
            add_new(RUN, "Cryptex", entry)
        except Exception:
            remove_owned(ONCE, NAME, watcher())
            raise
    except Exception:
        arm.unlink()
        raise
    print("HAZIR. Oturumu kapatip cryptexm14 hesabina tekrar girin.")
    print("Iki dakika dokunmadan bekleyin. Olcumden sonra kisa bir kalibrasyon penceresi acilir.")
    print("Sonuc dosyasi: " + str(RESULT))


if __name__ == "__main__":
    main()
