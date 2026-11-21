"""Smoke-test installed clients and emitted configuration inside fresh user profiles."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


def verify(client, tools, config, claude):
    with tempfile.TemporaryDirectory(prefix="cryptex-mcp-") as temp:
        root = Path(temp)
        workspace, home = root / "workspace", root / "home"
        workspace.mkdir()
        home.mkdir()
        env = {k: v for k, v in os.environ.items() if k.upper() in
               {"PATH", "WINDIR", "SYSTEMROOT", "PATHEXT", "COMSPEC", "TMP", "TMPDIR", "TEMP", "LANG"}}
        env.update(HOME=str(home), USERPROFILE=str(home), APPDATA=str(home / "appdata"),
                   LOCALAPPDATA=str(home / "local"), XDG_CONFIG_HOME=str(home / ".config"),
                   GROK_HOME=str(home / ".grok"), CLAUDE_CONFIG_DIR=str(home / ".claude"),
                   CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC="1", CI="1", NO_COLOR="1")
        def run(args):
            result = subprocess.run(args, cwd=workspace, env=env, capture_output=True,
                                    encoding="utf-8", errors="replace", timeout=50, check=False)
            if result.returncode:
                raise RuntimeError(f"{client} exited with {result.returncode}: {result.stderr[:350]}")
            return result.stdout

        snippet = run([shutil.which("node"), config, client])
        if client == "claude":
            server = json.loads(snippet)["mcpServers"]["cryptex"]
            run([claude, "mcp", "add-json", "--scope", "user", "cryptex", json.dumps(server)])
            assert re.search(r"Status:.*Connected", run([claude, "mcp", "get", "cryptex"]))
            return "Connected"
        windows = os.name == "nt"
        if client == "grok":
            folder = home / ".grok"
            folder.mkdir()
            (folder / "config.toml").write_text(snippet, encoding="utf-8")
            report = json.loads(run([tools / ("grok.exe" if windows else "grok"),
                                     "mcp", "doctor", "cryptex", "--json"]))
            server, = report["servers"]
            assert server["name"] == "cryptex" and server["transport"] == "stdio" and server["healthy"]
            checks = [item["label"] for item in server["checks"] if item["passed"]]
            assert "handshake OK" in checks and "3 tools discovered" in checks
            return "handshake and three tools"
        if client == "antigravity":
            folder = home / ".gemini" / "config"
            folder.mkdir(parents=True)
            (folder / "mcp_config.json").write_text(snippet, encoding="utf-8")
            output = run([tools / ("agy.exe" if windows else "agy"), "mcp", "list"])
            assert re.search(r"^cryptex\s+stdio\s+enabled\s+", output, re.MULTILINE)
            return "configured and enabled"
        folder = workspace / ".cursor"
        folder.mkdir()
        (folder / "mcp.json").write_text(snippet, encoding="utf-8")
        package = tools / "dist-package"
        agent = [package / "node.exe", package / "index.js"] if windows else [package / "cursor-agent"]
        run([*agent, "mcp", "enable", "cryptex"])
        catalog = run([*agent, "mcp", "list-tools", "cryptex"])
        for tool in ["cryptex_health", "cryptex_request_public_read", "cryptex_search_public_notes"]:
            assert tool in catalog, f"Cursor did not list {tool}"
        return "three named tools discovered"


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("tools", type=Path)
    parser.add_argument("config", type=Path)
    parser.add_argument("claude", type=Path)
    args = parser.parse_args()
    for name in ["claude", "grok", "antigravity", "cursor"]:
        print(f"{name}: {verify(name, args.tools.resolve(), args.config.resolve(), args.claude.resolve())}", flush=True)
