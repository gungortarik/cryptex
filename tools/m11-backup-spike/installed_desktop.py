"""Install an isolated M11 Desktop package on disposable native CI runners."""
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import uuid

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
    artifacts = run("cargo", "test", "--locked", "-p", "cryptexd", "--lib", *features,
                    "--no-run", "--message-format=json", stdout=subprocess.PIPE, text=True)
    binary, = [Path(item["executable"]) for line in artifacts.stdout.splitlines()
               if (item := json.loads(line)).get("executable") and item["target"]["name"] == "cryptexd"]
    triple, = [line.split()[1] for line in run("rustc", "-vV", capture_output=True, text=True).stdout.splitlines()
               if line.startswith("host:")]
    run("cargo", "build", "--locked", "-p", "cryptexd", "--bin", "cryptexd", "--features", "m11-installed-search-check")
    daemon = binary.parent.parent / ("cryptexd" + binary.suffix)
    sidecar = APP / "src-tauri/binaries" / ("cryptexd-" + triple + binary.suffix)
    sidecar.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(daemon, sidecar)
    pnpm = shutil.which("pnpm.cmd" if windows else "pnpm")
    run(pnpm, "--dir", str(APP), "exec", "tauri", "build", "--debug",
        "--features", "m11-installed-search-check", "--bundles", "nsis" if windows else "app",
        "--config", "src-tauri/tauri.m11-search.conf.json")
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
        env = dict(os.environ, CRYPTEX_M11_INSTALLED_DESKTOP=str(desktop))
        run(str(binary), "--exact", TEST, "--ignored", "--nocapture", env=env, timeout=240)


def check(desktop, work):
    for launch in (1, 2):
        env = dict(os.environ, CRYPTEX_FILE_FIXTURE_SOCKET=str(uuid.uuid4()), CRYPTEX_M11_REOPEN_FIXTURE=work)
        run(desktop, env=env, timeout=75)
        print(f"installed_search_launch={launch} both_notes_visible=true", flush=True)


if __name__ == "__main__":
    if sys.argv[1:] == ["install"]:
        install_and_run()
    elif len(sys.argv) == 4 and sys.argv[1] == "check":
        check(*sys.argv[2:])
    else:
        raise SystemExit("expected install or check DESKTOP WORK")
