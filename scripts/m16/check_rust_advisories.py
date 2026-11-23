"""Match public registry versions to OSV and report their native build scope."""
import json
from pathlib import Path
import re
import subprocess
import tomllib
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
CODEC = ROOT / "tools/m11-backup-spike/native-codec/Cargo.toml"
LOCKS = [ROOT / "Cargo.lock", CODEC.with_name("Cargo.lock"),
         ROOT / "tools/m1-secure-store-spike/Cargo.lock"]
TARGETS = {"windows": "x86_64-pc-windows-msvc", "macos": "aarch64-apple-darwin"}


def build_scopes():
    scopes = {}
    for os_name, target in TARGETS.items():
        for component, options in [
            ("product", ["-p", "cryptex-desktop", "-p", "cryptexd", "-p", "cryptex-updater"]),
            ("codec-trial", ["--manifest-path", str(CODEC)]),
        ]:
            output = subprocess.check_output(
                ["cargo", "tree", "--locked", "--all-features", "--prefix", "none",
                 "--format", "{p}", "--edges", "normal,build", "--target", target, *options],
                cwd=ROOT, text=True)
            for package in re.findall(r"^([\w-]+) v([^\s]+)", output, re.M):
                scopes.setdefault(package, set()).add(f"{component}-{os_name}")
    return scopes


def main():
    packages = sorted({
        (p["name"], p["version"]) for lock in LOCKS
        for p in tomllib.loads(lock.read_text(encoding="utf-8"))["package"]
        if p.get("source") == "registry+https://github.com/rust-lang/crates.io-index"
    })
    scopes = build_scopes()
    product_hits = []
    for offset in range(0, len(packages), 70):
        batch = packages[offset:offset + 70]
        queries = [{"package": {"ecosystem": "crates.io", "name": name}, "version": version}
                   for name, version in batch]
        request = urllib.request.Request("https://api.osv.dev/v1/querybatch",
                                         json.dumps({"queries": queries}).encode(),
                                         {"Content-Type": "application/json"})
        with urllib.request.urlopen(request, timeout=30) as response:
            results = json.load(response)["results"]
        for package, result in zip(batch, results, strict=True):
            advisories = sorted(v["id"] for v in result.get("vulns", []))
            if advisories:
                scope = scopes.get(package, {"outside-native-product-and-codec-graphs"})
                print(*package, ",".join(advisories), ",".join(sorted(scope)))
                if any(item.startswith("product-") for item in scope):
                    product_hits.append(package)
    print(f"public_registry_versions={len(packages)} affected_native_product_versions={len(product_hits)}")
    return int(bool(product_hits))


if __name__ == "__main__":
    raise SystemExit(main())
