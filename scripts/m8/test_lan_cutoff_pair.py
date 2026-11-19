"""Exercise both independent controllers across two real Syncthing processes on one CI host."""
import concurrent.futures
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import uuid
import zipfile

from lan_cutoff_packet import WIN_HASH, build_runner, free_port, generate, role, check_hash


def windows_native(folder):
    name = "syncthing-windows-amd64-v2.1.5"
    with urllib.request.urlopen(f"https://github.com/syncthing/syncthing/releases/download/v2.1.5/{name}.zip",timeout=45) as r:
        data = r.read(64*1024*1024+1)
    if len(data)>64*1024*1024:
        raise ValueError("archive_bound")
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        entry=z.getinfo(name+"/syncthing.exe")
        if entry.file_size>64*1024*1024:
            raise ValueError("member_bound")
        binary=z.read(entry)
    if hashlib.sha256(binary).hexdigest()!=WIN_HASH:
        raise ValueError("native_digest")
    target=folder/"syncthing.exe"
    target.write_bytes(binary)
    return target


def smoke(native, runner, work):
    check_hash(native, WIN_HASH if sys.platform=="win32" else
               __import__("lan_cutoff_packet").MAC_HASH[platform.machine()])
    run=str(uuid.uuid4())
    roots=[work/"A",work/"B"]
    for root in roots:
        root.mkdir()
    credentials=[root/"keys" for root in roots]
    ids=[generate(native,path) for path in credentials]
    ports=[free_port("127.0.0.1") for _ in roots]
    while ports[1]==ports[0]:
        ports[1]=free_port("127.0.0.1")
    def worker(i):
        try:
            role(roots[i],dict(packet=credentials[i], run=run, side="AB"[i],
                 local_ip="127.0.0.1",peer_ip="127.0.0.1", port=ports[i],peer_port=ports[1-i],
                 local_id=ids[i], peer_id=ids[1-i]),native,runner,timeout=40)
        except Exception:
            log=roots[i]/"test.log"
            if log.exists():
                print(log.read_text(errors="replace")[-3500:],flush=True)
            raise
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        list(pool.map(worker,range(2)))


if __name__=="__main__":
    repo=Path(__file__).resolve().parents[2]
    with tempfile.TemporaryDirectory(prefix="cx-cutoff-smoke-") as tmp:
        work=Path(tmp).resolve()
        if sys.platform=="darwin":
            from mac_transport_package import prepare
            def codesign(args, stage):
                return subprocess.check_output(args,stderr=subprocess.STDOUT,text=True)
            prepare(work/"package", codesign)
            native=work/"package/syncthing"
        else:
            native=windows_native(work)
        runner=build_runner(repo, repo/"target")
        smoke(native,runner,work)
        if sys.platform=="win32" and "GITHUB_OUTPUT" in os.environ:
            dest=Path(os.environ["RUNNER_TEMP"])/"cutoff-runner"
            dest.mkdir()
            shutil.copyfile(runner,dest/"cutoff-test.exe")
            (dest/"provenance.json").write_text(json.dumps(dict(source=os.environ["GITHUB_SHA"],
                runner_sha256=hashlib.sha256(runner.read_bytes()).hexdigest())))
