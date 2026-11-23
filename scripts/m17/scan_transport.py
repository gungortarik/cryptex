"""Inspect both official release binaries without installing or launching either one."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
from tempfile import TemporaryDirectory
import zipfile

REPOSITORY = "syncthing/syncthing"
TARGETS = {"windows-amd64": "syncthing.exe", "macos-arm64": "syncthing"}


def findings(output):
    decoder = json.JSONDecoder()
    found = set()
    scanner = None
    sbom = None
    while output.strip():
        record, offset = decoder.raw_decode(output.lstrip())
        output = output.lstrip()[offset:]
        if "config" in record:
            scanner = record["config"]
        if "SBOM" in record:
            sbom = record["SBOM"]
        if "finding" in record:
            found.add(record["finding"]["osv"])
    if not scanner or scanner.get("scan_mode") != "binary" or not sbom or not sbom.get("modules"):
        raise ValueError("Incomplete Go binary scan")
    return {"scanner": scanner, "findings": sorted(found), "go_version": sbom.get("go_version")}


def inspect(release, target, executable, directory):
    tag = release["tag_name"]
    stem = f"syncthing-{target}-{tag}"
    filename = stem + ".zip"
    asset, = [asset for asset in release["assets"] if asset["name"] == filename]
    expected = asset.get("digest") or ""
    if not re.fullmatch(r"sha256:[0-9a-f]{64}", expected):
        raise ValueError("Missing official asset SHA-256")
    subprocess.run(["gh", "release", "download", tag, "--repo", REPOSITORY,
                    "--pattern", filename, "--dir", directory], check=True, timeout=180)
    archive = directory / filename
    with archive.open("rb") as stream:
        if archive.stat().st_size != asset["size"] or "sha256:" + hashlib.file_digest(stream, "sha256").hexdigest() != expected:
            raise ValueError("Release asset integrity check failed")
    binary = directory / executable
    with zipfile.ZipFile(archive) as package:
        info, = [member for member in package.infolist() if member.filename == stem + "/" + executable]
        if not 0 < info.file_size <= 256 * 1024 * 1024:
            raise ValueError("Unexpected executable size")
        with package.open(info) as source, binary.open("xb") as dest:
            shutil.copyfileobj(source, dest)
    out = subprocess.run(["govulncheck", "-mode=binary", "-format=json", binary], check=True,
                         capture_output=True, text=True, timeout=240)
    return {"target": target, "archive_sha256": expected.removeprefix("sha256:"), **findings(out.stdout)}


def main():
    release = json.loads(subprocess.check_output(["gh", "api", f"repos/{REPOSITORY}/releases/latest"], timeout=60))
    if release["draft"] or release["prerelease"] or not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+", release["tag_name"]):
        raise ValueError("Expected an official stable release")
    with TemporaryDirectory() as tmp:
        results = [inspect(release, target, exe, Path(tmp)) for target, exe in TARGETS.items()]
    # This report concerns component matching only. The owner hold is released
    # separately after native compatibility, rollback and advisory review.
    report = {"release": release["tag_name"], "release_url": release["html_url"],
              "targets": results, "personal_use_authorized": False}
    Path(sys.argv[1]).write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({r["target"]: r["findings"] for r in results}))


if __name__ == "__main__":
    main()
