"""Open both owner tools from the extracted native package without an interactive owner."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import zipfile


def check(archive):
    with tempfile.TemporaryDirectory(prefix="cryptex offline kit ") as directory:
        root = Path(directory)
        if sys.platform == "darwin":
            # Exercise the executable permissions preserved in the distributed archive.
            subprocess.run(["ditto", "-x", "-k", archive, root], check=True)
        elif sys.platform == "win32":
            with zipfile.ZipFile(archive) as package:
                package.extractall(root)
        else:
            raise RuntimeError("Run on the OS for which the Kit was built")
        kit = root / "Cryptex-Offline-Kit"
        environment = dict(os.environ, NODE_OPTIONS="--cryptex-unknown-option")
        for title, expected in (
            ("Authorize", "Use an interactive terminal:"),
            ("Export public recipient", "Could not verify the Kit or export to a new folder."),
        ):
            if sys.platform == "win32":
                command = f'"{os.environ["ComSpec"]}" /d /c call "{title}.cmd"'
            else:
                command = [kit / f"{title}.command"]
            result = subprocess.run(command, cwd=kit, env=environment, input="\n", text=True,
                                    capture_output=True, timeout=30)
            if result.returncode != 1 or expected not in result.stderr:
                raise AssertionError(f"{title} did not reach the packaged owner prompt")


if __name__ == "__main__":
    check(Path(sys.argv[1]).resolve(strict=True))
