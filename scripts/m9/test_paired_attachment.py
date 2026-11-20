"""Run the daemon trial over two authenticated, loopback-only Syncthing peers."""
import argparse
import contextlib
import json
import os
from pathlib import Path
import platform
import re
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request
import uuid
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts/m8"))
from lan_cutoff_packet import MAC_HASH, WIN_HASH, check_hash, configure, generate
from mac_transport_package import prepare

LOCAL = "127.0.0.1"


def ports():
    with contextlib.ExitStack() as stack:
        sockets = [stack.enter_context(socket.socket()) for _ in range(4)]
        for sock in sockets:
            sock.bind((LOCAL, 0))
        return [sock.getsockname()[1] for sock in sockets]


@contextlib.contextmanager
def worker(native, root, gui, key):
    env = dict(os.environ, STNOUPGRADE="1", STNODEFAULTFOLDER="1")
    proc = subprocess.Popen([native, "serve", "--home", root / "transport", "--no-browser",
                             "--no-restart", "--no-upgrade"], env=env, stdout=subprocess.DEVNULL,
                            stderr=subprocess.DEVNULL)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def request(path, method):
        req = urllib.request.Request(f"http://{LOCAL}:{gui}/rest/{path}", method=method,
                                     headers={"X-API-Key": key})
        return opener.open(req, timeout=2)

    try:
        deadline = time.monotonic() + 30
        while proc.poll() is None and time.monotonic() < deadline:
            try:
                with request("system/status", "GET") as response:
                    identity = json.load(response)["myID"]
                break
            except OSError:
                time.sleep(.15)
        else:
            raise RuntimeError("Syncthing startup failed")
        yield identity
    finally:
        if proc.poll() is None:
            try:
                with request("system/shutdown", "POST"):
                    pass
            except OSError:
                pass
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait(timeout=10)


@contextlib.contextmanager
def test_process(cmd, env, phase):
    with tempfile.TemporaryFile(mode="w+", encoding="utf-8") as log, subprocess.Popen(
            cmd, env=dict(env, CRYPTEX_TEST_RECEIVER_PHASE=phase), stdout=log) as proc:
        try:
            yield proc, log
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            log.seek(0)
            print(log.read(), flush=True)


def finish_phase(proc, log, receipts):
    if proc.wait(timeout=180):
        raise RuntimeError("paired profile test process failed")
    log.seek(0)
    output = log.read()
    if any(receipt not in output for receipt in ("1 passed; 0 failed; 0 ignored", *receipts)):
        raise RuntimeError("missing paired profile test receipt")
    evidence = re.findall(r"^pending_restore_evidence=([0-9a-f]{64}:[0-9a-f]{64})$", output, re.MULTILINE)
    if len(evidence) != 1:
        raise RuntimeError("missing pending restore evidence")
    return evidence[0]


