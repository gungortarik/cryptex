"""Collect exact license documents from the Go module versions linked into official binaries."""
import base64
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
from urllib.request import urlopen

NOTICE = re.compile(r"^(?:licen[cs]e|notice|copying|copyright|unlicense)(?:$|[._-])", re.I)


def source_commit(repo, tag):
    commit = subprocess.check_output(["gh", "api", f"repos/{repo}/commits/{tag}", "--jq", ".sha"],
                                     text=True, timeout=30).strip()
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("Expected upstream source commit")
    return commit


def source_file(repo, commit, filename):
    url = f"https://raw.githubusercontent.com/{repo}/{commit}/{filename}"
    with urlopen(url, timeout=30) as response:
        data = response.read(4 * 1024 * 1024 + 1)
    if not 0 < len(data) <= 4 * 1024 * 1024:
        raise ValueError(f"Unexpected upstream file size for {filename}")
    return url, data


def module_ref(record):
    effective = record.get("Replace") or record
    path, version = effective["Path"], effective["Version"]
    if (not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9./_~-]+", path) or ".." in path.split("/")
            or not re.fullmatch(r"v[0-9][A-Za-z0-9.+-]+", version)):
        raise ValueError("Expected a versioned public Go module")
    return path, version


def documents_for(args):
    path, version, checksum, directory = args
    env = dict(os.environ, GOENV="off", GOWORK="off", GOTOOLCHAIN="local",
               GOPROXY="https://proxy.golang.org", GOSUMDB="sum.golang.org", GOPRIVATE="", GONOPROXY="", GONOSUMDB="")
    info = json.loads(subprocess.check_output(["go", "mod", "download", "-json", f"{path}@{version}"],
                                              cwd=directory, env=env, timeout=240))
    if (info.get("Path"), info.get("Version"), info.get("Sum")) != (path, version, checksum):
        raise ValueError(f"Source checksum mismatch for {path}@{version}")
    root = Path(info["Dir"])
    documents = {}
    for item in sorted(root.rglob("*")):
        if not NOTICE.match(item.name) or not item.is_file():
            continue
        if item.is_symlink() or not 0 < item.stat().st_size <= 4 * 1024 * 1024:
            raise ValueError(f"Unexpected notice entry in {path}@{version}")
        documents[item.relative_to(root).as_posix()] = item.read_bytes()
    return path, version, checksum, documents


def main(inspection, output):
    report = json.loads(Path(inspection).read_bytes())
    tag = report["release"]
    if not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+", tag):
        raise ValueError("Expected an official stable release tag")
    repo = "syncthing/syncthing"
    revision = source_commit(repo, tag)
    _, sums = source_file(repo, revision, "go.sum")
    locked = {(p, v): h for p, v, h in (line.split() for line in sums.decode("utf-8").splitlines())}
    modules = {}
    targets = {}
    for target in report["targets"]:
        if not target["linked_modules"]:
            raise ValueError("No linked modules for target")
        targets[target["target"]] = target["archive_sha256"]
        for dep in target["linked_modules"]:
            p, v = module_ref(dep)
            checksum = locked.get((p, v))
            binary_sum = (dep.get("Replace") or dep).get("Sum")
            if not checksum or not re.fullmatch(r"h1:[A-Za-z0-9+/]{43}=", checksum) or (binary_sum and checksum != binary_sum):
                raise ValueError(f"Missing or inconsistent upstream source lock for {p}@{v}")
            modules.setdefault((p, v, checksum), set()).add(target["target"])
    records, texts = [], {}
    with tempfile.TemporaryDirectory() as directory, ThreadPoolExecutor(max_workers=4) as worker:
        for p, v, h, files in worker.map(documents_for, [(*item, directory) for item in sorted(modules)]):
            names = {}
            for filename, data in files.items():
                key = hashlib.sha256(data).hexdigest()
                names[filename] = key
                if key not in texts:
                    texts[key] = base64.b64encode(data).decode("ascii")
            records.append({"module": p, "version": v, "sum": h, "targets": sorted(modules[p, v, h]),
                            "documents": names})
    sources = [source_file(repo, revision, "gui/default/syncthing/core/aboutModalView.html")]
    for go_version in sorted({target["go_version"] for target in report["targets"]}):
        if not re.fullmatch(r"go[0-9]+\.[0-9]+\.[0-9]+", go_version):
            raise ValueError("Expected a released Go toolchain in the official binary")
        go_revision = source_commit("golang/go", go_version)
        sources.extend(source_file("golang/go", go_revision, name) for name in ("LICENSE", "PATENTS"))
    upstream = {}
    for url, data in sources:
        key = hashlib.sha256(data).hexdigest()
        texts[key] = base64.b64encode(data).decode("ascii")
        upstream[url] = key
    missing = [x["module"] + "@" + x["version"] for x in records if not x["documents"]]
    Path(output).write_text(json.dumps({"release": tag, "archives": targets, "modules": records,
                                       "upstream_source": revision, "upstream_documents": upstream,
                                       "documents_base64": texts, "missing_module_notices": missing},
                                      sort_keys=True) + "\n", encoding="utf-8")
    print(f"{len(records)} linked module versions, {len(texts)} distinct notice files, {len(missing)} without notice files")


if __name__ == "__main__":
    main(*sys.argv[1:])
