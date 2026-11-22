"""Run the feature-specific debug build against a new temporary pilot profile."""
import os
from pathlib import Path
import subprocess
import tempfile
import uuid


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
            env={**os.environ, "CRYPTEX_FILE_FIXTURE_SOCKET": str(uuid.uuid4())},
        )
        if result.returncode or "M13 native shell verified" not in result.stdout.splitlines():
            raise RuntimeError(f"Shell rehearsal exited {result.returncode}. {result.stderr[-2000:]}")
        layouts = [line for line in result.stdout.splitlines() if line.startswith("M14 native layout ")]
        expected = [f"M14 native layout {theme} {zoom}x" for theme in ("Light", "Dark") for zoom in ("1", "1.5", "2")]
        if layouts != expected:
            raise RuntimeError(f"Unexpected native display matrix: {layouts}")
        print("\n".join(layouts))
        print("Verified the native shell, keyboard navigation, engine restart, stop review and confirmed stop.")


if __name__ == "__main__":
    main()
