"""Native release signing for the installed updater trial, in disposable CI accounts only."""
from contextlib import contextmanager
import base64
import os
from pathlib import Path
import hashlib
import json
import shutil
import subprocess
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[2]
WINDOWS = sys.platform == "win32"

from sign_owner_update import sign
from package_owner_updater import digest, package


def verify_delivered_notices(notices, profile, target):
    packages = notices["packages"]
    documents = notices["documents"]
    names = {item["name"] for item in packages}
    if notices["target"] != target or not {"cc", "process-wrap", "sha2"}.issubset(names):
        raise RuntimeError("Delivered notices have the wrong target or dependency list")
    for name in ("ed25519-dalek", "rusqlite"):
        if (name in names) != (profile == "owner-transport-update"):
            raise RuntimeError("Delivered notices do not match the enrolled-profile dependencies")
    if not all(item["documents"] for item in packages):
        raise RuntimeError("Delivered package lacks its notice")
    referenced = {doc["sha256"] for item in packages for doc in item["documents"]}
    if referenced != documents.keys():
        raise RuntimeError("Delivered package references differ from the embedded notice documents")
    for expected, item in documents.items():
        if item.keys() == {"text"}:
            content = item["text"].encode("utf-8")
        elif item.keys() == {"base64"}:
            content = base64.b64decode(item["base64"], validate=True)
        else:
            raise RuntimeError("Unrecognized embedded notice format")
        if not content or hashlib.sha256(content).hexdigest() != expected:
            raise RuntimeError("Delivered notice bytes differ from their digest")


def delivered_updater(image, destination, profile):
    """Exercise the executable and notices that the receiver actually extracts."""
    if os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted" or os.environ.get("GITHUB_ACTIONS") != "true":
        raise RuntimeError("Disposable signing runner required")
    certificate = Path(os.environ["CRYPTEX_UPDATE_SIGNER_DER"])
    pin = digest(certificate)
    approved = digest(image)
    output = destination / "cryptex-local-updater.tar.gz"
    expected = package(image, approved, certificate, pin, output, os.environ.get("CX_NATIVE_TEST_KEYCHAIN"),
                       expected_profile=profile)
    if digest(output) != expected or digest(image) != approved:
        raise RuntimeError("Published archive or reviewed updater changed")
    with tarfile.open(output, "r:gz") as archive:
        if archive.getnames() != [image.name, "notices/rust.json"] or not all(m.isfile() for m in archive):
            raise RuntimeError("Unexpected updater archive members")
        archive.extractall(destination, filter="data")
    tool = destination / image.name
    if subprocess.check_output([tool, "signer-fingerprint"], text=True, timeout=15).strip() != pin:
        raise RuntimeError("Delivered updater has the wrong trust pin")
    notices = json.loads((destination / "notices/rust.json").read_bytes())
    target = "x86_64-pc-windows-msvc" if WINDOWS else "aarch64-apple-darwin"
    verify_delivered_notices(notices, profile, target)
    print("owner_updater_archive_verified=true archive_sha256=" + expected, flush=True)
    return tool


@contextmanager
def signer():
    if os.environ.get("GITHUB_ACTIONS") != "true" or os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted":
        raise RuntimeError("Disposable signing runner required")
    if WINDOWS:
        if not os.environ.get("CX_UPDATE_CI_SIGNER") or not os.environ.get("CRYPTEX_UPDATE_SIGNER_DER"):
            raise RuntimeError("Use the temporary Windows signing fixture")
        yield
    elif sys.platform == "darwin":
        sys.path.insert(0, str(ROOT / "scripts/m8"))
        from test_mac_local_signing_ci import disposable_signing_identity
        with tempfile.TemporaryDirectory() as scratch:
            helper = Path(scratch).resolve() / "owner-cms"
            subprocess.run(["/usr/bin/clang", "-Wall", "-Wextra", "-Werror", ROOT / "scripts/m8/mac_owner_cms.c",
                            ROOT / "services/cryptexd/src/m8_owner_channel/native/inventory.c",
                            "-framework", "Security", "-framework", "CoreFoundation", "-o", helper], check=True, timeout=60)
            subprocess.run(["/usr/bin/codesign", "--sign", "-", "--identifier", "com.cryptex.ci.update-signer",
                            helper], check=True, timeout=30)
            with disposable_signing_identity(helper) as (directory, keychain, _password, _id, _cargo, _openssl):
                subprocess.run(["/usr/bin/security", "set-keychain-settings", "-lut", "3600", keychain], check=True, timeout=15)
                os.environ["CRYPTEX_UPDATE_SIGNER_DER"] = str(directory / "cert.der")
                os.environ["CX_UPDATE_CI_CMS_HELPER"] = str(helper)
                try:
                    yield
                finally:
                    del os.environ["CRYPTEX_UPDATE_SIGNER_DER"], os.environ["CX_UPDATE_CI_CMS_HELPER"]
    else:
        raise RuntimeError("Unsupported signing platform")


