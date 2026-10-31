#!/usr/bin/env python3
"""Cross-process Unix-socket P0-3 spike check for macOS."""

from __future__ import annotations

import json
import os
import select
import socket
import stat
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path


FRAME_LIMIT = 64 * 1024


def fail(message: str) -> RuntimeError:
    return RuntimeError(message)


def main() -> int:
    if sys.platform != "darwin":
        raise fail("This script is for a physical macOS run.")

    repo = Path(__file__).resolve().parents[3]
    binary = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else repo / "target/debug/m0-ipc-spike"
    if not binary.is_file():
        raise fail(f"Spike binary not found: {binary}; build it first.")

    runtime_root = Path.home() / "Library/Application Support/Cryptex-Dev/p0-3"
    run_root = runtime_root / f"ipc-macos-{datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')}"
    run_root.mkdir(parents=True, exist_ok=False)
    run_id = f"mac-{time.time_ns() % 10**16:x}"
    socket_dir = Path("/tmp") / f"cpx-{run_id}"
    socket_path = socket_dir / "s"
    checks: list[dict[str, object]] = []
    servers: list[subprocess.Popen[str]] = []
    clients: list[subprocess.Popen[str]] = []
    server_logs: list[str] = []

    def start_server() -> subprocess.Popen[str]:
        server = subprocess.Popen(
            [str(binary), "serve", "--id", run_id],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        servers.append(server)
        assert server.stdout is not None
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            ready, _, _ = select.select([server.stdout], [], [], 0.1)
            if ready:
                line = server.stdout.readline()
                server_logs.append(line)
                if line.startswith("ready "):
                    return server
                if server.poll() is not None:
                    break
            if server.poll() is not None:
                break
        stderr = server.stderr.read() if server.stderr else ""
        raise fail(f"IPC server did not become ready (exit={server.poll()}): {stderr}")

    def stop_server(server: subprocess.Popen[str]) -> None:
        if server.poll() is None:
            server.terminate()
            server.wait(timeout=5)

    def client(role: str, capability: str = "status.read", version: int = 1, accepted: bool = True) -> dict[str, object]:
        result = subprocess.run(
            [
                str(binary), "client", "--id", run_id, "--role", role,
                "--capability", capability, "--version", str(version),
            ],
            check=False,
            capture_output=True,
            text=True,
            timeout=12,
        )
        lines = [line for line in result.stdout.splitlines() if line.strip()]
        if not lines:
            raise fail(f"{role} client returned no protocol response: {result.stderr}")
        response = json.loads(lines[-1])
        if response.get("accepted") is not accepted:
            raise fail(f"Unexpected {role} response: {response}")
        if accepted and result.returncode != 0:
            raise fail(f"Authorized {role} client exited {result.returncode}.")
        if not accepted and result.returncode == 0:
            raise fail(f"Unauthorized {role} client unexpectedly succeeded.")
        checks.append({"role": role, "capability": capability, "version": version, **response})
        return response

    def raw_request(payload: bytes, expected_error: str) -> None:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
            connection.settimeout(5)
            connection.connect(str(socket_path))
            connection.sendall(payload)
            response = bytearray()
            while len(response) <= 4096:
                chunk = connection.recv(1)
                if not chunk or chunk == b"\n":
                    response.extend(chunk)
                    break
                response.extend(chunk)
        parsed = json.loads(response)
        if parsed.get("error") != expected_error:
            raise fail(f"Expected {expected_error}; received {parsed}.")
        checks.append({"role": "raw", "accepted": False, "error": parsed["error"]})

    outcome = "FAIL_MACOS_SPIKE_ONLY_NOT_ADR_ACCEPTANCE"
    try:
        server = start_server()

        parent_info = socket_dir.lstat()
        if not stat.S_ISDIR(parent_info.st_mode) or parent_info.st_uid != os.getuid() or parent_info.st_mode & 0o077:
            raise fail("Unix socket parent is not a current-user-only directory.")
        if len(os.fsencode(socket_path)) >= 104:
            raise fail("Unix socket path exceeds the macOS sockaddr_un limit.")
        checks.append({
            "role": "transport",
            "accepted": True,
            "socket_path_bytes": len(os.fsencode(socket_path)),
            "parent_mode": oct(stat.S_IMODE(parent_info.st_mode)),
            "parent_uid": parent_info.st_uid,
        })

        for role in ("desktop", "cli", "mcp"):
            client(role)
        client("cli", version=2, accepted=False)
        client("mcp", capability="memory.write", accepted=False)
        client("unknown", accepted=False)
        raw_request(b"{\n", "malformed_request")
        raw_request(b"A" * (FRAME_LIMIT + 1), "frame_too_large")

        stop_server(server)
        retry = subprocess.Popen(
            [str(binary), "client", "--id", run_id, "--role", "desktop"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        clients.append(retry)
        time.sleep(0.5)
        restarted = start_server()
        stdout, stderr = retry.communicate(timeout=12)
        if retry.returncode != 0:
            raise fail(f"Reconnect client exited {retry.returncode}: {stderr}")
        response = json.loads(stdout.splitlines()[-1])
        if not response.get("accepted") or response.get("daemon_pid") != restarted.pid:
            raise fail(f"Client did not reconnect to restarted process {restarted.pid}: {response}")
        checks.append({"role": "desktop", "accepted": True, "reconnected_to_pid": response["daemon_pid"]})

        result = {
            "experiment": "m0-p0-3-macos-local-ipc",
            "runtime_root": str(run_root),
            "endpoint": str(socket_path),
            "checks": checks,
            "result": "PASS_MACOS_SPIKE_ONLY_NOT_ADR_ACCEPTANCE",
        }
        (run_root / "summary.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        (run_root / "server-startup.log").write_text("".join(server_logs), encoding="utf-8")
        print(json.dumps(result, indent=2))
        outcome = result["result"]
        return 0
    except Exception as error:
        failure = {
            "experiment": "m0-p0-3-macos-local-ipc",
            "runtime_root": str(run_root),
            "endpoint": str(socket_path),
            "checks": checks,
            "result": outcome,
            "error": str(error),
        }
        (run_root / "summary.json").write_text(json.dumps(failure, indent=2) + "\n", encoding="utf-8")
        (run_root / "server-startup.log").write_text("".join(server_logs), encoding="utf-8")
        raise
    finally:
        for client_process in clients:
            if client_process.poll() is None:
                client_process.terminate()
                client_process.wait(timeout=5)
        for server_process in servers:
            stop_server(server_process)
        try:
            if socket_path.is_socket():
                info = socket_dir.lstat()
                if info.st_uid == os.getuid() and not socket_dir.is_symlink() and socket_dir == Path("/tmp") / f"cpx-{run_id}":
                    socket_path.unlink()
                    socket_dir.rmdir()
        except FileNotFoundError:
            pass
        except OSError as error:
            print(f"warning: unable to remove temporary socket directory: {error}", file=sys.stderr)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:  # noqa: BLE001
        print(f"test_unix_ipc_spike: {error}", file=sys.stderr)
        raise SystemExit(1)
