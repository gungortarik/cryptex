#!/usr/bin/env python3
"""Synthetic local hard-link/copy projection spike; no Cryptex data is read."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import tempfile
import time
from pathlib import Path


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def run_case(source: Path, out: Path) -> dict[str, object]:
    out.mkdir()
    result: dict[str, object] = {"source_bytes": source.stat().st_size}
    source_hash = digest(source)

    started = time.perf_counter_ns()
    try:
        os.link(source, out / "hard link.bin")
        result["hard_link"] = "pass"
        result["hard_link_ms"] = round((time.perf_counter_ns() - started) / 1e6, 3)
        result["same_file"] = os.path.samefile(source, out / "hard link.bin")
        (out / "hard link.bin").unlink()
        result["source_after_link_cleanup_sha256"] = digest(source)
    except OSError as exc:
        result["hard_link"] = "unavailable"
        result["hard_link_error"] = {"type": type(exc).__name__, "errno": exc.errno}

    started = time.perf_counter_ns()
    shutil.copyfile(source, out / "copy.tmp")
    result["copy_ms"] = round((time.perf_counter_ns() - started) / 1e6, 3)
    os.replace(out / "copy.tmp", out / "copied file.bin")
    result["copy_bytes"] = (out / "copied file.bin").stat().st_size
    result["copy_matches"] = digest(out / "copied file.bin") == source_hash
    (out / "copied file.bin").unlink()
    result["source_after_copy_cleanup_sha256"] = digest(source)
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", help="new dedicated output directory; must not exist")
    parser.add_argument("--other-volume", help="optional existing directory on another volume")
    args = parser.parse_args()
    root = Path(args.root).expanduser().resolve() if args.root else Path(
        tempfile.mkdtemp(prefix="cryptex-m6-attachment-p1-")
    ).resolve()
    if args.root:
        root.mkdir(parents=True, exist_ok=False)
    if not root.is_dir() or any(root.iterdir()):
        raise SystemExit("Output root must be a new or empty dedicated directory")

    fixture_dir = root / "synthetic source – café"
    fixture_dir.mkdir()
    cases: list[dict[str, object]] = []
    sizes = (0, 4096, 1024 * 1024, 64 * 1024 * 1024)
    for size in sizes:
        source = fixture_dir / f"fixture-{size}.bin"
        with source.open("wb") as stream:
            stream.truncate(size)
        cases.append({"size": size, **run_case(source, root / f"same-volume-{size}")})

    # Hard links share the inode/file record. Prove mutation on disposable bytes only.
    mutation_source = fixture_dir / "mutation-proof.bin"
    mutation_source.write_bytes(b"source-before")
    mutation_link = root / "mutation-link.bin"
    mutation: dict[str, object] = {}
    try:
        os.link(mutation_source, mutation_link)
        mutation["hard_link"] = "pass"
        mutation_link.write_bytes(b"changed-through-projection")
        mutation["source_changed_through_link"] = mutation_source.read_bytes() == b"changed-through-projection"
    except OSError as exc:
        mutation["hard_link"] = "unavailable"
        mutation["error"] = {"type": type(exc).__name__, "errno": exc.errno}
    finally:
        if mutation_link.exists():
            mutation_link.unlink()
    mutation_source.unlink()

    # A failed staging copy must not publish a final path or alter the source.
    failure_source = fixture_dir / "failure probe – résumé.bin"
    failure_source.write_bytes(b"synthetic-source")
    failure_out = root / "failure-probe"
    failure_out.mkdir()
    failed_copy: dict[str, object] = {}
    try:
        shutil.copyfile(failure_source, failure_out)  # destination is a directory
        failed_copy["unexpected_success"] = True
    except OSError as exc:
        failed_copy["error"] = {"type": type(exc).__name__, "errno": exc.errno}
        failed_copy["source_unchanged"] = failure_source.read_bytes() == b"synthetic-source"
        failed_copy["final_absent"] = not (failure_out / failure_source.name).exists()

    # Simulate interruption after partial staging: only the temporary output is removed.
    interrupted = root / "interrupted-copy"
    interrupted.mkdir()
    partial = interrupted / "attachment.tmp"
    partial.write_bytes(b"partial")
    final = interrupted / "attachment.bin"
    interrupted_copy = {"final_absent_before_cleanup": not final.exists()}
    partial.unlink()
    interrupted_copy["final_absent_after_cleanup"] = not final.exists()
    interrupted_copy["source_unchanged"] = failure_source.read_bytes() == b"synthetic-source"
    failure_source.unlink()

    # Filesystem name behavior only; no production sanitizer is inferred here.
    names = root / "synthetic source – café"
    (names / "résumé note.md").write_text("synthetic", encoding="utf-8")
    (names / "CaseName.md").write_text("upper", encoding="utf-8")
    case_name_probe: dict[str, object] = {"unicode_space_name_created": (names / "résumé note.md").exists()}
    try:
        with (names / "casename.md").open("x", encoding="utf-8") as stream:
            stream.write("lower")
        case_name_probe["case_variant_created"] = True
    except FileExistsError:
        case_name_probe["case_variant_created"] = False

    cross_volume: dict[str, object] = {"status": "not_run"}
    if args.other_volume:
        other = Path(args.other_volume).expanduser().resolve()
        if not other.is_dir():
            raise SystemExit("--other-volume must name an existing directory")
        other = Path(tempfile.mkdtemp(prefix="cryptex-m6-attachment-p1-", dir=other))
        source = fixture_dir / "fixture-4096.bin"
        cross_volume = {"target": str(other), "same_volume": os.stat(root).st_dev == os.stat(other).st_dev}
        try:
            os.link(source, other / "cryptex-m6-spike-link.bin")
            (other / "cryptex-m6-spike-link.bin").unlink()
            cross_volume["hard_link"] = "pass"
        except OSError as exc:
            cross_volume["hard_link"] = "unavailable"
            cross_volume["error"] = {"type": type(exc).__name__, "errno": exc.errno}
        copied = other / "cryptex-m6-spike-copy.tmp"
        try:
            shutil.copyfile(source, copied)
            cross_volume["copy_fallback"] = (
                copied.stat().st_size == source.stat().st_size and digest(copied) == digest(source)
            )
        except OSError as exc:
            cross_volume["copy_fallback"] = "failed"
            cross_volume["copy_error"] = {"type": type(exc).__name__, "errno": exc.errno}
        finally:
            if copied.exists():
                copied.unlink()

    report = {
        "host": platform.platform(),
        "python": platform.python_version(),
        "root": str(root),
        "filesystem_device_id": os.stat(root).st_dev,
        "same_volume_cases": cases,
        "hard_link_mutation": mutation,
        "copy_failure_probe": failed_copy,
        "interrupted_staging_probe": interrupted_copy,
        "filename_probe": case_name_probe,
        "cross_volume": cross_volume,
        "other_volume_probe_root": str(other) if args.other_volume else None,
        "cleanup_scope": "generated paths are beneath unique probe roots; source fixtures retained",
    }
    (root / "benchmark.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    return 0 if all(case.get("copy_matches") for case in cases) else 1


if __name__ == "__main__":
    raise SystemExit(main())
