"""Bounded standalone SDK evidence for the actual default-off TCP adapter."""
from pathlib import Path

from mac_identity_preflight import PreflightError


CASES = ("ipv4_close", "ipv6_close", "listener_only", "fd_reuse", "pre_send_exec",
         "queued_exec", "root_loss", "shutdown_exit")


def check_native_endpoints(root, run):
    root.mkdir(mode=0o700)
    binary = root / "endpoint-probe"
    # Include the actual adapter through the standalone test translation unit.
    # Its deterministic libproc interception never enters Cargo or the daemon.
    source = Path.cwd() / "services/cryptexd/tests/mac_transport_endpoint.c"
    run(["/usr/bin/xcrun", "clang", "-std=c11", "-Wall", "-Wextra", "-Werror",
         source, "-framework", "Security", "-framework", "CoreFoundation", "-lbsm",
         "-o", binary], "compile_native_endpoint_probe", 30)
    run(["/usr/bin/codesign", "--force", "--sign", "-", "--timestamp=none",
         "--identifier", "com.cryptex.m8.endpoint-probe", binary],
        "sign_native_endpoint_probe", 20)
    for case in CASES:
        marker = "mac_transport_endpoint_" + case + "=passed"
        output = run([binary, case], "native_endpoint_" + case, 15)
        if output.splitlines() != [marker]:
            raise PreflightError("native_endpoint_marker_mismatch")
        print(marker)
    print("mac_transport_endpoint_races=passed")
