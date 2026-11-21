"""Install an isolated M11 Desktop package on disposable native CI runners."""
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import uuid
from packaged_codec import stage as stage_codec

ROOT = Path(__file__).resolve().parents[2]
APP = ROOT / "apps/desktop"
TEST = "knowledge_materialization::pairing_recovery_tests::m11_backup_read::restored_note_and_attachment_pass_guarded_reads"


def run(*args, **kwargs):
    return subprocess.run(args, check=True, cwd=ROOT, **kwargs)


def install_and_run():
    if os.environ.get("GITHUB_ACTIONS") != "true" or sys.platform not in ("win32", "darwin"):
        raise RuntimeError("package installation requires a disposable native CI runner")
    windows = sys.platform == "win32"
    features = [] if windows else ["--features", "m8-local-startup-check"]
    resources = APP / "src-tauri/binaries"
    pins = stage_codec(resources / "m11-codec")
    build_env = dict(os.environ, CRYPTEX_M11_PACKAGE_CODEC_DIGESTS=json.dumps(pins))
    run("cargo", "test", "--locked", "-p", "cryptexd", "--bin", "cryptexd", *features,
        "tests::first_device_destination_starts_blank_and_preserves_existing_profiles",
        "--", "--exact", env=build_env)
    artifacts = run("cargo", "test", "--locked", "-p", "cryptexd", "--lib", *features,
                    "--no-run", "--message-format=json", stdout=subprocess.PIPE, text=True, env=build_env)
    binary, = [Path(item["executable"]) for line in artifacts.stdout.splitlines()
               if (item := json.loads(line)).get("executable") and item["target"]["name"] == "cryptexd"]
    triple, = [line.split()[1] for line in run("rustc", "-vV", capture_output=True, text=True).stdout.splitlines()
               if line.startswith("host:")]
    daemon_features = "m11-installed-search-check" if windows else "m11-installed-search-check,m8-local-startup-check"
    run("cargo", "build", "--locked", "-p", "cryptexd", "--bin", "cryptexd", "--features", daemon_features, env=build_env)
    daemon = binary.parent.parent / ("cryptexd" + binary.suffix)
    sidecar = resources / ("cryptexd-" + triple + binary.suffix)
    shutil.copy2(daemon, sidecar)
    pnpm = shutil.which("pnpm.cmd" if windows else "pnpm")
    run(pnpm, "--dir", str(APP), "exec", "tauri", "build", "--debug",
        "--features", "m11-installed-search-check", "--bundles", "nsis" if windows else "app",
        "--config", "src-tauri/tauri.m11-search.conf.json", env=build_env)
    bundles = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "debug/bundle"
    with tempfile.TemporaryDirectory(prefix="cryptex-m11-install-", dir=os.environ["RUNNER_TEMP"]) as directory:
        install = Path(directory)
        if windows:
            installer, = (bundles / "nsis").glob("*.exe")
            run(str(installer), "/S", "/D=" + str(install), timeout=180)
            desktop = install / "cryptex-desktop.exe"
        else:
            bundle, = (bundles / "macos").glob("*.app")
            installed = install / "Applications" / bundle.name
            installed.parent.mkdir()
            run("ditto", str(bundle), str(installed))
            with (installed / "Contents/Info.plist").open("rb") as info:
                name = plistlib.load(info)["CFBundleExecutable"]
            desktop = installed / "Contents/MacOS" / name
        if not desktop.is_file() or not desktop.with_name(sidecar.name.replace("-" + triple, "")).is_file():
            raise RuntimeError("installed Desktop or bundled recovery reader missing")
        codec = (desktop.parent if windows else installed / "Contents/Resources") / "m11-codec"
        for role, name in {"runtime": "node.exe" if windows else "node",
                           "entry": "encrypt.cjs", "restore": "restore.cjs",
                           "zip": "fflate.cjs", "openpgp": "openpgp.min.cjs"}.items():
            with (codec / name).open("rb") as installed_file:
                if hashlib.file_digest(installed_file, "sha256").hexdigest() != pins[role]:
                    raise RuntimeError(f"Installed backup {role} differs from the build input")
        env = dict(os.environ, CRYPTEX_M11_INSTALLED_DESKTOP=str(desktop),
                   CRYPTEX_M11_TEST_INSTALLED_CODEC=str(codec))
        run(str(binary), "--exact", TEST, "--ignored", "--nocapture", env=env, timeout=420)
        run(str(binary), "--exact", "knowledge_materialization::pairing_recovery_tests::m11_backup_read::installed_writer_handoff",
            "--ignored", "--nocapture", env=env, timeout=180)
        run(str(binary), "--exact", "ipc::purge_review::backup_test::verified_backup_preserves_purge_and_unaffected_notes",
            "--ignored", "--nocapture", env=env, timeout=180)
        print("m11_installed_codec=passed public_recipient=true restored_with_daemon_signature=true", flush=True)


def check(desktop, work):
    for launch in (1, 2):
        env = dict(os.environ, CRYPTEX_FILE_FIXTURE_SOCKET=str(uuid.uuid4()), CRYPTEX_M11_REOPEN_FIXTURE=work)
        run(desktop, env=env, timeout=75)
        print(f"installed_search_launch={launch} both_notes_visible=true", flush=True)


if __name__ == "__main__":
    if sys.argv[1:] == ["install"]:
        install_and_run()
    elif len(sys.argv) == 4 and sys.argv[1] == "export":
        run(sys.argv[2], env=dict(os.environ, CRYPTEX_FILE_FIXTURE_SOCKET=str(uuid.uuid4()),
                                  CRYPTEX_M11_REOPEN_FIXTURE=sys.argv[3]), timeout=85)
    elif len(sys.argv) == 4 and sys.argv[1] == "check":
        check(*sys.argv[2:])
    else:
        raise SystemExit("expected install or check DESKTOP WORK")
