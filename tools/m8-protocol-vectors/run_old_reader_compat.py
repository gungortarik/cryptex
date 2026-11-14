"""Drive unmodified historical product services using a daemon-owned test fixture."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
SOURCE = "ae61603b517f68b63d559ccb5ee77d744176e6a6"


def run(*args, cwd):
    return subprocess.run(args, cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--old-root", type=Path, required=True)
    args = parser.parse_args()
    old = args.old_root.resolve(strict=True)
    assert old != ROOT and old.name == ".old-product-reader"
    assert run("git", "rev-parse", "HEAD", cwd=old) == SOURCE
    run("git", "diff", "--exit-code", "HEAD", "--", ".", cwd=old)
    harness = ROOT / "tools/m8-protocol-vectors/old_reader_compat.rs"
    fixture = ROOT / "crates/events/tests/fixtures/m8-old-reader-v1-v3.json"
    target = old / "services/cryptexd/tests"
    target.mkdir(exist_ok=True)
    for destination, source in [(target / "m8_old_reader_compat.rs", harness),
                                (target / "m8_old_reader_fixture.json", fixture)]:
        with destination.open("xb") as output:
            output.write(source.read_bytes())
    env = dict(os.environ, CRYPTEX_OLD_READER_SOURCE=SOURCE)
    subprocess.run(["cargo", "+1.98.1", "test", "--locked", "-p", "cryptexd", "--test",
                    "m8_old_reader_compat", "old_product_holds_signed_v3_without_mutation", "--",
                    "--exact", "--nocapture", "--test-threads=1"], cwd=old, env=env, check=True)
    run("git", "diff", "--exit-code", "HEAD", "--", ".", cwd=old)
    actual = set(run("git", "ls-files", "--others", "--exclude-standard", cwd=old).splitlines())
    assert actual == {"services/cryptexd/tests/m8_old_reader_compat.rs", "services/cryptexd/tests/m8_old_reader_fixture.json"}
    receipt = {"source_commit": run("git", "rev-parse", "HEAD", cwd=ROOT),
        "historical_product_commit": SOURCE, "historical_tracked_source_unchanged": True,
        "harness_sha256": hashlib.sha256(harness.read_bytes()).hexdigest(),
        "fixture_sha256": hashlib.sha256(fixture.read_bytes()).hexdigest(),
        "outcome": "passed", "scope": "historical product-service Brain guard; not installed old Desktop/secure-store acceptance",
        "positive_baseline": "v1 only; older v2-semantic hold is not credited as a v3 refusal",
        "guarded_surfaces": ["search", "timeline", "detail", "vault", "rebuild"],
        "canonical_bytes_database_state_and_vault_tree_preserved": True}
    output = Path(os.environ["RUNNER_TEMP"]) / "m8-old-reader-result.json"
    output.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8", newline="\n")
    print("historical_product_source_and_fixture_verified=passed")


if __name__ == "__main__":
    main()