def image(path, identifier):
    if WINDOWS:
        subprocess.run(["pwsh", "-NoProfile", "-File", ROOT / "scripts/m17/sign_update_fixture.ps1",
                        "image", path], check=True, timeout=60)
    else:
        subprocess.run(["/usr/bin/codesign", "--force", "--timestamp=none", "--options", "runtime",
                        "--keychain", os.environ["CX_NATIVE_TEST_KEYCHAIN"], "--sign", os.environ["CX_NATIVE_TEST_SIGNER"],
                        "--identifier", identifier, path], check=True, timeout=60)


def receipt(tool, package, output, wrong=False):
    digest = subprocess.check_output([tool, "fingerprint", package], text=True).strip()
    pin = hashlib.sha256(Path(os.environ["CRYPTEX_UPDATE_SIGNER_DER"]).read_bytes()).hexdigest()
    helper = os.environ.get("CX_UPDATE_CI_CMS_HELPER")
    if not wrong:
        sign(tool, package, digest, pin, output, helper)
        return
    # Negative controls run once per build, using the baseline package.
    with tempfile.TemporaryDirectory() as temp:
        scratch = Path(temp)
        refused = scratch / "refused.cms"
        for expected_digest, expected_pin in (("0" * 64, pin), (digest, "0" * 64)):
            try:
                sign(tool, package, expected_digest, expected_pin, refused, helper)
            except subprocess.CalledProcessError:
                pass
            else:
                raise RuntimeError("Signed update with an unapproved package or identity")
            if refused.exists():
                raise RuntimeError("Rejected signing request published a receipt")
        existing = scratch / "existing.cms"
        existing.write_bytes(b"original")
        try:
            sign(tool, package, digest, pin, existing, helper)
        except FileExistsError:
            pass
        else:
            raise RuntimeError("Signer accepted existing output")
        if existing.read_bytes() != b"original":
            raise RuntimeError("Existing output changed")
        content = scratch / "statement"
        content.write_bytes(subprocess.check_output([tool, "statement", package, digest], timeout=180))
        if WINDOWS:
            subprocess.run(["pwsh", "-NoProfile", "-File", ROOT / "scripts/m17/sign_update_fixture.ps1",
                            "wrong-receipt", content, output], check=True, timeout=60)
        else:
            subprocess.run([shutil.which("openssl"), "req", "-x509", "-newkey", "rsa:2048", "-nodes",
                            "-keyout", scratch / "key.pem", "-out", scratch / "cert.pem", "-days", "1",
                            "-subj", "/CN=Cryptex unrelated CI"], stderr=subprocess.DEVNULL, check=True, timeout=30)
            subprocess.run([shutil.which("openssl"), "cms", "-sign", "-binary", "-nodetach", "-md", "sha256",
                            "-in", content, "-signer", scratch / "cert.pem", "-inkey", scratch / "key.pem",
                            "-outform", "DER", "-out", output], check=True, timeout=30)


if __name__ == "__main__":
    image_path, destination = map(Path, sys.argv[1:])
    delivered_updater(image_path, destination, "owner-transport-update")
