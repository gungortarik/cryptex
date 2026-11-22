"""Bundle the installer whose installed payload passed the release smoke."""
import json
import os
from pathlib import Path
import zipfile

from installed_update import ROOT, digest

scripts = Path(__file__).resolve().parent
output = Path(os.environ["RUNNER_TEMP"])
commit = os.environ["GITHUB_SHA"]
setup, = (ROOT / "target/release/bundle/nsis").glob("*-setup.exe")
smoke = json.loads((output / "m14-release-smoke.json").read_bytes())
manifest = {"source": commit, "installer_sha256": digest(setup),
            "desktop_sha256": smoke["desktop_sha256"], "daemon_sha256": smoke["daemon_sha256"]}
prefix = "Cryptex-Konsol/"
target = output / f"Cryptex-Konsol-{commit[:8]}.zip"
with zipfile.ZipFile(target, "x", compression=zipfile.ZIP_DEFLATED) as bundle:
    bundle.write(setup, prefix + "Cryptex-setup.exe")
    bundle.writestr(prefix + "package.json", json.dumps(manifest))
    for name in ("login.py", "windows_console_login.py", "windows_login_handoff.py"):
        bundle.write(scripts / name, prefix + name)
    bundle.writestr(prefix + "KONTROL.cmd", '@echo off\r\ncd /d "%~dp0"\r\npython -B windows_login_handoff.py\r\npause\r\n')
