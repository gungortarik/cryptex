#!/usr/bin/python3
"""One new isolated combined-loss test; aggregate-only owner receipt."""
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import uuid
import zipfile

from mac_identity_preflight import PreflightError, run_bounded
from mac_combined_loss_check import check_combined_loss


def main():
    os.umask(0o077)
    package = Path(__file__).resolve().parent
    meta = json.loads((package / "fixture.json").read_text(encoding="utf-8"))
    report = {"source": meta["source"], "outcome": "incomplete",
        "scope": "combined supervisor-loss exclusion under fixture-only containment; not production orphan cleanup"}
    print("Birlikte süreç kaybı testi başladı. Pencereyi açık bırak.\n"
          "Test yalnızca kendi geçici süreçlerini ve profilini kullanır.", flush=True)
    if sys.platform != "darwin" or platform.machine() != meta["architecture"]:
        report["outcome"] = "platform_mismatch"
    elif datetime.now(timezone.utc) >= datetime.fromisoformat(meta["expires_utc"]):
        report["outcome"] = "fixture_expired"
    else:
        root = Path(tempfile.mkdtemp(prefix="cryptex-owner-combined-")).resolve()
        def run(args, stage, timeout=30):
            outcome = run_bounded([str(arg) for arg in args], timeout)
            if outcome.code or outcome.timed_out or outcome.limited or outcome.pipe_failed:
                raise PreflightError(stage + "_failed")
            return outcome.output
        try:
            check_combined_loss(package / "positive.app/Contents/MacOS/cryptexd", root / "transport", run)
            report["combined_supervisor_loss_and_refusal_verified"] = True
            report["credential_cleanup_verified"] = True
            shutil.rmtree(root)
            report["runtime_cleanup_verified"] = True
            report["outcome"] = "passed"
        except PreflightError as error:
            report["outcome"] = "failed"
            report["diagnostic"] = str(error)
        except (OSError, ValueError, TypeError, KeyError, subprocess.SubprocessError):
            report["outcome"] = "collector_incomplete"
        finally:
            if report.get("runtime_cleanup_verified") is not True:
                print(f"Test kaydı inceleme için korundu: {root}", flush=True)
    output = package / ("MAC-BIRLIKTE-SONUC-" + uuid.uuid4().hex[:8] + ".zip")
    with zipfile.ZipFile(output, "x") as archive:
        archive.writestr("result.json", json.dumps(report, indent=2) + "\n")
    print(f"Sonuç: {report['outcome']}\nPaylaşılacak dosya: {output}", flush=True)
    return 0 if report["outcome"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
