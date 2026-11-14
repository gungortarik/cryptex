"""Check both workspace code and the separate historical integration driver."""
import subprocess
import sys

failed = False
for command in [
    ["cargo", "fmt", "--all", "--", "--check"],
    ["rustfmt", "--edition", "2024", "--check", "tools/m8-protocol-vectors/old_reader_compat.rs"],
]:
    failed |= subprocess.run(command, check=False).returncode != 0
sys.exit(1 if failed else 0)
