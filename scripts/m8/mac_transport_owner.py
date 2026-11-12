#!/usr/bin/python3
"""One-click local native pairing/transport fixture; aggregate-only receipt."""
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
from mac_transport_check import check_native_pairing, check_parent_loss


def main():
    os.umask(0o077)
    package = Path(__file__).resolve().parent
    meta = json.loads((package / "fixture.json").read_text(encoding="utf-8"))
    report = {"source": meta["source"], "outcome": "incomplete",
              "scope": "isolated real local engines and signed native pairing; not cross-device sync"}
    print("Mac aktarım kimliği ve eşleştirme testi başladı. Pencereyi açık bırak.\n"
          "Test kendi geçici profillerini kullanır; kişisel verileri aktarmaz.", flush=True)
    if sys.platform != "darwin" or platform.machine() != meta["architecture"]:
        report["outcome"] = "platform_mismatch"
    elif datetime.now(timezone.utc) >= datetime.fromisoformat(meta["expires_utc"]):
        report["outcome"] = "fixture_expired"
    else:
        temp = Path(tempfile.mkdtemp(prefix="cryptex-owner-transport-")).resolve()

        def run(args, stage, timeout=30):
            result = run_bounded([str(arg) for arg in args], timeout)
            if result.code or result.timed_out or result.limited or result.pipe_failed:
                raise PreflightError(stage + "_failed")
            return result.output

        try:
            binary = package / "positive.app/Contents/MacOS"
            output = run([binary / "cryptexd", "--m8-local-transport-driver", temp / "identity"],
                         "managed_identity_restart", 180)
            if not all(marker in output.splitlines() for marker in (
                    "mac_transport_foreign_socket_no_credentials=passed",
                    "mac_transport_owned_identity_restart=passed",
                    "mac_transport_helper_loss_cleanup=passed",
                    "mac_transport_missing_key_no_regeneration=passed")):
                raise PreflightError("managed_identity_marker_missing")
            report["identity_restart_and_refusals_verified"] = True
            check_native_pairing(binary / "cryptex-desktop", temp / "pairing", run)
            report["native_pairing_and_reopen_verified"] = True
            check_parent_loss(binary / "cryptexd", temp / "loss", run)
            report["daemon_loss_and_unresolved_refusal_verified"] = True
            report["credential_cleanup_verified"] = True
            shutil.rmtree(temp)
            report["runtime_cleanup_verified"] = True
            report["outcome"] = "passed"
        except PreflightError as error:
            report["outcome"] = "failed"
            report["diagnostic"] = str(error)
        except (OSError, ValueError, TypeError, KeyError, subprocess.SubprocessError):
            report["outcome"] = "collector_incomplete"
        finally:
            if report.get("runtime_cleanup_verified") is not True:
                print(f"Test kaydı inceleme için korundu: {temp}", flush=True)
    output = package / ("MAC-AKTARIM-SONUC-" + uuid.uuid4().hex[:8] + ".zip")
    with zipfile.ZipFile(output, "x") as archive:
        archive.writestr("result.json", json.dumps(report, indent=2) + "\n")
    print(f"Sonuç: {report['outcome']}\nPaylaşılacak dosya: {output}", flush=True)
    return 0 if report["outcome"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
