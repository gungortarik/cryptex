"""Read a previous release's committed paired journal with both native release builds."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import zipfile

from previous_daemon import REVISION
from test_owner_transport import run_trial

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


def build(tree, current, target, destination):
    args = ["cargo", "test", "--locked", "-p", "cryptexd", "--lib", "--target-dir", target]
    features = []
    if sys.platform == "darwin":
        features.append("m8-local-startup-check")
    if current:
        features.append("m17-owner-transport")
    if features:
        args.extend(["--features", ",".join(features)])
    args.extend(["--no-run", "--message-format=json"])
    output = subprocess.check_output(args, cwd=tree, text=True, timeout=900)
    artifacts = [item["executable"] for item in map(json.loads, output.splitlines())
                 if item.get("reason") == "compiler-artifact" and item["profile"]["test"]
                 and item["target"]["name"] == "cryptexd" and item.get("executable")]
    if len(artifacts) != 1:
        raise RuntimeError("Expected a single native service test binary")
    shutil.copy2(artifacts[0], destination)


def run_phase(binary, phase, root):
    env = dict(os.environ, CRYPTEX_PAIRED_HANDOFF=str(root), CRYPTEX_PAIRED_PHASE=phase)
    result = subprocess.run([binary, TEST, "--ignored", "--exact", "--test-threads=1"],
                            env=env, stdout=subprocess.PIPE, text=True, timeout=90)
    if result.returncode or f"test {TEST} ... ok" not in result.stdout:
        raise RuntimeError(f"Paired release check failed during {phase}: {result.stdout[-2048:]}")
    print(f"paired_exchange_{phase}=passed", flush=True)


def inventory(root):
    return {p.relative_to(root): hashlib.sha256(p.read_bytes()).digest()
            for p in root.rglob("*") if p.is_file()}


def check_database(path, expected, *, update=False):
    db = sqlite3.connect(path.as_uri() + ("?mode=rw" if update else "?mode=ro"), uri=True)
    try:
        if db.execute("PRAGMA integrity_check").fetchall() != [("ok",)]:
            raise RuntimeError("Cold database failed its integrity check")
        if db.execute("SELECT label, length(data) FROM proof").fetchall() != [(expected, 262144)]:
            raise RuntimeError("Unexpected restored database contents")
        if update:
            db.execute("UPDATE proof SET label = 'candidate'")
            db.commit()
    finally:
        db.close()


def prepare(output):
    if sys.platform not in ("darwin", "win32"):
        raise RuntimeError("Native Mac or Windows required")
    current = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    with tempfile.TemporaryDirectory(prefix="cx17-pair-") as directory:
        work = Path(directory).resolve()
        previous = checkout(work, "a", REVISION)
        candidate = checkout(work, "b", current)
        target = work / "target"
        if sys.platform == "darwin":
            # Compile temporary libtests under a throwaway public certificate pin.
            # This test performs no signing, Keychain reads or engine launch.
            env_pin = work / "pin.der"
            subprocess.run(["/usr/bin/openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes",
                            "-subj", "/CN=Cryptex temporary build pin", "-days", "1", "-outform",
                            "DER", "-out", env_pin, "-keyout", os.devnull], check=True, timeout=30)
            os.environ["CRYPTEX_M8_LOCAL_STARTUP_CERT_DER"] = str(env_pin)
        suffix = ".exe" if sys.platform == "win32" else ""
        output.mkdir(mode=0o700)
        build(previous, False, target, output / ("previous" + suffix))
        build(candidate, True, target, output / ("candidate" + suffix))


def exercise(previous, candidate):
    with tempfile.TemporaryDirectory(prefix="cx17-profile-") as temporary:
        runtime = Path(temporary).resolve()
        run_phase(previous, "write_previous", runtime)
        home = runtime / "native"
        source = runtime / "source"
        receiver = runtime / "receiver"
        # A multi-page synthetic database accompanies the genuine 1.0.1 journal.
        database = Path("index-v2") / "index.db"
        (home / database.parent).mkdir(mode=0o700)
        db = sqlite3.connect(home / database)
        try:
            db.execute("CREATE TABLE proof (label TEXT, data BLOB)")
            db.execute("INSERT INTO proof VALUES ('previous', zeroblob(262144))")
            db.commit()
        finally:
            db.close()
        before = inventory(home)
        histories = [inventory(source), inventory(receiver)]
        run_phase(candidate, "stage_candidate", runtime)
        if inventory(home) != before:
            raise RuntimeError("Staging changed the previous native home")
        retained = runtime / "retained"
        home.rename(retained)
        (runtime / "staged").rename(home)
        run_phase(candidate, "verify_candidate", runtime)
        check_database(home / database, "previous", update=True)
        check_database(home / database, "candidate")
        home.rename(runtime / "rejected")
        retained.rename(home)
        run_phase(previous, "verify_restored", runtime)
        check_database(home / database, "previous")
        if inventory(home) != before or [inventory(source), inventory(receiver)] != histories:
            raise RuntimeError("Rollback changed the previous native home or paired histories")
        print("paired_release_history_and_rollback_bytes_preserved=passed", flush=True)
    run_trial(candidate)


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "prepare":
        prepare(Path(sys.argv[2]).resolve())
    elif len(sys.argv) == 4 and sys.argv[1] == "exercise":
        exercise(*(Path(arg).resolve(strict=True) for arg in sys.argv[2:]))
    else:
        raise SystemExit("Expected prepare OUTPUT or exercise PREVIOUS CANDIDATE")
