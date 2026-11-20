"""Bidirectional hosted-OS backup transfer. All credentials belong to this run."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "m11-backup-spike"))
from daemon_roundtrip import BRAIN_IDENTITY, envelope, unpack, write_generation
from rehearse import FIXTURE, NODE


def contents(platform):
    fixture = json.loads(FIXTURE.read_bytes())
    wire = fixture["baseline_wire"].encode("utf-8")
    stream = fixture["device_id"].encode().hex()
    blob = f"Cryptex M11 disposable {platform} cross-platform backup".encode()
    identity = json.dumps({"schema_version": 1, "brain_id": fixture["brain_id"],
                           "created_at": "2026-01-01T00:00:00Z"}, indent=2).encode() + b"\n"
    return {
        BRAIN_IDENTITY: identity,
        f"canonical/events/{stream}/00000000000000000000.seg": struct.pack("<4sHI", b"CXE1", 1, len(wire)) + wire,
        "canonical/blobs/" + hashlib.sha256(blob).hexdigest(): blob,
    }


def setup(root):
    public, private = root / "public", root / "private"
    public.mkdir(parents=True)
    private.mkdir(mode=0o700)
    for role in ("recipient", "writer"):
        subprocess.run(NODE + ["keygen", public / (role + ".asc"), private / (role + ".sec")],
                       check=True, timeout=30)


def send(root, platform):
    profile = root / "source"
    for name, data in contents(platform).items():
        path = profile / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    private = root / "private"
    shutil.copyfile(root / "peer-public/recipient.asc", private / "recovery.pub")
    output = root / "outgoing"
    output.mkdir()
    write_generation(profile, private, output / "backup.pgp")


def receive(root, peer):
    verified = root / "verified.zip"
    envelope("decrypt-verify", [root / "private/recipient.sec", root / "peer-public/writer.asc"],
             root / "incoming/backup.pgp", verified)
    expected = contents(peer)
    restore = root / "restored"
    unpack(verified, restore, hashlib.sha256(expected[BRAIN_IDENTITY]).digest())
    if {p.relative_to(restore).as_posix(): p.read_bytes() for p in restore.rglob("*") if p.is_file()} != expected:
        raise ValueError("Restored peer inventory mismatch")
    print(f"m11_cross_restore=passed from={peer} members={len(expected)} signer=verified")


def fetch(root, kind, peer):
    artifact = f"m11-{kind}-{peer}-{os.environ['GITHUB_RUN_ATTEMPT']}"
    run = os.environ["GITHUB_RUN_ID"]
    api = f"repos/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{run}/artifacts?per_page=100"
    deadline = time.monotonic() + 480
    while time.monotonic() < deadline:
        listing = subprocess.run(["gh", "api", api], check=True, capture_output=True, timeout=30)
        if any(a["name"] == artifact and not a["expired"] for a in json.loads(listing.stdout)["artifacts"]):
            destination = root / ("peer-public" if kind == "public" else "incoming")
            with tempfile.TemporaryDirectory(dir=root) as tmp:
                downloaded = Path(tmp) / "artifact"
                result = subprocess.run(["gh", "run", "download", run, "--name", artifact, "--dir", downloaded],
                                        capture_output=True, timeout=60)
                if result.returncode == 0:
                    downloaded.rename(destination)
                    return
                # The list and download endpoints can briefly disagree just
                # after upload. Other errors (including integrity failures) stop the job.
                if b"no artifact matches" not in result.stderr.lower():
                    raise RuntimeError(f"Peer artifact download failed ({result.returncode}): {artifact}")
        time.sleep(5)
    raise TimeoutError(f"Peer artifact unavailable: {artifact}")


if __name__ == "__main__":
    action, path, *args = sys.argv[1:]
    {"setup": setup, "send": send, "receive": receive, "fetch": fetch}[action](Path(path), *args)
