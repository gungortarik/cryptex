"""Use Antigravity ACP for discovery only, with a fresh project and no model prompt."""
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import sys
import tempfile
import threading
import time

TOOLS = ["cryptex_health", "cryptex_request_public_read", "cryptex_search_public_notes"]


def probe(exe, entry, *, interactive=False):
    with tempfile.TemporaryDirectory(prefix="cryptex-agy-") as temp:
        root = Path(temp)
        workspace, home = root / "workspace", root / "home"
        workspace.mkdir()
        config = home / ".gemini" / "config"
        config.mkdir(parents=True)
        receipt = root / "catalog.json"
        args = [str(Path(__file__).with_name("discovery-witness.mjs").resolve()),
                str(entry.resolve()), str(receipt)]
        node = shutil.which("node")
        if not node:
            raise RuntimeError("Node.js is required")
        server = {"command": node, "args": args}
        (config / "mcp_config.json").write_text(json.dumps({"mcpServers": {"cryptex": server}}), encoding="utf-8")
        env = {k: v for k, v in os.environ.items() if k.upper() in {
            "PATH", "SYSTEMROOT", "WINDIR", "COMSPEC", "PATHEXT", "TEMP", "TMP", "TMPDIR", "TERM", "LANG",
        }}
        env.update(HOME=str(home), USERPROFILE=str(home), APPDATA=str(home / "appdata"),
                   LOCALAPPDATA=str(home / "local"), XDG_CONFIG_HOME=str(home / ".config"))
        def connect():
            child = subprocess.Popen([exe, "acp"], cwd=workspace, env=env, stdin=subprocess.PIPE,
                                     stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                     encoding="utf-8", errors="strict")
            events = queue.Queue()
            def read_output():
                try:
                    for line in child.stdout:
                        events.put(json.loads(line))
                finally:
                    events.put(None)
            threading.Thread(target=read_output, daemon=True).start()
            def rpc(number, method, params):
                child.stdin.write(json.dumps({"jsonrpc": "2.0", "id": number, "method": method, "params": params}) + "\n")
                child.stdin.flush()
                until = time.monotonic() + 45
                while time.monotonic() < until:
                    event = events.get(timeout=max(0.01, until-time.monotonic()))
                    if event is None:
                        raise RuntimeError("Antigravity closed the session")
                    if event.get("id") == number and "method" not in event:
                        return event
                raise TimeoutError(method)
            try:
                init = rpc(1, "initialize", {"protocolVersion": 1, "clientCapabilities": {}})["result"]
                if "Antigravity" not in init["agentInfo"]["name"]:
                    raise RuntimeError("Unexpected ACP client")
                session = rpc(2, "session/new", {"cwd": str(workspace), "mcpServers": [
                    {"name": "cryptex", **server, "env": []},
                ]})
                error = session.get("error", {})
                if error.get("code") == -32000 and "authentication required" in error.get("message", "").lower():
                    return "authentication_required"
                if not session.get("result", {}).get("sessionId"):
                    raise RuntimeError("Antigravity did not establish the disposable session")
                until = time.monotonic() + 25
                while not receipt.exists() and time.monotonic() < until:
                    time.sleep(0.2)
                if not receipt.exists():
                    raise RuntimeError("The bridge did not observe Antigravity listing its tools")
                return "discovered"
            finally:
                child.stdin.close()
                try:
                    child.wait(timeout=4)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
        result = connect()
        if interactive and result == "authentication_required":
            print("Antigravity gecici profilde acilacak. Kendi hesabinizla giris yapin. /mcp ekraninda Cryptex araclarini goruntuleyin ve uygulamadan cikin. Modele mesaj yazmaniz gerekmiyor.", flush=True)
            subprocess.run([exe], cwd=workspace, env=env, check=True)
            result = "discovered" if receipt.exists() else connect()
        if result == "discovered" and json.loads(receipt.read_text(encoding="utf-8")) != {"bridge": "cryptex-local", "tools": TOOLS}:
            raise RuntimeError("Unexpected bridge catalog")
        return result


if __name__ == "__main__":
    state = probe(sys.argv[1], Path(sys.argv[2]), interactive="--interactive" in sys.argv[3:])
    print("antigravity_acp=" + state)
    if state != "discovered":
        sys.exit(1)
