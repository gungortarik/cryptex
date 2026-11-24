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


def release_notice(tag, target, modules, texts, source_notices):
    """Plain-text distribution copy; keep each original document's bytes intact."""
    target_name = target["target"]
    headings = [f"Syncthing {tag} — {target_name}",
                f"Official archive SHA-256: {target['archive_sha256']}",
                f"Executable SHA-256: {target['executable_sha256']}", "", "Linked Go modules"]
    documents = {}

    def include(label, data, expected):
        if hashlib.sha256(data).hexdigest() != expected:
            raise ValueError(f"Notice hash mismatch for {label}")
        if expected not in documents:
            documents[expected] = ([], data)
        documents[expected][0].append(label)

    for filename in ("LICENSE.txt", "AUTHORS.txt"):
        record = target["upstream_notices"][filename]
        data = base64.b64decode(record["contents_base64"], validate=True)
        if len(data) != record["bytes"]:
            raise ValueError(f"Upstream notice size mismatch for {filename}")
        include(f"Syncthing {tag}/{filename}", data, record["sha256"])

    for url, data in source_notices:
        key = hashlib.sha256(data).hexdigest()
        include(url, data, key)
    for module in modules:
        if target_name not in module["targets"]:
            continue
        label = f"{module['module']}@{module['version']}"
        headings.append(label)
        if not module["documents"]:
            headings.append("  No notice file found in the inspected module source.")
        for name, key in sorted(module["documents"].items()):
            include(f"{label}/{name}", base64.b64decode(texts[key], validate=True), key)
    chunks = ["\n".join(headings).encode("utf-8")]
    for key, (labels, data) in documents.items():
        chunks.append(("\n".join(labels) + f"\nSHA-256: {key}\n\n").encode("utf-8") + data)
    return b"\n\n".join(chunks) + b"\n"


def main(inspection, output, distribution=None):
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
    about = source_file(repo, revision, "gui/default/syncthing/core/aboutModalView.html")
    sources = [about]
    go_notices = {}
    for go_version in sorted({target["go_version"] for target in report["targets"]}):
        if not re.fullmatch(r"go[0-9]+\.[0-9]+\.[0-9]+", go_version):
            raise ValueError("Expected a released Go toolchain in the official binary")
        go_revision = source_commit("golang/go", go_version)
        go_notices[go_version] = [source_file("golang/go", go_revision, name) for name in ("LICENSE", "PATENTS")]
        sources.extend(go_notices[go_version])
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
    if distribution is not None:
        from scan_transport import TARGETS
        if sorted(t["target"] for t in report["targets"]) != sorted(TARGETS):
            raise ValueError("Expected both official release targets")
        rendered = {t["target"]: release_notice(tag, t, records, texts, [about, *go_notices[t["go_version"]]])
                    for t in report["targets"]}
        directory = Path(distribution)
        directory.mkdir()
        for target, contents in rendered.items():
            (directory / f"Syncthing-{target}-NOTICES.txt").write_bytes(contents)


if __name__ == "__main__":
    main(*sys.argv[1:])
