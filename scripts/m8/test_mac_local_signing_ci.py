"""CI-only local-certificate-positive native exchange. No product activation."""
import os
from pathlib import Path
import secrets
import shutil
import sys
import tempfile

from mac_identity_preflight import PreflightError, checked, run_bounded


def main():
    if (sys.platform != "darwin" or os.environ.get("GITHUB_ACTIONS") != "true"
            or os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted"
            or os.environ.get("RUNNER_OS") != "macOS"):
        raise PreflightError("disposable_macos_ci_required")
    openssl = shutil.which("openssl")
    cargo = shutil.which("cargo")
    if not openssl or not cargo:
        raise PreflightError("ci_signing_tools_missing")
    with tempfile.TemporaryDirectory(prefix="cx-local-signing-") as directory:
        root = Path(directory)
        keychain = root / "fixture.keychain-db"
        password = secrets.token_hex(24)
        created = False
        controls = {
            "CX_NATIVE_TEST_SIGNER": None,
            "CX_NATIVE_TEST_KEYCHAIN": None,
            "CX_NATIVE_TEST_CERT_DER": None,
        }
        saved = {name: os.environ.get(name) for name in controls}
        try:
            config = root / "certificate.cnf"
            config.write_text(
                "[req]\ndistinguished_name=dn\n[dn]\n"
                "[code_signing]\nbasicConstraints=critical,CA:FALSE\n"
                "keyUsage=critical,digitalSignature\nextendedKeyUsage=codeSigning\n",
                encoding="ascii")
            checked(run_bounded, [openssl, "req", "-x509", "-newkey", "rsa:2048",
                    "-nodes", "-keyout", str(root / "key.pem"), "-out", str(root / "cert.pem"),
                    "-days", "1", "-sha256", "-subj", "/CN=Cryptex isolated native CI signer",
                    "-config", str(config), "-extensions", "code_signing"],
                    "generate_ci_identity", 30)
            checked(run_bounded, [openssl, "x509", "-in", str(root / "cert.pem"),
                    "-outform", "DER", "-out", str(root / "cert.der")], "export_ci_certificate")
            fingerprint = checked(run_bounded, [openssl, "x509", "-in", str(root / "cert.pem"),
                    "-noout", "-fingerprint", "-sha1"], "identify_ci_certificate").strip()
            fingerprint = fingerprint.rsplit("=", 1)[-1].replace(":", "").upper()
            if len(fingerprint) != 40 or any(c not in "0123456789ABCDEF" for c in fingerprint):
                raise PreflightError("ci_certificate_selector_invalid")
            checked(run_bounded, [openssl, "pkcs12", "-export", "-in", str(root / "cert.pem"),
                    "-inkey", str(root / "key.pem"), "-out", str(root / "identity.p12"),
                    "-passout", "pass:" + password, "-keypbe", "PBE-SHA1-3DES",
                    "-certpbe", "PBE-SHA1-3DES", "-macalg", "sha1"], "package_ci_identity")
            # Retain cleanup ownership even if create reports a late failure.
            created = True
            checked(run_bounded, ["/usr/bin/security", "create-keychain", "-p", password, str(keychain)],
                    "create_ci_keychain")
            checked(run_bounded, ["/usr/bin/security", "unlock-keychain", "-p", password, str(keychain)],
                    "unlock_ci_keychain")
            checked(run_bounded, ["/usr/bin/security", "import", str(root / "identity.p12"),
                    "-k", str(keychain), "-P", password, "-T", "/usr/bin/codesign"], "import_ci_identity")
            checked(run_bounded, ["/usr/bin/security", "set-key-partition-list",
                    "-S", "apple-tool:,apple:,codesign:", "-s", "-k", password, str(keychain)],
                    "authorize_ci_codesign")
            os.environ.update({
                "CX_NATIVE_TEST_SIGNER": fingerprint,
                "CX_NATIVE_TEST_KEYCHAIN": str(keychain),
                "CX_NATIVE_TEST_CERT_DER": str(root / "cert.der"),
            })
            result = checked(run_bounded, [cargo, "test", "--locked", "-p", "cryptexd",
                    "--lib", "m8_owner_channel::native::tests::native_supervised_channel_roundtrip_and_refusals",
                    "--", "--exact", "--test-threads=1"], "local_certificate_native_exchange", 180)
            if "1 passed; 0 failed" not in result or "0 passed; 0 failed" in result:
                raise PreflightError("native_test_inventory_mismatch")
            print("local_certificate_native_exchange=passed")
        finally:
            for name, value in saved.items():
                if value is None:
                    os.environ.pop(name, None)
                else:
                    os.environ[name] = value
            if created and keychain.exists():
                checked(run_bounded, ["/usr/bin/security", "delete-keychain", str(keychain)],
                        "delete_ci_keychain")
                if keychain.exists():
                    raise PreflightError("ci_keychain_cleanup_failed")
    if root.exists():
        raise PreflightError("ci_fixture_directory_cleanup_failed")
    print("local_certificate_fixture_cleanup=passed")


if __name__ == "__main__":
    try:
        main()
    except PreflightError as error:
        print(str(error), file=sys.stderr)
        sys.exit(2)
