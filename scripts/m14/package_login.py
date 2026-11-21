"""Build the login rehearsal handoff for the runner's native architecture."""
import os
from pathlib import Path
import platform
import sys
import zipfile

binary = Path(sys.argv[1]).resolve(strict=True)
name = f"Cryptex-M14-{platform.system()}-{platform.machine()}"
target = Path(os.environ["RUNNER_TEMP"]) / f"{name}.zip"
with zipfile.ZipFile(target, "x", compression=zipfile.ZIP_DEFLATED) as archive:
    archive.write(binary, f"{name}/{binary.name}")
    archive.write(Path(__file__).with_name("login.py"), f"{name}/login.py")
    if os.name == "nt":
        archive.writestr(f"{name}/KONTROL.cmd", '@echo off\r\ncd /d "%~dp0"\r\npython login.py\r\npause\r\n')
    else:
        launcher = zipfile.ZipInfo(f"{name}/KONTROL.command")
        launcher.external_attr = 0o100755 << 16
        archive.writestr(launcher, '#!/bin/sh\ncd "$(dirname "$0")" || exit 1\npython3 login.py\nprintf "\\nEnter ile kapatin."\nread -r answer\n')
print(target)
