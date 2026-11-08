"""Package an exact Git revision and native Mac runner with Unix ZIP modes."""

import argparse
import hashlib
from pathlib import Path, PurePosixPath
import subprocess
import tempfile
import zipfile


ROOT = Path(__file__).resolve().parents[2]


def git(*args: str) -> bytes:
    return subprocess.check_output(["git", "-C", str(ROOT), *args])


def build(revision: str, output: Path) -> tuple[str, str]:
    commit = git("rev-parse", "--verify", f"{revision}^{{commit}}").decode().strip()
    if len(commit) != 40 or any(c not in "0123456789abcdef" for c in commit):
        raise ValueError("invalid commit binding")
    temporary_parent = Path(tempfile.gettempdir()).resolve()
    with tempfile.TemporaryDirectory(prefix="cx-mac-package-", dir=temporary_parent) as temporary:
        scratch = Path(temporary).resolve()
        if scratch.parent != temporary_parent or not scratch.name.startswith("cx-mac-package-"):
            raise ValueError("unexpected package scratch location")
        source = scratch / "source.zip"
        subprocess.run(
            ["git", "-C", str(ROOT), "archive", "--format=zip", f"--output={source}", commit],
            check=True,
        )
        with zipfile.ZipFile(source) as archive:
            if sum(entry.file_size for entry in archive.infolist()) > 128 * 1024 * 1024:
                raise ValueError("source archive exceeds package bound")
            for entry in archive.infolist():
                path = PurePosixPath(entry.filename)
                if path.is_absolute() or ".." in path.parts or "\\" in entry.filename:
                    raise ValueError("unsafe source archive path")
                if (entry.external_attr >> 16) & 0o170000 == 0o120000:
                    raise ValueError("source archive contains a symlink")
        source_bytes = source.read_bytes()
    digest = hashlib.sha256(source_bytes).hexdigest()
    members = {
        "MAC-TEST.command": (0o100755, git("show", f"{commit}:scripts/m8/MAC-TEST.command")),
        "MAC-TEST-BASLA.txt": (0o100644, git("show", f"{commit}:scripts/m8/MAC-TEST-BASLA.txt")),
        "SOURCE-COMMIT.txt": (0o100644, (commit + "\n").encode()),
        "SOURCE-SHA256.txt": (0o100644, (digest + "\n").encode()),
        "source.zip": (0o100644, source_bytes),
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, "w") as archive:
        for name, (mode, data) in members.items():
            item = zipfile.ZipInfo(name)
            item.create_system = 3
            item.external_attr = mode << 16
            item.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(item, data)
    with zipfile.ZipFile(output) as archive:
        if archive.namelist() != list(members):
            raise ValueError("unexpected package member list")
        for name, (mode, data) in members.items():
            if archive.read(name) != data or archive.getinfo(name).external_attr >> 16 != mode:
                raise ValueError("package member or mode mismatch")
    print(f"source_commit={commit}")
    print(f"source_sha256={digest}")
    print(f"package={output.resolve()}")
    return commit, digest


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", default="HEAD")
    parser.add_argument(
        "--output", type=Path,
        default=Path.home() / "Downloads" / "cryptex-mac-test-20261108.zip",
    )
    args = parser.parse_args()
    build(args.revision, args.output)
