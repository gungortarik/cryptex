"""Run only disposable cutoff profiles and two pinned, send-only/receive-only LAN folders."""
import contextlib
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request
import uuid
import xml.etree.ElementTree as ET
import zipfile

TEST = "incoming_publication::lan_cutoff::paired_native_exchange"
WIN_HASH = "36a0f7bc372f64fa7cc4f5654fa324c0dd9f7fef2e07565e00c6e1cf73f50344"
MAC_HASH = {"arm64": "6de41132c8a3fc78fc57486df32f9ac69d6f53db1e1910bb46f548ee7f29725e",
            "x86_64": "277466c540abc5898c03b3e5e6d6349001483ba40b3ffa5e3782f5ba23f12f7e"}
ID = re.compile(r"(?:[A-Z2-7]{7}-){7}[A-Z2-7]{7}")


def check_hash(path, digest):
    if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
        raise ValueError("file_digest")


def generate(native, home):
    home.mkdir(mode=0o700, exist_ok=True)
    subprocess.run([native, "generate", "--home", home, "--no-port-probing"], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, timeout=30)
    device = ET.parse(home / "config.xml").getroot().find("device").get("id")
    if not ID.fullmatch(device):
        raise ValueError("native_identity")
    return device


def free_port(host):
    with socket.socket() as sock:
        sock.bind((host, 0))
        return sock.getsockname()[1]


def configure(native, root, cfg, gui):
    home = root / "transport"
    home.mkdir(mode=0o700)
    for name in ("cert.pem", "key.pem"):
        shutil.copyfile(cfg["packet"] / name, home / name)
        (home / name).chmod(0o600)
    if generate(native, home) != cfg["local_id"]:
        raise ValueError("local_transport_identity")
    tree = ET.parse(home / "config.xml")
    xml = tree.getroot()
    for folder in xml.findall("folder"):
        xml.remove(folder)
    for key, value in {"listenAddress": f"tcp://{cfg['local_ip']}:{cfg['port']}",
                       "localAnnounceEnabled": "false", "globalAnnounceEnabled": "false",
                       "relaysEnabled": "false", "natEnabled": "false", "urAccepted": "-1",
                       "autoUpgradeIntervalH": "0", "startBrowser": "false"}.items():
        options = xml.find("options")
        for old in options.findall(key):
            options.remove(old)
        ET.SubElement(options, key).text = value
    xml.find("gui/address").text = f"127.0.0.1:{gui}"
    peer = ET.SubElement(xml, "device", id=cfg["peer_id"], name="Disposable cutoff peer")
    ET.SubElement(peer, "address").text = f"tcp://{cfg['peer_ip']}:{cfg['peer_port']}"
    folders = {}
    for name, side, kind in (("out", cfg["side"], "sendonly"),
                              ("in", "B" if cfg["side"] == "A" else "A", "receiveonly")):
        folder_id = f"cutoff-{cfg['run']}-{side}"
        path = root / name
        path.mkdir()
        (path / ".stfolder").mkdir()
        folder = ET.SubElement(xml, "folder", id=folder_id, path=str(path), type=kind,
                               rescanIntervalS="1", fsWatcherEnabled="false")
        for device in (cfg["local_id"], cfg["peer_id"]):
            ET.SubElement(folder, "device", id=device)
        folders[name] = folder_id
    tree.write(home / "config.xml", encoding="utf-8", xml_declaration=True)
    return xml.findtext("gui/apikey"), folders


