"""Verify the native Kit package against two daemon-created, signed generations."""
from pathlib import Path
import subprocess
import sys
import tempfile
from test_offline_launchers import extract


if __name__ == "__main__":
    with tempfile.TemporaryDirectory(prefix="cryptex-offline-edition-") as temp:
        kit = extract(Path(sys.argv[1]), Path(temp))
        runtime = kit / ("node.exe" if sys.platform == "win32" else "node")
        subprocess.run([runtime, "--no-addons", Path(__file__).with_name("test_kit_advance.cjs"),
                        kit, temp, *sys.argv[2:]], check=True, timeout=120)
