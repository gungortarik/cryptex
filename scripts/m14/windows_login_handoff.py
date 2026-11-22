"""One-click Windows sign-in measurement, limited to the cryptexm14 account."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import uuid
import winreg

from windows_console_login import ConsoleObserver, FLAG, RUN, check_entry, identity, observe, registration
from windows_observer_task import observer_task

HERE = Path(__file__).resolve().parent
BASE = Path(os.environ["LOCALAPPDATA"])
DESKTOP = BASE / "Programs/Cryptex/cryptex-desktop.exe"
RUNTIME = BASE / "Cryptex-Dev/runtime/p0-2"
STATE = BASE / "Cryptex-M14-Console-Login"
RESULT = HERE / "M14-KONSOL-SONUC.json"


def hash_file(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


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


def task(operation, token):
    observer_task(operation, token, Path(__file__))


def main():
    if os.environ.get("USERNAME", "").lower() != "cryptexm14" or os.environ.get("CRYPTEXD_BIN"):
        raise RuntimeError("Bu paketi cryptexm14 gecici hesabinda acin.")
    session = identity(os.getpid())[1]
    build = json.loads((HERE / "package.json").read_bytes())
    entry = f'"{DESKTOP}" {FLAG}'
    arm = STATE / "armed.json"
    if len(sys.argv) == 3 and sys.argv[1] == "observe":
        token = sys.argv[2]
        prior = json.loads(arm.read_bytes())
        if prior != {"session": prior["session"], "source": build["source"], "folder": str(HERE), "token": token}:
            raise RuntimeError("Hazirlik kaydi bu pakete ait degil.")
        if session == prior["session"]:
            # Only the on-demand preflight can run before the real sign-in.
            with ConsoleObserver() as observer:
                observer.calibrate()
            (STATE / (token + ".ready")).touch(exist_ok=False)
            return
        report = {"platform": "win32", "activation": "interactive-windows-sign-in", "source": build["source"],
                  "observer": "per-user-logon-trigger", "distinct_logon": True}
        try:
            check_entry(registration(), DESKTOP)
            report.update(observe(DESKTOP, RUNTIME, session, windows_logon=True))
            installed(build, allow_install=False)
            report.update(distinct_logon=True, desktop_sha256=build["desktop_sha256"])
        except Exception as error:
            report["error"] = str(error)
        try:
            remove_owned(RUN, "Cryptex", entry)
            report["startup_entry_removed"] = registration() is None
        except Exception as error:
            report["cleanup_error"] = str(error)
        try:
            task("remove", token)
            report["observer_task_removed"] = True
        except Exception as error:
            report["observer_cleanup_error"] = str(error)
        report["passed"] = (report.get("startup_entry_removed", False) and report.get("observer_task_removed", False)
                            and "error" not in report and "cleanup_error" not in report)
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
        state = json.loads(arm.read_bytes())
        if state.get("folder") != str(HERE) or state.get("source") != build["source"] or not state.get("token"):
            raise RuntimeError("Onceki paketin hazirlik kaydi var. Bu olcum baslatilmadi.")
        if state["session"] == session and registration() == entry:
            print("Hazir. Windows oturumunu kapatip cryptexm14 hesabina yeniden girin.")
        else:
            print("Olcum devam ediyor. Yaklasik iki dakika sonra bu klasorde sonucu kontrol edin.")
        return
    if registration() is not None:
        raise RuntimeError("Hesabin mevcut Cryptex baslangic kaydi korunuyor.")
    installed(build, allow_install=(HERE / "Cryptex-setup.exe").is_file())
    STATE.mkdir(exist_ok=True)
    token = uuid.uuid4().hex
    marker = STATE / (token + ".ready")
    with arm.open("x", encoding="utf-8") as out:
        json.dump({"source": build["source"], "session": session, "folder": str(HERE), "token": token}, out)
    try:
        task("create", token)
        try:
            task("preflight", token)
            deadline = time.monotonic() + 30
            while not marker.exists():
                if time.monotonic() >= deadline:
                    raise TimeoutError("Olcum araci gecici hesapta kalibrasyonu tamamlayamadi.")
                time.sleep(.1)
            add_new(RUN, "Cryptex", entry)
        except Exception:
            task("remove", token)
            raise
    except Exception:
        arm.unlink()
        raise
    finally:
        marker.unlink(missing_ok=True)
    print("HAZIR. Oturumu kapatip cryptexm14 hesabina tekrar girin.")
    print("Iki dakika dokunmadan bekleyin. Olcumden sonra kisa bir kalibrasyon penceresi acilir.")
    print("Sonuc dosyasi: " + str(RESULT))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        if len(sys.argv) == 1:
            diagnostic = HERE / "M14-HAZIRLIK-HATA.json"
            diagnostic.write_text(json.dumps({"stage": "prepare", "error": str(error)}, indent=2),
                                  encoding="utf-8")
            print("Hazirlik tamamlanamadi. Oturumu kapatmayin. Sonuc: " + str(diagnostic))
        raise
