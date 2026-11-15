"""CI-only reproduction using a pinned signed app and 256 descriptor ceiling."""
import hashlib
import json
from pathlib import Path, PurePosixPath
import resource
import stat
import subprocess
import sys
import zipfile


def main():
    archive, destination = map(Path, sys.argv[1:])
    expected = "70db1c49e4e37e53236794737c673d84d4c103c87b816fd93f982be22e7955e4"
    if hashlib.sha256(archive.read_bytes()).hexdigest() != expected:
        raise ValueError("unexpected native archive")
    destination.mkdir(mode=0o700)
    prefix = "Cryptex-Mac-Aktarim/"
    with zipfile.ZipFile(archive) as source:
        members = source.infolist()
        if len(members) != 15 or len({i.filename for i in members}) != 15:
            raise ValueError("invalid archive inventory")
        for item in members:
            path = PurePosixPath(item.filename)
            if (not item.filename.startswith(prefix) or path.is_absolute()
                    or ".." in path.parts or "\\" in item.filename
                    or not stat.S_ISREG(item.external_attr >> 16)
                    or item.file_size > 64 * 1024 * 1024):
                raise ValueError("unsafe archive member")
        for item in members:
            target = destination / item.filename
            target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            target.write_bytes(source.read(item))
            target.chmod((item.external_attr >> 16) & 0o777)
    package = destination / prefix
    launcher = package / "MAC-AKTARIM.command"
    # Only the external collector changes; signed app and helpers remain intact.
    launcher.write_bytes(Path(__file__).with_name("mac_transport_owner.py").read_bytes())
    launcher.chmod(0o700)
    before = resource.getrlimit(resource.RLIMIT_NOFILE)
    limit = min(256, before[0]) if before[0] != resource.RLIM_INFINITY else 256
    resource.setrlimit(resource.RLIMIT_NOFILE, (limit, before[1]))
    # Same runner limit inherited by the launcher and all native children.
    # No raw native output is uploaded; owner collector emits only fixed codes.
    result = subprocess.run([str(launcher)], stdout=subprocess.DEVNULL,
                            stderr=subprocess.DEVNULL, timeout=420)
    outputs = list(package.glob("MAC-AKTARIM-SONUC-*.zip"))
    if len(outputs) != 1:
        raise ValueError("owner result missing")
    with zipfile.ZipFile(outputs[0]) as output:
        if output.namelist() != ["result.json"] or output.getinfo("result.json").file_size > 65536:
            raise ValueError("invalid result shape")
        report = json.loads(output.read("result.json"))
    receipt = {"scope": "CI reproduction only; not owner acceptance", "descriptor_soft_limit": limit,
               "prior_soft_limit": before[0], "launcher_exit": result.returncode,
               "native_archive_sha256": expected, "result": report}
    (destination / "fd-repro.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"outcome": report.get("outcome"), "soft_limit": limit,
                      "post_rejection_validation": report.get("post_rejection_validation")}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
