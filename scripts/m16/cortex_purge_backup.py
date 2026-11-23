"""Run the disposable importer and post-purge recovery harness on Windows or macOS."""
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TEST = "ipc::purge_review::cortex_backup_test::encrypted_restore_after_cortex_purge"


def main():
    features = ["m15-disposable-import", "m11-installed-search-check"]
    build_env = os.environ.copy()
    if sys.platform == "darwin":
        features.append("m8-local-startup-check")
        build_env["CRYPTEX_M8_LOCAL_STARTUP_CERT_DER"] = str(
            ROOT / "services/cryptexd/src/m8_owner_channel/release_inventory/fixtures/signer.der"
        )
    build = subprocess.run(
        ["cargo", "test", "--locked", "-p", "cryptexd", "--features", ",".join(features),
         "--lib", "--bin", "cryptexd", "--no-run", "--message-format=json-render-diagnostics"],
        cwd=ROOT, env=build_env, check=True, stdout=subprocess.PIPE, text=True,
    )
    binaries = {}
    for line in build.stdout.splitlines():
        item = json.loads(line)
        if item.get("executable") and item["target"]["name"] == "cryptexd":
            kind, = item["target"]["kind"]
            binaries[kind] = item["executable"]
    run = subprocess.run(
        [binaries["lib"], TEST, "--exact", "--ignored", "--nocapture"], cwd=ROOT,
        env=dict(os.environ, CRYPTEX_M16_IMPORT_FIXTURE=binaries["bin"]),
        text=True, stdout=subprocess.PIPE, timeout=180,
    )
    print(run.stdout, end="")
    run.check_returncode()
    if f"test {TEST} ... ok" not in run.stdout:
        raise RuntimeError("Cortex purge recovery test did not run")


if __name__ == "__main__":
    main()
