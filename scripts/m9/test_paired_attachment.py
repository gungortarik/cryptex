"""Run the daemon trial over two authenticated, loopback-only Syncthing peers."""
import argparse
import contextlib
import json
import os
from pathlib import Path
import platform
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


def build_binary():
    result = subprocess.run(["cargo", "test", "--locked", "-p", "cryptexd", "--lib", "--no-run",
                             "--features", "m8-development-product", "--message-format=json"],
                            cwd=ROOT, stdout=subprocess.PIPE, check=True, text=True)
    binary, = [item["executable"] for item in map(json.loads, result.stdout.splitlines())
               if item.get("executable") and item["target"]["name"] == "cryptexd" and item["profile"]["test"]]
    return binary


def main():
    parser = argparse.ArgumentParser()
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

        def start(stack):
            for identity, (root, gui, key) in zip(ids, configs):
                if stack.enter_context(worker(native, root, gui, key)) != identity:
                    raise RuntimeError("Syncthing identity mismatch")

        with contextlib.ExitStack() as workers:
            start(workers)
            cmd = [binary, "managed_profile::paired_attachment_transfer", "--exact", "--ignored", "--nocapture"]
            env = dict(os.environ, CRYPTEX_TEST_WIRE=tmp, CRYPTEX_TEST_DEVICE_A=ids[0], CRYPTEX_TEST_DEVICE_B=ids[1])
            with tempfile.TemporaryFile(mode="w+", encoding="utf-8") as log, subprocess.Popen(
                    cmd, env=env, stdout=log) as test:
                def wait_marker(name, timeout):
                    deadline = time.monotonic() + timeout
                    while not (base / name).is_file():
                        if test.poll() is not None or time.monotonic() >= deadline:
                            raise RuntimeError(f"controller did not reach {name}")
                        time.sleep(.15)

                try:
                    wait_marker("connect", 420)
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
                    wait_marker("disconnect", 130)
                    workers.close()
                    (base / "offline").touch()
                    wait_marker("reconnect", 45)
                    start(workers)
                    if test.wait(timeout=240):
                        raise RuntimeError("paired attachment suite failed")
                finally:
                    if test.poll() is None:
                        test.kill()
                        test.wait()
                    log.seek(0)
                    output = log.read()
                    print(output, flush=True)
                if ("1 passed; 0 failed; 0 ignored" not in output
                        or "automatic_attachment_sync=verified" not in output):
                    raise RuntimeError("missing automatic recovery receipt")


if __name__ == "__main__":
    main()
