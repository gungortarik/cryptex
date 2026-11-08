"""Package pinned source and one-click isolated Windows platform checks."""

import argparse
import hashlib
from pathlib import Path, PurePosixPath
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[2]


def git(*args: str) -> bytes:
    return subprocess.check_output(["git", "-C", str(ROOT), *args])


def build(revision: str, output: Path) -> None:
    commit = git("rev-parse", "--verify", f"{revision}^{{commit}}").decode().strip()
    if len(commit) != 40 or any(c not in "0123456789abcdef" for c in commit):
        raise ValueError("invalid source commit")
    parent = Path(tempfile.gettempdir()).resolve()
    with tempfile.TemporaryDirectory(prefix="cx-windows-package-", dir=parent) as temporary:
        scratch = Path(temporary).resolve()
        if scratch.parent != parent or not scratch.name.startswith("cx-windows-package-"):
            raise ValueError("invalid package scratch path")
        source = scratch / "source.zip"
        subprocess.run(["git", "-C", str(ROOT), "archive", "--format=zip", f"--output={source}", commit], check=True)
        with zipfile.ZipFile(source) as archive:
            if sum(e.file_size for e in archive.infolist()) > 128 * 1024 * 1024:
                raise ValueError("source expansion bound")
            for entry in archive.infolist():
                path = PurePosixPath(entry.filename)
                if path.is_absolute() or ".." in path.parts or "\\" in entry.filename or ":" in entry.filename:
                    raise ValueError("unsafe source path")
                if (entry.external_attr >> 16) & 0o170000 == 0o120000:
                    raise ValueError("source symlink")
        data = source.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    members = {
        name: git("show", f"{commit}:scripts/m8/{name}").replace(b"\r\n", b"\n")
        for name in ("WIN-TEST.cmd", "WIN-TEST.ps1", "WIN-TEST-BASLA.txt")
    }
    members["WIN-TEST.cmd"] = members["WIN-TEST.cmd"].replace(b"\n", b"\r\n")
    members.update({"SOURCE-COMMIT.txt": (commit + "\n").encode(), "SOURCE-SHA256.txt": (digest + "\n").encode(), "source.zip": data})
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, content in members.items():
            archive.writestr(name, content)
    with zipfile.ZipFile(output) as archive:
        if archive.namelist() != list(members) or any(archive.read(n) != b for n, b in members.items()):
            raise ValueError("package verification failed")
    print(f"source_commit={commit}")
    print(f"source_sha256={digest}")
    print(f"package={output.resolve()}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", default="HEAD")
    parser.add_argument("--output", type=Path, default=Path.home() / "Downloads" / "cryptex-win-test-20261108.zip")
    options = parser.parse_args()
    build(options.revision, options.output)
