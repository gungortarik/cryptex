"""Inspect both official release binaries without installing or launching either one."""
import base64
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
from tempfile import TemporaryDirectory
import zipfile

REPOSITORY = "syncthing/syncthing"
TARGETS = {"windows-amd64": "syncthing.exe", "macos-arm64": "syncthing"}


def upstream_notices(package, stem):
    result = {}
    for name in ("LICENSE.txt", "AUTHORS.txt"):
        matches = [entry for entry in package.infolist() if entry.filename == stem + "/" + name]
        if len(matches) != 1 or not 0 < matches[0].file_size <= 1024 * 1024:
            raise ValueError(f"Missing, duplicated or oversized upstream notice: {name}")
        contents = package.read(matches[0])
        if not contents.decode("utf-8").strip():
            raise ValueError(f"Empty upstream notice: {name}")
        result[name] = {"bytes": len(contents), "sha256": hashlib.sha256(contents).hexdigest(),
                        "contents_base64": base64.b64encode(contents).decode("ascii")}
    return result


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
        notices = upstream_notices(package, stem)
        info, = [member for member in package.infolist() if member.filename == stem + "/" + executable]
        if not 0 < info.file_size <= 256 * 1024 * 1024:
            raise ValueError("Unexpected executable size")
        with package.open(info) as source, binary.open("xb") as dest:
            shutil.copyfileobj(source, dest)
    out = subprocess.run(["govulncheck", "-mode=binary", "-format=json", binary], check=True,
                         capture_output=True, text=True, timeout=240)
    build = json.loads(subprocess.check_output(["go", "version", "-m", "-json", binary], timeout=30))
    if build.get("Main", {}).get("Path") != "github.com/syncthing/syncthing" or not build.get("Deps"):
        raise ValueError("Missing official executable dependency inventory")
    settings = {item["Key"]: item["Value"] for item in build["Settings"]}
    go_os, go_arch = {"macos-arm64": ("darwin", "arm64"), "windows-amd64": ("windows", "amd64")}[target]
    if (settings.get("GOOS"), settings.get("GOARCH")) != (go_os, go_arch):
        raise ValueError("Official executable build target differs from its archive")
    with binary.open("rb") as stream:
        binary_hash = hashlib.file_digest(stream, "sha256").hexdigest()
    return {"target": target, "archive_sha256": expected.removeprefix("sha256:"),
            "executable_sha256": binary_hash, "upstream_notices": notices,
            "linked_modules": build["Deps"],
            "source": f"https://github.com/{REPOSITORY}/tree/{tag}", **findings(out.stdout)}


def main():
    import argparse
    from owner_transport import VERSION

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("--owner-release", action="store_true")
    args = parser.parse_args()
    selector = f"tags/v{VERSION}" if args.owner_release else "latest"
    release = json.loads(subprocess.check_output(["gh", "api", f"repos/{REPOSITORY}/releases/{selector}"], timeout=60))
    if release["draft"] or release["prerelease"] or not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+", release["tag_name"]):
        raise ValueError("Expected an official stable release")
    with TemporaryDirectory() as tmp:
        results = [inspect(release, target, exe, Path(tmp)) for target, exe in TARGETS.items()]
    # This report concerns component matching only. The owner hold is released
    # separately after native compatibility, rollback and advisory review.
    report = {"release": release["tag_name"], "release_url": release["html_url"],
              "targets": results, "personal_use_authorized": False}
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({r["target"]: r["findings"] for r in results}))


if __name__ == "__main__":
    main()