def role(root, cfg, native, runner, timeout=420):
    gui = free_port("127.0.0.1")
    key, folders = configure(native, root, cfg, gui)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def get(path):
        req = urllib.request.Request(f"http://127.0.0.1:{gui}/rest/{path}", headers={"X-API-Key": key})
        with opener.open(req, timeout=2) as response:
            return json.load(response)

    with contextlib.ExitStack() as stack:
        def start(args, env, output):
            process = subprocess.Popen(args, cwd=root, env=env, stdout=output, stderr=subprocess.STDOUT)
            def stop():
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=8)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
            stack.callback(stop)
            return process

        env = dict(os.environ, STNOUPGRADE="1", STNODEFAULTFOLDER="1")
        transport = start([native, "serve", "--home", root/"transport", "--no-browser", "--no-restart",
                           "--no-upgrade"], env, subprocess.DEVNULL)
        for _ in range(80):
            if transport.poll() is not None:
                raise RuntimeError("transport_start")
            try:
                status = get("system/status")
            except OSError:
                time.sleep(.25)
                continue
            if status["myID"] != cfg["local_id"]:
                raise RuntimeError("transport_identity")
            break
        else:
            raise RuntimeError("transport_ready_timeout")
        print("Aktarim hazir. Diger cihaz bekleniyor...", flush=True)
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if get("system/connections")["connections"].get(cfg["peer_id"], {}).get("connected"):
                break
            time.sleep(1)
        else:
            raise RuntimeError("peer_connection_timeout")
        print("Diger cihaz baglandi. Iki yon ve yeniden acilis kontrol ediliyor...", flush=True)
        env.update({"CRYPTEX_CUTOFF_"+k: str(v) for k, v in dict(
            SIDE=cfg["side"], BRAIN=cfg["run"], LOCAL_ID=cfg["local_id"], PEER_ID=cfg["peer_id"],
            OUT=root/"out", IN=root/"in").items()})
        with (root/"test.log").open("wb") as log:
            test = start([runner, TEST, "--exact", "--ignored", "--nocapture"], env, log)
            code = test.wait(timeout=420)
        output = (root/"test.log").read_text(errors="replace")
        if code or f"test {TEST} ... ok" not in output or "1 passed; 0 failed" not in output:
            # Surface the bounded test location/phase, never include signing data or note bytes.
            location = re.search(r"(?:src[/\\])?lan_cutoff_test\.rs:(\d+)", output)
            missing = re.search(r"round ([12]) missing ([a-z0-9-]+)", output)
            phase = "-".join(missing.groups()) if missing else "check"
            raise RuntimeError("test_" + phase + "_line_" + (location[1] if location else "unknown"))
        # Syncthing may still be delivering the last acknowledgement after the test exits.
        remaining = sum(p.is_file() and not p.name.startswith(".") for p in (root/"out").iterdir())
        query = urllib.parse.urlencode(dict(folder=folders["out"], device=cfg["peer_id"]))
        deadline = time.monotonic() + 45
        while time.monotonic() < deadline:
            completion = get("db/completion?"+query)
            if completion["globalItems"] >= remaining and completion["completion"] == 100:
                return
            time.sleep(.5)
        # The receiver of the final ack already has direct evidence of delivery of its verdict.
        if cfg["side"] == "A":
            return
        raise RuntimeError("final_ack_delivery")


def build_runner(source, target):
    env = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_PROFILE_TEST_DEBUG="0")
    args = ["cargo", "test", "--locked", "-p", "cryptexd", "--lib", "--no-run", "--message-format=json"]
    if sys.platform == "darwin":
        args += ["--features", "m8-local-startup-check"]
        env["CRYPTEX_M8_LOCAL_STARTUP_CERT_DER"] = str(source/"services/cryptexd/src/m8_owner_channel/release_inventory/fixtures/signer.der")
    output = subprocess.check_output(args, cwd=source, env=env, text=True, timeout=1200)
    binaries = [Path(obj["executable"]) for obj in map(json.loads, output.splitlines())
                if obj["reason"]=="compiler-artifact" and obj["target"]["name"]=="cryptexd" and obj.get("executable")]
    if len(binaries) != 1:
        raise RuntimeError("build_inventory")
    return binaries[0]


def main():
    packet = Path(__file__).resolve().parent
    cfg = json.loads((packet/"packet.json").read_text())
    cfg["packet"] = packet
    system = platform.system()
    report = dict(schema=1, run=cfg["run"], source=cfg["source"], platform=system, passed=False)
    if system != cfg["platform"]:
        raise RuntimeError("wrong_platform")
    try:
        with tempfile.TemporaryDirectory(prefix="cryptex-cutoff-") as temp:
            root = Path(temp).resolve()
            report["stage"] = "payload"
            arch = platform.machine()
            if system=="Darwin":
                native = packet/("syncthing-"+arch)
                check_hash(native, MAC_HASH[arch])
                native.chmod(0o700)
                # Source is bound to the package commit; all build/runtime output goes in temp.
                check_hash(packet/"source.zip",cfg["source_zip_hash"])
                sys.path.insert(0, str(packet/"source.zip")+"/scripts/m8")
                from mac_lan_setup import extract_source, tool
                source = root/"src"
                extract_source(packet/"source.zip",source,cfg["source_zip_hash"])
                os.environ["PATH"] = str(Path(tool("cargo")).parent) + os.pathsep + os.environ.get("PATH", "")
                print("Mac icin test araci derleniyor. Bu pencere acik kalsin.", flush=True)
                report["stage"] = "local_test_build"
                runner = build_runner(source,root/"target")
            else:
                native = packet/"syncthing.exe"
                check_hash(native,WIN_HASH)
                runner = packet/"cutoff-test.exe"
                check_hash(runner,cfg["runner_sha256"])
            report["stage"] = "lan_test"
            role(root,cfg,native,runner)
        report.update(passed=True, stage="both_directions_and_restart")
    except (OSError, KeyError, ValueError, RuntimeError, subprocess.SubprocessError) as exc:
        report["error"] = str(exc) if type(exc) is RuntimeError else type(exc).__name__
    download = Path.home()/"Downloads"
    output = download/f"YETKI-SONUC-{system}-{uuid.uuid4().hex[:12]}.zip"
    with zipfile.ZipFile(output,"x") as archive:
        archive.writestr("result.json", json.dumps(report, indent=2))
    print("GECTI." if report["passed"] else "Tamamlanamadi.", output, flush=True)
    return 0 if report["passed"] else 1


if __name__=="__main__":
    sys.exit(main())
