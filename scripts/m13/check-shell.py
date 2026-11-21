"""Run the feature-specific debug build against a new temporary pilot profile."""
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    suffix = ".exe" if os.name == "nt" else ""
    target = Path(os.environ.get("CARGO_TARGET_DIR", "target"))
    binary = (target / "debug" / ("cryptex-desktop" + suffix)).resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="cryptex-m13-shell-") as directory:
        runtime = Path(directory).resolve(strict=True)
        (runtime / "pilot-runtime.marker").write_bytes(b"m0-native-desktop-runtime-v1\n")
        result = subprocess.run(
            [binary, "--cryptex-pilot-runtime", runtime], cwd=runtime, timeout=120,
            capture_output=True, text=True, errors="replace",
        )
        if result.returncode or "M13 native shell verified" not in result.stdout.splitlines():
            raise RuntimeError(f"Shell rehearsal exited {result.returncode}. {result.stderr[-2000:]}")
        print("Verified title bar, menus and tray presence, system font policy, both themes, close and reopen.")


if __name__ == "__main__":
    main()
