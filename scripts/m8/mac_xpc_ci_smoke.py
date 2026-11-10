"""Disposable Mac CI ad-hoc smoke; no owner certificate or feasibility pass."""
import json
import os
from pathlib import Path
import re
import sys

sys.dont_write_bytecode = True
from mac_identity_preflight import checked, run_bounded
from mac_xpc_probe import CLIENT, SERVER, cleanup, exercise, new_private_work, validate_work


def main():
    if sys.platform != "darwin" or os.environ.get("GITHUB_ACTIONS") != "true":
        raise RuntimeError("disposable Mac CI only")
    source = Path(__file__).with_name("mac_xpc_probe.c").resolve()
    work = new_private_work()
    validate_work(work, source.parent)
    os.environ["TMPDIR"] = str(work)
    binaries = {}
    requirements = {}
    report = {"scope": "disposable_ci_adhoc_smoke_only", "private_key_use": "not_requested",
              "production_changed": False, "m8_complete": False}
    try:
        sdk = checked(run_bounded, ["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-path"], "sdk").strip()
        for name, identifier in (("server", SERVER), ("expected", CLIENT), ("wrong_id", CLIENT + ".wrong")):
            binary = work / name
            checked(run_bounded, ["/usr/bin/xcrun", "clang", "-std=c11", "-fblocks", "-Wall", "-Wextra", "-Werror",
                                 "-mmacosx-version-min=12.0", "-isysroot", sdk, str(source), "-framework", "Security",
                                 "-framework", "CoreFoundation", "-o", str(binary)], "compile", 60)
            checked(run_bounded, ["/usr/bin/codesign", "--force", "--timestamp=none", "--sign", "-",
                                 "--identifier", identifier, str(binary)], "adhoc_sign")
            detail = checked(run_bounded, ["/usr/bin/codesign", "-d", "--verbose=4", str(binary)], "adhoc_detail")
            hashes = re.findall(r"^CDHash=([0-9a-f]{40})$", detail, re.M)
            if len(hashes) != 1:
                raise RuntimeError("CI code hash unavailable")
            requirements[name] = 'identifier "' + identifier + '" and cdhash H"' + hashes[0] + '"'
            binaries[name] = binary
        exercise(report, binaries, requirements["expected"], requirements["server"], work, ("wrong_id",))
        cases = {case["name"]: case for case in report["cases"]}
        for name in ("expected_guard", "expected_observer", "expected_after_negatives", "fresh_process", "wrong_id_observer"):
            if cases[name]["verdict"] != "pass":
                raise RuntimeError("CI synthetic observation failed: " + name)
        if cases["origin_endpoint_stored"]["client_result"] != "stored":
            raise RuntimeError("CI endpoint courier failed")
        if cases["forwarded_endpoint_after_origin_exit"]["verdict"] != "signer_only_insufficient":
            raise RuntimeError("CI forwarded endpoint observation differs")
    finally:
        report["temporary_job_cleanup"] = cleanup(work)
        report["server_process_exit"] = "not_independently_observed"
        print(json.dumps(report, indent=2))
        if report["temporary_job_cleanup"] not in ("registration_removed", "not_registered"):
            raise RuntimeError("CI synthetic job cleanup not confirmed")
    # Working files remain private in disposable runner TEMP; no certificate exists.


if __name__ == "__main__":
    main()
