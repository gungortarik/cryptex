"""Build an exact-commit, opt-in Windows native controller transfer pilot."""
import argparse
import hashlib
import ipaddress
import json
from pathlib import Path
import re
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[2]
NATIVE = "36a0f7bc372f64fa7cc4f5654fa324c0dd9f7fef2e07565e00c6e1cf73f50344"


def git(*args):
    return subprocess.check_output(["git", "-C", str(ROOT), *args])


def build(revision, relay_path, binary, output, previous_run=None):
    commit = git("rev-parse", "--verify", f"{revision}^{{commit}}").decode().strip()
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("invalid commit")
    raw = relay_path.read_bytes()
    relay = json.loads(raw)
    if len(raw) > 2048 or set(relay) != {"schema", "session", "peer_id", "address"} or relay["schema"] != 1:
        raise ValueError("relay schema")
    if not re.fullmatch(r"[0-9a-f]{32}", relay["session"]) or not re.fullmatch(r"(?:[A-Z2-7]{7}-){7}[A-Z2-7]{7}", relay["peer_id"]):
        raise ValueError("relay identity")
    address = re.fullmatch(r"tcp://([0-9.]+):([0-9]+)", relay["address"])
    if not address or not 1024 < int(address[2]) <= 65535:
        raise ValueError("relay address")
    ip = ipaddress.IPv4Address(address[1])
    if not any(ip in ipaddress.IPv4Network(n) for n in ("10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16", "127.0.0.0/8")):
        raise ValueError("private IPv4 required")
    native = binary.read_bytes()
    if hashlib.sha256(native).hexdigest() != NATIVE:
        raise ValueError("native digest mismatch")
    # TemporaryDirectory deletes only its own fresh, verified scratch directory.
    with tempfile.TemporaryDirectory(prefix="cx-native-package-") as scratch:
        path = Path(scratch) / "source.zip"
        subprocess.run(["git", "-C", str(ROOT), "archive", "--format=zip", f"--output={path}", commit], check=True)
        source = path.read_bytes()
    members = {n: git("show", f"{commit}:scripts/m8/{n}") for n in (
        "NATIVE-AKTAR.cmd", "NATIVE-AKTAR.ps1", "NATIVE-AKTAR-BASLA.txt")}
    members["NATIVE-AKTAR.cmd"] = members["NATIVE-AKTAR.cmd"].replace(b"\r\n", b"\n").replace(b"\n", b"\r\n")
    if previous_run is not None:
        if not re.fullmatch(r"[0-9a-f]{12}", previous_run):
            raise ValueError("invalid previous run")
        members["stop-native-controller-run.ps1"] = git("show", f"{commit}:scripts/m8/stop-native-controller-run.ps1")
        cancel = (f'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0stop-native-controller-run.ps1" '
                  f'-Run {previous_run} -Session {relay["session"]}\r\n'
                  'if errorlevel 1 (\r\n  pause\r\n  exit /b 1\r\n)\r\n').encode()
        members["NATIVE-AKTAR.cmd"] = members["NATIVE-AKTAR.cmd"].replace(
            b'powershell.exe -NoProfile', cancel + b'powershell.exe -NoProfile', 1)
    members.update({"source.zip": source, "SOURCE-COMMIT.txt": (commit+"\n").encode(),
        "SOURCE-SHA256.txt": (hashlib.sha256(source).hexdigest()+"\n").encode(),
        "relay.json": raw, "syncthing.exe": native})
    for n in ("LICENSE.txt", "AUTHORS.txt"):
        members[n] = git("show", f"{commit}:third_party/syncthing/2.1.5/{n}")
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, "x", zipfile.ZIP_DEFLATED) as archive:
        for n, data in members.items():
            archive.writestr(n, data)
    with zipfile.ZipFile(output) as archive:
        assert archive.namelist() == list(members)
        assert all(archive.read(n) == data for n, data in members.items())
    print(f"source_commit={commit}\npackage={output.resolve()}\nsha256={hashlib.sha256(output.read_bytes()).hexdigest()}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", default="HEAD")
    parser.add_argument("--relay", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--previous-run", help="Cancel this exact waiting synthetic VM run before starting")
    args = parser.parse_args()
    build(args.revision, args.relay, args.binary, args.output, args.previous_run)
