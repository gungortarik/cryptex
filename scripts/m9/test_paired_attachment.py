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
        with contextlib.ExitStack() as stack:
            for i, (port, peer_port, gui) in enumerate([(a, b, gui_a), (b, a, gui_b)]):
                root = roots[i]
                cfg = dict(local_id=ids[i], peer_id=ids[1-i], local_ip=LOCAL, peer_ip=LOCAL,
                           port=port, peer_port=peer_port, packet=root / "identity", side=root.name, run=run)
                key, _ = configure(native, root, cfg, gui)
                if stack.enter_context(worker(native, root, gui, key)) != ids[i]:
                    raise RuntimeError("Syncthing identity mismatch")
            result = subprocess.run([binary, "managed_profile::paired_attachment_transfer", "--exact",
                                     "--ignored", "--nocapture"], env=dict(os.environ, CRYPTEX_TEST_WIRE=tmp,
                                     CRYPTEX_TEST_DEVICE_A=ids[0], CRYPTEX_TEST_DEVICE_B=ids[1]),
                                    stdout=subprocess.PIPE, text=True, timeout=480)
            print(result.stdout, flush=True)
            result.check_returncode()
            if "1 passed; 0 failed; 0 ignored" not in result.stdout:
                raise RuntimeError("paired attachment suite did not run")


if __name__ == "__main__":
    main()
