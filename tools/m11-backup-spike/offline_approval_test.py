"""Exercise the offline signer using only disposable caller-provided paths."""
from pathlib import Path
import subprocess
import sys
import tempfile

from offline_kit import scripts
from packaged_codec import SPIKE


if __name__ == "__main__":
    with tempfile.TemporaryDirectory() as temp:
        scripts(Path(temp))
        subprocess.run(["node", SPIKE / "offline_approval_test.cjs",
                        Path(temp) / "authorize.cjs", *sys.argv[1:]], check=True, timeout=90)
