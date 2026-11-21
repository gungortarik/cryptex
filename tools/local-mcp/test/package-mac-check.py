"""Build the macOS owner-check ZIP with the locked bridge dependencies."""
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import zipfile

HERE = Path(__file__).resolve().parent
SOURCE = HERE.parent
LAUNCHER = """#!/bin/bash
cd "$(dirname "$0")" || exit 1
python3 mac-owner-check.py "$@"
status=$?
if [ "$#" -eq 0 ]; then
  if [ "$status" -ne 0 ]; then echo "Kontrol tamamlanamadi. Terminaldeki son mesaji paylasabilirsiniz."; fi
  read -r -p "Pencereyi kapatmak icin Enter'a basin." _
fi
exit "$status"
"""

with tempfile.TemporaryDirectory() as temp:
    root = Path(temp) / "Cryptex-M12-Antigravity"
    bridge = root / "bridge"
    bridge.mkdir(parents=True)
    for name in ["package.json", "package-lock.json", "stdio.mjs", "server.mjs", "consent.mjs", "health.mjs", "client-config.mjs"]:
        shutil.copyfile(SOURCE / name, bridge / name)
    subprocess.run([shutil.which("npm.cmd" if os.name == "nt" else "npm"), "ci", "--prefix",
                    bridge, "--ignore-scripts", "--omit=dev", "--no-audit"], check=True)
    for name in ["discovery-witness.mjs", "antigravity-acp.py", "mac-owner-check.py"]:
        shutil.copyfile(HERE / name, root / name)
    (root / "KONTROL.command").write_bytes(LAUNCHER.encode())
    with zipfile.ZipFile(sys.argv[1], "w", compression=zipfile.ZIP_DEFLATED) as bundle:
        for file in sorted(root.rglob("*")):
            if not file.is_file() or ".bin" in file.parts:
                continue
            item = zipfile.ZipInfo((Path(root.name) / file.relative_to(root)).as_posix())
            item.create_system = 3
            item.external_attr = (stat.S_IFREG | (0o755 if file.suffix == ".command" else 0o644)) << 16
            bundle.writestr(item, file.read_bytes(), compress_type=zipfile.ZIP_DEFLATED)
