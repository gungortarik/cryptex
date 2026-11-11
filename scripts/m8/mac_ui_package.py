"""Export only the tested positive bundle and the interactive test launcher."""
import hashlib
import json
import re
import ssl
import zipfile
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath


def export_fixture(bundle, files, destination, source, architecture, expires):
    if not re.fullmatch(r"[0-9a-f]{40}", source) or architecture not in ("arm64", "x86_64"):
        raise ValueError("invalid fixture source or architecture")
    if not expires.startswith("notAfter="):
        raise ValueError("missing fixture expiry")
    expiry = datetime.fromtimestamp(ssl.cert_time_to_seconds(expires[9:]), timezone.utc)
    binaries = {"Contents/MacOS/cryptex-desktop", "Contents/MacOS/cryptexd"}
    required = binaries | {"Contents/Resources/startup-check.txt", "Contents/Resources/frontend/index.html"}
    manifest = "Contents/Resources/m8-startup.json"
    cms = "Contents/Resources/m8-startup.cms"
    if not required.issubset(item["path"] for item in files):
        raise ValueError("incomplete fixture inventory")

    def add(archive, name, data, executable=False):
        info = zipfile.ZipInfo("Cryptex-Mac-UI/" + name)
        info.create_system = 3
        info.external_attr = (0o100700 if executable else 0o100600) << 16
        archive.writestr(info, data, compress_type=zipfile.ZIP_DEFLATED)

    with zipfile.ZipFile(destination, "x") as archive:
        for item in files:
            name = item["path"]
            path = bundle / name
            if ((name not in required and not name.startswith("Contents/Resources/frontend/"))
                    or ".." in PurePosixPath(name).parts or "\\" in name
                    or path.resolve() != bundle.resolve() / name):
                raise ValueError("invalid fixture member")
            data = path.read_bytes()
            if len(data) != item["size"] or hashlib.sha256(data).hexdigest() != item["sha256"]:
                raise ValueError("fixture changed after test")
            add(archive, "positive.app/" + name, data, name in binaries)
        for name in (manifest, cms):
            add(archive, "positive.app/" + name, (bundle / name).read_bytes())
        add(archive, "fixture.json", json.dumps({
            "source": source, "architecture": architecture, "expires_utc": expiry.isoformat(),
        }).encode())
        add(archive, "MAC-ARAYUZ.command", Path(__file__).with_name("mac_ui_owner.py").read_bytes(), True)
