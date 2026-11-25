"""Read a previous release's committed paired journal with both native release builds."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import zipfile

from previous_daemon import REVISION

ROOT = Path(__file__).resolve().parents[2]
TEST = "managed_profile::exchange_install::tests::paired_release_handoff::paired_exchange_release_handoff"


def checkout(work, label, revision):
    tree = work / label
    archive = work / f"{label}.zip"
    actual = subprocess.check_output(["git", "rev-parse", revision + "^{commit}"],
                                     cwd=ROOT, text=True).strip()
    if actual != revision:
        raise RuntimeError("Unexpected source revision")
    subprocess.run(["git", "archive", "--format=zip", "-o", archive, revision], cwd=ROOT, check=True)
    with zipfile.ZipFile(archive) as files:
        files.extractall(tree)
    tests = tree / "services/cryptexd/src/managed_profile/exchange_install"
    (tests / "tests").mkdir(exist_ok=True)
    shutil.copyfile(Path(__file__).with_suffix(".rs"), tests / "tests/paired_release_handoff.rs")
    with (tests / "tests.rs").open("ab") as source:
        source.write(b"\nmod paired_release_handoff;\n")
    return tree


def run_phase(tree, current, phase, root, target):
    args = ["cargo", "test", "--locked", "-p", "cryptexd", "--lib", "--target-dir", target]
    features = []
    if sys.platform == "darwin":
        features.append("m8-local-startup-check")
    if current:
        features.append("m17-owner-transport")
    if features:
        args.extend(["--features", ",".join(features)])
    args.extend([TEST, "--", "--ignored", "--exact", "--test-threads=1"])
    env = dict(os.environ, CRYPTEX_PAIRED_HANDOFF=str(root), CRYPTEX_PAIRED_PHASE=phase)
    result = subprocess.run(args, cwd=tree, env=env, stdout=subprocess.PIPE, text=True, timeout=900)
    if result.returncode or f"test {TEST} ... ok" not in result.stdout:
        raise RuntimeError(f"Paired release check failed during {phase}: {result.stdout[-2048:]}")
    print(f"paired_exchange_{phase}=passed", flush=True)


def inventory(root):
    return {p.relative_to(root): hashlib.sha256(p.read_bytes()).digest()
            for p in root.rglob("*") if p.is_file()}


def main():
    if sys.platform not in ("darwin", "win32"):
        raise RuntimeError("Native Mac or Windows required")
    current = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    with tempfile.TemporaryDirectory(prefix="cx17-pair-") as directory:
        work = Path(directory).resolve()
        previous = checkout(work, "a", REVISION)
        candidate = checkout(work, "b", current)
        runtime = work / "profile"
        runtime.mkdir(mode=0o700)
        target = work / "target"
        if sys.platform == "darwin":
            # Compile temporary libtests under a throwaway public certificate pin.
            # This test performs no signing, Keychain reads or engine launch.
            env_pin = work / "pin.der"
            subprocess.run(["/usr/bin/openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes",
                            "-subj", "/CN=Cryptex temporary build pin", "-days", "1", "-outform",
                            "DER", "-out", env_pin, "-keyout", os.devnull], check=True, timeout=30)
            os.environ["CRYPTEX_M8_LOCAL_STARTUP_CERT_DER"] = str(env_pin)
        run_phase(previous, False, "write_previous", runtime, target)
        home = runtime / "native"
        source = runtime / "source"
        receiver = runtime / "receiver"
        before = inventory(home)
        histories = [inventory(source), inventory(receiver)]
        run_phase(candidate, True, "stage_candidate", runtime, target)
        if inventory(home) != before:
            raise RuntimeError("Staging changed the previous native home")
        retained = runtime / "retained"
        home.rename(retained)
        (runtime / "staged").rename(home)
        run_phase(candidate, True, "verify_candidate", runtime, target)
        home.rename(runtime / "rejected")
        retained.rename(home)
        run_phase(previous, False, "verify_restored", runtime, target)
        if inventory(home) != before or [inventory(source), inventory(receiver)] != histories:
            raise RuntimeError("Rollback changed the previous native home or paired histories")
        print("paired_release_history_and_rollback_bytes_preserved=passed", flush=True)


if __name__ == "__main__":
    main()
