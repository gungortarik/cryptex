"""Compile-only public SDK surface audit; never an authentication proof."""
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import sys
import tempfile

sys.dont_write_bytecode = True
from mac_identity_preflight import PreflightError, checked, run_bounded, scalar

HEADERS = """#include <Security/Security.h>
#include <xpc/xpc.h>
#include <bsm/libbsm.h>
#include <mach/mach.h>
#include <sys/un.h>
#include <sys/event.h>
#include <libproc.h>
"""
EXPRESSIONS = {
    "message_code_validation": "(void)&SecCodeCreateWithXPCMessage; (void)&SecCodeCheckValidity;",
    "joint_pid_audit_guest_lookup": "(void)&SecCodeCopyGuestWithAttributes; (void)kSecGuestAttributePid; (void)kSecGuestAttributeAudit;",
    "task_name_right": "(void)&task_name_for_pid; (void)&pid_for_task; (void)&mach_port_type;",
    "task_self_audit": "(void)TASK_AUDIT_TOKEN; (void)TASK_AUDIT_TOKEN_COUNT; (void)&task_info;",
    "kqueue_process_exit": "(void)EVFILT_PROC; (void)NOTE_EXIT; (void)&kevent;",
    "xpc_message_audit_getter": "(void)&xpc_dictionary_get_audit_token;",
    "xpc_connection_audit_getter": "(void)&xpc_connection_get_audit_token;",
    "audit_pid_getter": "(void)&audit_token_to_pid;",
    "audit_pidversion_getter": "(void)&audit_token_to_pidversion;",
    "unix_peer_audit": "(void)SOL_LOCAL; (void)LOCAL_PEERTOKEN;",
    "libproc_unique_instance": "(void)PROC_PIDUNIQIDENTIFIERINFO; struct proc_uniqidentifierinfo info; (void)sizeof(info);",
    "libproc_start_time": "struct proc_bsdinfo info; (void)info.pbi_start_tvsec; (void)info.pbi_start_tvusec; (void)&proc_pidinfo;",
}


def compile_observation(result):
    if result.limited or result.timed_out or result.pipe_failed:
        raise PreflightError("sdk_audit_process_bound_failed")
    return "compile_pass" if result.code == 0 else "compile_failed"


def audit(work, run=run_bounded):
    sdk = checked(run, ["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-path"], "sdk_path").strip()
    if not sdk.startswith("/") or len(sdk) > 4096 or any(c in sdk for c in "\r\n\0"):
        raise PreflightError("sdk_path_invalid")
    report = {"schema": 1, "scope": "public_sdk_peer_identity_api_surface_only",
              "collected_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
              "exact_instance_binding": "not_proven", "native_authorization": "not_tested",
              "product_minimum_os": "unconfirmed", "production_changed": False, "m8_complete": False}
    report["sdk_version"] = scalar(run, ["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-version"], r"\d{1,3}(?:\.\d{1,3}){1,2}", "sdk_version")
    report["macos_version"] = scalar(run, ["/usr/bin/sw_vers", "-productVersion"], r"\d{1,3}(?:\.\d{1,3}){1,2}", "os_version")
    report["architecture"] = scalar(run, ["/usr/bin/uname", "-m"], r"arm64|x86_64", "architecture")
    report["observations"] = {}
    for name, expression in {"baseline_public_headers": "", **EXPRESSIONS}.items():
        source = work / (name + ".c")
        with source.open("x", encoding="utf-8", newline="\n") as output:
            output.write(HEADERS + "void probe(void) { " + expression + " }\n")
        result = run(["/usr/bin/xcrun", "--sdk", "macosx", "clang", "-std=c11", "-fblocks",
                      "-Wall", "-Wextra", "-Werror", "-fsyntax-only", "-isysroot", sdk, str(source)], 30)
        status = compile_observation(result)
        if name == "baseline_public_headers" and status != "compile_pass":
            raise PreflightError("sdk_public_header_baseline_failed")
        report["observations"][name] = status
        # Fixed source snippets and public SDK diagnostics only, on disposable CI.
        # Never enumerate accounts/credentials, include private headers or run binaries.
        if status == "compile_failed":
            print("SDK_AUDIT_DIAGNOSTIC " + name, file=sys.stderr)
            print(result.output, file=sys.stderr)
    return report


def main():
    if sys.platform != "darwin" or os.environ.get("GITHUB_ACTIONS") != "true":
        return 2
    work = Path(tempfile.mkdtemp(prefix="cryptex-peer-api.", dir="/private/tmp"))
    if any((p / ".git").exists() or (p / ".git").is_symlink() for p in (work, *work.parents)):
        raise PreflightError("sdk_audit_runtime_inside_repository")
    os.environ["TMPDIR"] = str(work)
    print(json.dumps(audit(work), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
