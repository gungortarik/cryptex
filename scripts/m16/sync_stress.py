"""Exercise repeated partitions of two enrolled disposable peers on one native host."""
import argparse
from contextlib import ExitStack
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import uuid
import xml.etree.ElementTree as ET

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "scripts/m9"))
from test_paired_attachment import LOCAL, MAC_HASH, WIN_HASH, check_hash, configure, generate, ports, prepare, worker
from lan_cutoff_packet import build_runner

ROUNDS = 6


def exercise(base, native, binary):
    check_hash(native, WIN_HASH if sys.platform == "win32" else MAC_HASH[platform.machine()])
    roots = [base / "A", base / "B"]
    for root in roots:
        root.mkdir(mode=0o700)
    ids = [generate(native, root / "identity") for root in roots]
    if ids[0] == ids[1]:
        raise RuntimeError("duplicate transport identity")

    def phase(name, number):
        env = dict(os.environ, CRYPTEX_TEST_WIRE=str(base), CRYPTEX_TEST_DEVICE_A=ids[0],
                   CRYPTEX_TEST_DEVICE_B=ids[1], CRYPTEX_M16_ROUND=str(number), CRYPTEX_M16_PHASE=name)
        cmd = [binary, "managed_profile::repeated_offline_exchange", "--exact", "--ignored", "--nocapture"]
        result = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=180)
        receipt = f"m16_exchange phase={name} round={number} verified=true"
        if (result.returncode or receipt not in result.stdout.splitlines()
                or "1 passed; 0 failed; 0 ignored" not in result.stdout):
            # The controller runs exclusively against synthetic profiles.
            print(result.stdout, result.stderr, file=sys.stderr)
            raise RuntimeError(f"sync stress failed in {name} round {number}")
        print(receipt, flush=True)

    phase("enroll", 0)
    a, b, gui_a, gui_b = ports()
    run_id = str(uuid.uuid4())
    configs = []
    for i, (port, remote, gui) in enumerate([(a, b, gui_a), (b, a, gui_b)]):
        root = roots[i]
        cfg = dict(local_id=ids[i], peer_id=ids[1-i], local_ip=LOCAL, peer_ip=LOCAL,
                   port=port, peer_port=remote, packet=root / "identity", side=root.name, run=run_id)
        key, _ = configure(native, root, cfg, gui)
        path = root / "transport/config.xml"
        tree = ET.parse(path)
        xml = tree.getroot()
        for folder in xml.findall("folder"):
            xml.remove(folder)
        inbox = root / "profile/transport/incoming"
        (inbox / ".stfolder").mkdir(exist_ok=True)
        folder = ET.SubElement(xml, "folder", id=f"m16-{run_id}", path=str(inbox),
                               type="sendreceive", rescanIntervalS="1", fsWatcherEnabled="false")
        for identity in ids:
            ET.SubElement(folder, "device", id=identity)
        tree.write(path, encoding="utf-8", xml_declaration=True)
        configs.append((root, gui, key))

    for number in range(ROUNDS + 1):
        if number:
            phase("offline", number)
        with ExitStack() as stack:
            for i, config in enumerate(configs):
                if stack.enter_context(worker(native, *config)) != ids[i]:
                    raise RuntimeError("transport started with a different identity")
            phase("online", number)
        # The read runs in a fresh daemon test process with both transports down.
        phase("reopen", number)
    print(f"m16_stress rounds={ROUNDS} verified_notes_per_replica={2 + ROUNDS * 8} peers=2")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--windows-artifact", type=Path)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="cryptex-m16-sync-") as tmp:
        base = Path(tmp)
        if args.windows_artifact and sys.platform == "win32":
            native = args.windows_artifact / "syncthing.exe"
            binary = args.windows_artifact / "cutoff-test.exe"
        elif sys.platform == "darwin" and not args.windows_artifact:
            prepare(base / "native", lambda cmd, _: subprocess.check_output(cmd, stderr=subprocess.STDOUT, text=True))
            native = base / "native/syncthing"
            binary = build_runner(REPO, REPO / "target")
        else:
            parser.error("run on macOS or use the Windows standard-user package")
        exercise(base, native, binary)


if __name__ == "__main__":
    main()
