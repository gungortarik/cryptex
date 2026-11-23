"""Sign and package an approved Cryptex Mac application using the owner's Keychain."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile

from prepare_mac_inventory import output_directory, prepare, publish
from sign_owner_update import sign


def fingerprint(updater, bundle):
    return subprocess.check_output([updater, "fingerprint", bundle], text=True, timeout=90).strip()


def package(bundle, approved, revision, public_der, pin, updater, cms_helper, output, keychain=None):
    if not re.fullmatch("[0-9a-f]{64}", approved) or not re.fullmatch("[0-9a-f]{40}", revision):
        raise ValueError("Expected the approved package SHA-256 and full source revision")
    der = public_der.read_bytes()
    if not der or hashlib.sha256(der).hexdigest() != pin.lower():
        raise ValueError("Certificate differs from the approved SHA-256")
    if output.exists() or output.is_symlink():
        raise FileExistsError(output)
    parent = output_directory(bundle, output.parent)
    if fingerprint(updater, bundle) != approved:
        raise ValueError("Different unsigned application")
    # codesign selects a Keychain identity by SHA-1. The independent trust pin
    # is SHA-256 over the full DER certificate and is checked again on both roles.
    identity = hashlib.sha1(der).hexdigest().upper()
    with tempfile.TemporaryDirectory(dir=parent) as tmp:
        stage = Path(tmp) / "delivery"
        stage.mkdir(mode=0o700)
        app = stage / "Cryptex.app"
        subprocess.run(["/usr/bin/ditto", bundle, app], check=True, timeout=120)
        if fingerprint(updater, app) != approved:
            raise ValueError("Application changed while staging")
        daemon = app / "Contents/MacOS/cryptexd"
        for image, role in ((daemon, "com.cryptex.daemon"), (app, "com.cryptex.desktop")):
            args = ["/usr/bin/codesign", "--force", "--timestamp=none", "--options", "runtime",
                    "--sign", identity, "--identifier", role]
            if keychain:
                args.extend(["--keychain", keychain])
            subprocess.run([*args, image], check=True, timeout=90)
        signed_image = hashlib.sha256(daemon.with_name("cryptex-desktop").read_bytes()).hexdigest()
        prepare(app, revision, pin, signed_image, cms_helper, stage)
        signed_package = fingerprint(updater, app)
        sign(updater, app, signed_package, pin.lower(), stage / "update.cms", cms_helper)
        info = {"source": revision, "certificate_sha256": pin.lower(), "input_sha256": approved,
                "desktop_sha256": signed_image, "package_sha256": signed_package}
        (stage / "release.json").write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8")
        publish(stage, output)
    return info


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    parser.add_argument("approved_sha256", help="Reviewed unsigned full-package fingerprint")
    parser.add_argument("revision", help="Full reviewed source commit")
    parser.add_argument("public_der", type=Path, help="Public DER code-signing certificate")
    parser.add_argument("certificate_sha256")
    parser.add_argument("output", type=Path, help="New directory for the signed delivery")
    parser.add_argument("--updater", type=Path, required=True, help="Trusted Cryptex updater from the signing machine")
    parser.add_argument("--mac-helper", type=Path, required=True, help="Reviewed Keychain CMS helper")
    parser.add_argument("--keychain", type=Path, help="Optional owner Keychain containing the selected identity")
    args = parser.parse_args()
    print(json.dumps(package(args.bundle.resolve(strict=True), args.approved_sha256, args.revision,
                             args.public_der.resolve(strict=True), args.certificate_sha256,
                             args.updater.resolve(strict=True), args.mac_helper.resolve(strict=True),
                             args.output.absolute(), args.keychain), indent=2))