def build_binary():
    result = subprocess.run(["cargo", "test", "--locked", "-p", "cryptexd", "--lib", "--no-run",
                             "--features", "m8-development-product", "--message-format=json"],
                            cwd=ROOT, stdout=subprocess.PIPE, check=True, text=True)
    binary, = [item["executable"] for item in map(json.loads, result.stdout.splitlines())
               if item.get("executable") and item["target"]["name"] == "cryptexd" and item["profile"]["test"]]
    return binary


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--purge-only", action="store_true")
    parser.add_argument("--native", type=Path)
    parser.add_argument("--test-binary", type=Path)
    args = parser.parse_args()
    binary = args.test_binary or build_binary()
    with tempfile.TemporaryDirectory(prefix="cryptex-m9-transfer-") as tmp:
        base = Path(tmp)
        native = args.native
        if native is None:
            prepare(base / "native", lambda cmd, _: subprocess.check_output(cmd, stderr=subprocess.STDOUT, text=True))
            native = base / "native/syncthing"
        check_hash(native, WIN_HASH if os.name == "nt" else MAC_HASH[platform.machine()])
        roots = [base / "A", base / "B"]
        for root in roots:
            root.mkdir(mode=0o700)
        ids = [generate(native, root / "identity") for root in roots]
        assert ids[0] != ids[1]
        a, b, gui_a, gui_b = ports()
        run = str(uuid.uuid4())
        configs = []
        for i, (port, peer_port, gui) in enumerate([(a, b, gui_a), (b, a, gui_b)]):
            root = roots[i]
            cfg = dict(local_id=ids[i], peer_id=ids[1-i], local_ip=LOCAL, peer_ip=LOCAL,
                       port=port, peer_port=peer_port, packet=root / "identity", side=root.name, run=run)
            key, _ = configure(native, root, cfg, gui)
            configs.append((root, gui, key))

        def start_worker(stack, index):
            if stack.enter_context(worker(native, *configs[index])) != ids[index]:
                raise RuntimeError("Syncthing identity mismatch")

        def start(stack):
            for index in range(len(configs)):
                start_worker(stack, index)

        with contextlib.ExitStack() as workers:
            start(workers)
            cmd = [binary, "managed_profile::paired_attachment_transfer", "--exact", "--ignored", "--nocapture"]
            env = dict(os.environ, CRYPTEX_TEST_WIRE=tmp, CRYPTEX_TEST_DEVICE_A=ids[0], CRYPTEX_TEST_DEVICE_B=ids[1])
            def wait_marker(proc, name, timeout):
                deadline = time.monotonic() + timeout
                while not (base / name).is_file():
                    if proc.poll() is not None or time.monotonic() >= deadline:
                        raise RuntimeError(f"controller did not reach {name}")
                    time.sleep(.15)

            with test_process(cmd, env, "purge") as (test, log):
                wait_marker(test, "partition-purge", 150)
                workers.close()
                (base / "purge-offline").touch()
                wait_marker(test, "reconnect-purge", 90)
                start(workers)
                if test.wait(timeout=150):
                    raise RuntimeError("paired purge reconciliation failed")
                log.seek(0)
                output = log.read()
                if "stale_purge_reconnect=verified stale_replay=refused replica_disk=clean survivors=2 reopened=2" not in output:
                    raise RuntimeError("missing purge reconciliation evidence")
                if "purge_snapshot_restore=verified full=clean projection_only=clean survivors=2" not in output:
                    raise RuntimeError("missing purge snapshot restore evidence")
            if args.purge_only:
                return
            with test_process(cmd, env, "enroll") as (test, log):
                wait_marker(test, "connect", 420)
                workers.close()
                for root in roots:
                    config = root / "transport/config.xml"
                    tree = ET.parse(config)
                    inbox = root / "profile/transport/incoming"
                    assert inbox.is_dir()
                    (inbox / ".stfolder").mkdir(exist_ok=True)
                    folder = ET.SubElement(tree.getroot(), "folder", id=f"m9-{run}", path=str(inbox),
                                           type="sendreceive", rescanIntervalS="1", fsWatcherEnabled="false")
                    for identity in ids:
                        ET.SubElement(folder, "device", id=identity)
                    tree.write(config, encoding="utf-8", xml_declaration=True)
                start(workers)
                wait_marker(test, "disconnect", 130)
                workers.close()
                (base / "offline").touch()
                wait_marker(test, "reconnect", 45)
                # Delay only the forget segment in the sender's test transport.
                # Both signed canonical packs remain untouched.
                delay = roots[0] / "profile/transport/incoming/.stignore"
                with delay.open("x", encoding="ascii") as ignore:
                    ignore.write("00000000000000000003.seg\n")
                start_worker(workers, 0)
                receiver = workers.enter_context(contextlib.ExitStack())
                start_worker(receiver, 1)
                wait_marker(test, "stop-lifecycle-receiver", 420)
                receiver.close()
                (base / "lifecycle-receiver-offline").touch()
                wait_marker(test, "resume-lifecycle-receiver", 120)
                start_worker(receiver, 1)
                before = finish_phase(test, log, (
                    "automatic_attachment_sync=verified",
                    "paired_note_lifecycle=verified receiver_search=visible-hidden-restored",
                ))
            receiver.close()
            with test_process(cmd, env, "reopen") as (test, log):
                wait_marker(test, "release-forget-segment", 150)
                start_worker(receiver, 1)
                delay.unlink()
                wait_marker(test, "partition-note-conflict", 240)
                workers.close()
                (base / "note-conflict-offline").touch()
                wait_marker(test, "reconnect-note-conflict", 120)
                start(workers)
                after = finish_phase(test, log, (
                    "offline_note_lifecycle=verified queued=2 imported=2 identical_replays=2",
                    "reordered_note_lifecycle=verified first=restore deferred=1 converged=true",
                    "corrupt_forget_recovery=verified signature=rejected published_on_rejection=0 corrected_imports=2 replay_added=0",
                    "offline_note_conflict=verified peer_restore=rejected author_forget=applied histories=equal target=hidden replay_added=0",
                    "unconfirmed_peer_purge=refused deliveries=2 history_unchanged=true blobs_unchanged=true",
                ))
            if before != after:
                raise RuntimeError("pending restore or visible notes changed across restart")
            print("pending_restore_restart=verified phases=2 queued_after_restart=1 imported=2 replay_added=0")


if __name__ == "__main__":
    main()
