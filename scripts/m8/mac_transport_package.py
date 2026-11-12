"""CI/package input only: exact upstream native Mac members, never extractall."""
import hashlib
import io
import platform
import stat
import urllib.request
import zipfile

from mac_identity_preflight import PreflightError

PINS = {
    "arm64": ("4a3153cce6f3bdc6290824e409b1622c3b78ad8b8a8b2fc846c0061b61e44d4a",
              27811792, "6de41132c8a3fc78fc57486df32f9ac69d6f53db1e1910bb46f548ee7f29725e",
              "660731a8e54dee77597c0526865dc938525fc294"),
    "x86_64": ("f4535e479472a1ae43d3e6ffc4bcfc7651179e948387c08f3696847d142b62c7",
               29591776, "277466c540abc5898c03b3e5e6d6349001483ba40b3ffa5e3782f5ba23f12f7e",
               "1411413e32d8730e78c90156db86cd661bae2f9a"),
}
NOTICES = {
    "LICENSE.txt": (17099, "9221c2f936159b8446d329249fb4c0f25be510f447383a0f13336ac7985668a3"),
    "AUTHORS.txt": (17156, "2c197afd6113ec13ae134a20ce8f101d9812b078c3ef32c42c3bb4cdaf2b96a2"),
}


def selected_members(data, prefix, specifications):
    """Only exact top-level members, with size/type/digest validation before writes."""
    result = {}
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        infos = archive.infolist()
        for name, (size, digest) in specifications.items():
            selected = [entry for entry in infos if entry.filename.casefold() == (prefix + name).casefold()]
            if len(selected) != 1 or selected[0].filename != prefix + name:
                raise PreflightError("transport_archive_member_ambiguous")
            info = selected[0]
            mode = info.external_attr >> 16
            if info.file_size != size or info.is_dir() or stat.S_IFMT(mode) not in (0, stat.S_IFREG):
                raise PreflightError("transport_archive_member_invalid")
            with archive.open(info) as stream:
                raw = stream.read(size + 1)
            if len(raw) != size or hashlib.sha256(raw).hexdigest() != digest:
                raise PreflightError("transport_archive_member_digest")
            result[name] = raw
    return result


def prepare(destination, run, *, machine=None, download=None):
    machine = machine or platform.machine()
    if machine not in PINS:
        raise PreflightError("transport_architecture_unsupported")
    archive_hash, size, binary_hash, cdhash = PINS[machine]
    upstream_arch = "amd64" if machine == "x86_64" else "arm64"
    stem = f"syncthing-macos-{upstream_arch}-v2.1.5"
    url = f"https://github.com/syncthing/syncthing/releases/download/v2.1.5/{stem}.zip"
    if download is None:
        with urllib.request.urlopen(url, timeout=30) as response:
            data = response.read(64 * 1024 * 1024 + 1)
    else:
        data = download(url)
    if len(data) > 64 * 1024 * 1024 or hashlib.sha256(data).hexdigest() != archive_hash:
        raise PreflightError("transport_archive_digest")
    members = selected_members(data, stem + "/", {"syncthing": (size, binary_hash), **NOTICES})
    destination.mkdir(mode=0o700)
    for name, raw in members.items():
        path = destination / name
        with path.open("xb") as stream:
            stream.write(raw)
        path.chmod(0o500 if name == "syncthing" else 0o400)
    # Public codesign tool observes the real SDK choice; do not assume a digest
    # computed from an arbitrary CodeDirectory would be the selected live image.
    text = run(["/usr/bin/codesign", "-d", "--verbose=4", destination / "syncthing"],
               "inspect_pinned_native_transport")
    if [line[7:] for line in text.splitlines() if line.startswith("CDHash=")] != [cdhash]:
        raise PreflightError("transport_native_image_hash")
