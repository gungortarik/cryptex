"""Build the read-only macOS preflight package with executable Unix ZIP modes."""

import argparse
from pathlib import Path
import zipfile


def build(output: Path) -> None:
    source = Path(__file__).resolve().parent
    modes = {"MAC-KONTROL.command": 0o100755, "MAC-BASLA.txt": 0o100644}
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, mode in modes.items():
            item = zipfile.ZipInfo(name)
            item.create_system = 3
            item.external_attr = mode << 16
            archive.writestr(item, (source / name).read_bytes().replace(b"\r\n", b"\n"))
    with zipfile.ZipFile(output) as archive:
        assert archive.namelist() == list(modes)
        for name, mode in modes.items():
            assert archive.getinfo(name).external_attr >> 16 == mode
            assert b"\r" not in archive.read(name)
    print(output.resolve())


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        default=Path.home() / "Downloads" / "cryptex-mac-kontrol-20261108.zip",
    )
    build(parser.parse_args().output)
