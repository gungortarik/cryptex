"""CI-only local-certificate-positive native exchange. No product activation."""
import os
from pathlib import Path
import secrets
import shlex
import shutil
import sys
import tempfile

sys.dont_write_bytecode = True
from mac_identity_preflight import PreflightError, checked, run_bounded
from mac_local_startup_ci import run_real_startup_checks


def main():
    if (sys.platform != "darwin" or os.environ.get("GITHUB_ACTIONS") != "true"
            or os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted"
            or os.environ.get("RUNNER_OS") != "macOS"):
        raise PreflightError("disposable_macos_ci_required")
    openssl = shutil.which("openssl")
    cargo = shutil.which("cargo")
    rustc = shutil.which("rustc")
    if not openssl or not cargo or not rustc:
        raise PreflightError("ci_signing_tools_missing")
    with tempfile.TemporaryDirectory(prefix="cx-local-signing-") as directory:
        root = Path(directory)
        keychain = root / "fixture.keychain-db"
        password = secrets.token_hex(24)
        created = False
        search_attempted = False
        original_search = shlex.split(checked(run_bounded,
            ["/usr/bin/security", "list-keychains", "-d", "user"], "capture_ci_search_list"))
        if any(not Path(path).is_absolute() for path in original_search):
            raise PreflightError("ci_search_list_invalid")
        controls = {
            "CX_NATIVE_TEST_SIGNER": None,
            "CX_NATIVE_TEST_KEYCHAIN": None,
            "CX_NATIVE_TEST_CERT_DER": None,
            "RUSTC": None,
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
            search_attempted = True
            checked(run_bounded, ["/usr/bin/security", "list-keychains", "-d", "user", "-s",
                    *original_search, str(keychain)], "include_ci_signing_keychain")
            identities = checked(run_bounded, ["/usr/bin/security", "find-identity",
                    "-p", "codesigning", str(keychain)], "verify_ci_signing_identity")
            if fingerprint not in identities.upper():
                raise PreflightError("generated_ci_signing_identity_unavailable")
            os.environ.update({
                "CX_NATIVE_TEST_SIGNER": fingerprint,
                "CX_NATIVE_TEST_KEYCHAIN": str(keychain),
                "CX_NATIVE_TEST_CERT_DER": str(root / "cert.der"),
                # run_bounded intentionally uses a minimal system PATH.
                "RUSTC": rustc,
            })
            args = [cargo, "test", "--locked", "-p", "cryptexd",
                    "--lib", "m8_owner_channel::native::tests::native_supervised_channel_roundtrip_and_refusals",
                    "--", "--exact", "--test-threads=1"]
            outcome = run_bounded(args, 180)
            if outcome.code or outcome.timed_out or outcome.limited or outcome.pipe_failed:
                # Disposable fixture diagnostics only. Never print private
                # material, passwords or a reusable certificate selector.
                diagnostic = outcome.output.replace(password, "[redacted]").replace(
                    fingerprint, "[fixture]").replace(str(root), "[fixture-root]")
                print(diagnostic[:8192], file=sys.stderr)
            result = checked(lambda *_: outcome, [], "local_certificate_native_exchange", 180)
            if "1 passed; 0 failed" not in result or "0 passed; 0 failed" in result:
                raise PreflightError("native_test_inventory_mismatch")
            print("local_certificate_native_exchange=passed")
            run_real_startup_checks(root, checked, run_bounded, cargo, openssl,
                                    keychain, password, fingerprint)
        finally:
            for name, value in saved.items():
                if value is None:
                    os.environ.pop(name, None)
                else:
                    os.environ[name] = value
            try:
                if search_attempted:
                    checked(run_bounded, ["/usr/bin/security", "list-keychains", "-d", "user", "-s",
                            *original_search], "restore_ci_search_list")
            finally:
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
    except Exception:
        # Provisioning argv can contain disposable keychain/P12 passwords.
        # Cleanup has already run through finally/context exit.
        print("local_certificate_ci_internal_failure", file=sys.stderr)
        sys.exit(2)
