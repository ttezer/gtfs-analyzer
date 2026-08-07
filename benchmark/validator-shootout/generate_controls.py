#!/usr/bin/env python3
from __future__ import annotations

import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent
FIXTURES = ROOT / "fixtures"


def read_zip(path: Path) -> dict[str, str]:
    with zipfile.ZipFile(path) as zf:
        return {name: zf.read(name).decode("utf-8") for name in zf.namelist()}


def write_zip(path: Path, files: dict[str, str]) -> None:
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for name, content in files.items():
            zf.writestr(name, content)


def append_column(content: str, header: str, values: list[str]) -> str:
    lines = content.rstrip("\n").split("\n")
    assert len(lines) - 1 == len(values)
    lines[0] += f",{header}"
    for i, value in enumerate(values, 1):
        lines[i] += f",{value}"
    return "\n".join(lines) + "\n"


def main() -> None:
    base = read_zip(FIXTURES / "baseline.zip")

    # Control for invalid_location_type: same optional column is present, but all values valid.
    files = dict(base)
    files["stops.txt"] = append_column(files["stops.txt"], "location_type", ["0", "0"])
    write_zip(FIXTURES / "control_valid_location_type.zip", files)

    # Control for frequency_headway_zero: same frequencies.txt structure, valid positive headway.
    files = dict(base)
    files["frequencies.txt"] = (
        "trip_id,start_time,end_time,headway_secs,exact_times\n"
        "T1,08:00:00,09:00:00,600,0\n"
    )
    write_zip(FIXTURES / "control_valid_frequency.zip", files)

    print("generated paired crash-attribution controls")


if __name__ == "__main__":
    main()
