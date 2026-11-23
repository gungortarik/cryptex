"""Native release signing for the installed updater trial, in disposable CI accounts only."""
from contextlib import contextmanager
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
WINDOWS = sys.platform == "win32"


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
        with disposable_signing_identity() as (directory, _keychain, _password, _id, _cargo, _openssl):
            subprocess.run(["/usr/bin/security", "set-keychain-settings", "-lut", "3600", _keychain], check=True, timeout=15)
            os.environ["CRYPTEX_UPDATE_SIGNER_DER"] = str(directory / "cert.der")
            try:
                yield
            finally:
                del os.environ["CRYPTEX_UPDATE_SIGNER_DER"]
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
    arch = {"AMD64": "x86_64", "x86_64": "x86_64", "arm64": "aarch64"}[platform.machine()]
    message = f"cryptex.owner-package.v1\nos={'windows' if WINDOWS else 'macos'}\narch={arch}\nsha256={digest}\n"
    with tempfile.TemporaryDirectory() as temp:
        content = Path(temp) / "statement"
        content.write_bytes(message.encode("ascii"))
        if WINDOWS:
            subprocess.run(["pwsh", "-NoProfile", "-File", ROOT / "scripts/m17/sign_update_fixture.ps1",
                            "wrong-receipt" if wrong else "receipt", content, output], check=True, timeout=60)
        else:
            key_dir = Path(os.environ["CRYPTEX_UPDATE_SIGNER_DER"]).parent
            if wrong:
                key_dir = Path(temp)
                subprocess.run([shutil.which("openssl"), "req", "-x509", "-newkey", "rsa:2048", "-nodes",
                                "-keyout", key_dir / "key.pem", "-out", key_dir / "cert.pem", "-days", "1",
                                "-subj", "/CN=Cryptex unrelated CI"], stderr=subprocess.DEVNULL, check=True, timeout=30)
            subprocess.run([shutil.which("openssl"), "cms", "-sign", "-binary", "-nodetach", "-md", "sha256",
                            "-in", content, "-signer", key_dir / "cert.pem", "-inkey", key_dir / "key.pem",
                            "-outform", "DER", "-out", output], check=True, timeout=30)
