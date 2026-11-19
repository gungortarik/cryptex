"""Prepare one private, per-device LAN packet for each of two disposable transport IDs."""
import argparse
import hashlib
import io
import ipaddress
import json
from pathlib import Path
import subprocess
import tempfile
import urllib.request
import uuid
import zipfile

from lan_cutoff_packet import WIN_HASH, check_hash, generate
from mac_lan_setup import source_members
from mac_transport_package import NOTICES, PINS, selected_members

ROOT = Path(__file__).resolve().parents[2]


def private_ip(value):
    ip = ipaddress.IPv4Address(value)
    if not any(ip in ipaddress.ip_network(net) for net in ("10.0.0.0/8","172.16.0.0/12","192.168.0.0/16")):
        raise ValueError("private_lan_required")
    return str(ip)


def write_packet(path, members):
    with zipfile.ZipFile(path, "x", compression=zipfile.ZIP_DEFLATED, compresslevel=5) as archive:
        for name, data in members.items():
            entry = zipfile.ZipInfo("Cryptex-Yetki/"+name)
            entry.create_system = 3
            entry.external_attr = (0o100700 if name.endswith(".command") or name.startswith("syncthing-")
                                   else 0o100600) << 16
            archive.writestr(entry, data, compress_type=zipfile.ZIP_DEFLATED)
    with zipfile.ZipFile(path) as archive:
        for name, data in members.items():
            if archive.read("Cryptex-Yetki/"+name) != data:
                raise ValueError("packet_readback")


def packages(artifact, native, revision, out, ips):
    check_hash(native, WIN_HASH)
    rev = subprocess.check_output(["git","rev-parse",revision+"^{commit}"],cwd=ROOT,text=True).strip()
    meta = json.loads((artifact/"provenance.json").read_text(encoding="utf-8-sig"))
    if meta["source"] != rev:
        raise ValueError("runner_source_mismatch")
    runner = artifact/"cutoff-test.exe"
    check_hash(runner,meta["runner_sha256"])
    source = subprocess.check_output(["git","archive","--format=zip",rev],cwd=ROOT)
    with zipfile.ZipFile(io.BytesIO(source)) as archive:
        source_members(archive)
        script=archive.read("scripts/m8/lan_cutoff_packet.py")
        common={name:archive.read("third_party/syncthing/2.1.5/"+name) for name in NOTICES}
    common["lan_cutoff_packet.py"] = script
    run = str(uuid.uuid4())
    if ips[0] == ips[1]:
        raise ValueError("distinct_lan_addresses_required")
    mac = {"source.zip":source}
    for machine,(archive_hash,size,digest,_) in PINS.items():
        arch = "amd64" if machine=="x86_64" else machine
        stem=f"syncthing-macos-{arch}-v2.1.5"
        with urllib.request.urlopen(f"https://github.com/syncthing/syncthing/releases/download/v2.1.5/{stem}.zip",timeout=60) as response:
            data=response.read(64*1024*1024+1)
        if len(data)>64*1024*1024 or hashlib.sha256(data).hexdigest()!=archive_hash:
            raise ValueError("mac_native_archive_digest")
        mac["syncthing-"+machine]=selected_members(data,stem+"/",{"syncthing":(size,digest)})["syncthing"]
    with tempfile.TemporaryDirectory(prefix="cx-cutoff-packet-") as temp:
        homes = [Path(temp)/side for side in ("A","B")]
        ids = [generate(native,home) for home in homes]
        assert ids[0]!=ids[1]
        for i,system in enumerate(("Windows","Darwin")):
            cfg=dict(run=run,source=rev,platform=system,side="AB"[i],local_id=ids[i],peer_id=ids[1-i],
                     local_ip=ips[i],peer_ip=ips[1-i],port=54381+i,peer_port=54382-i)
            members=dict(common)
            if i==0:
                members.update({"cutoff-test.exe":runner.read_bytes(),"syncthing.exe":native.read_bytes(),
                    "WINDOWS-BASLA.cmd":b'@echo off\r\ncd /d "%~dp0"\r\nif not exist packet.json (echo Once ZIP dosyasini Tumunu Ayikla ile acin. & pause & exit /b 1)\r\npy -3 -B lan_cutoff_packet.py\r\npause\r\n'})
                cfg["runner_sha256"]=meta["runner_sha256"]
            else:
                members.update(mac)
                members["MAC-BASLA.command"]=b'#!/bin/sh\ncd "$(dirname "$0")" || exit 1\nexec /usr/bin/python3 -B lan_cutoff_packet.py\n'
                cfg["source_zip_hash"]=hashlib.sha256(source).hexdigest()
            # Transfer credentials are disposable and unique to this packet and side.
            # No signing key used for Cryptex records ever enters either archive.
            for key in ("cert.pem","key.pem"):
                members[key]=(homes[i]/key).read_bytes()
            members["packet.json"]=json.dumps(cfg,indent=2).encode()
            label="Mac" if i else "VM"
            path=out/f"Cryptex-Yetki-{label}-{run[:8]}.zip"
            write_packet(path,members)
            print(path,flush=True)


if __name__=="__main__":
    parser=argparse.ArgumentParser()
    parser.add_argument("--artifact",type=Path,required=True)
    parser.add_argument("--syncthing",type=Path,required=True)
    parser.add_argument("--revision",required=True)
    parser.add_argument("--vm-ip",type=private_ip,required=True)
    parser.add_argument("--mac-ip",type=private_ip,required=True)
    parser.add_argument("--output",type=Path,required=True)
    args=parser.parse_args()
    packages(args.artifact,args.syncthing,args.revision,args.output,[args.vm_ip,args.mac_ip])
