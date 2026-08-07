#!/usr/bin/env python3
from __future__ import annotations

import json
import shutil
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent
FIXTURES = ROOT / "fixtures"

BASE = {
    "agency.txt": """agency_id,agency_name,agency_url,agency_timezone,agency_lang\nA,Test Transit,https://example.com,Europe/Istanbul,en\n""",
    "stops.txt": """stop_id,stop_name,stop_lat,stop_lon\nS1,Start,41.000000,29.000000\nS2,End,41.010000,29.010000\n""",
    "routes.txt": """route_id,agency_id,route_short_name,route_long_name,route_type\nR1,A,1,Test Route,3\n""",
    "trips.txt": """route_id,service_id,trip_id,shape_id\nR1,WK,T1,SH1\n""",
    "stop_times.txt": """trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\nT1,08:00:00,08:00:00,S1,1,0\nT1,08:10:00,08:10:00,S2,2,10\n""",
    "calendar.txt": """service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nWK,1,1,1,1,1,1,1,20260101,20261231\n""",
    "shapes.txt": """shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,41.000000,29.000000,1,0\nSH1,41.010000,29.010000,2,10\n""",
    "feed_info.txt": """feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date,feed_version\nTest Publisher,https://example.com,en,20260101,20261231,1\n""",
    "fare_attributes.txt": """fare_id,price,currency_type,payment_method,transfers,agency_id\nF1,2.50,USD,0,0,A\n""",
}


def replace(files: dict[str, str], filename: str, old: str, new: str) -> None:
    assert old in files[filename], (filename, old)
    files[filename] = files[filename].replace(old, new, 1)


def m_invalid_calendar_date(files: dict[str, str]) -> None:
    replace(files, "calendar.txt", "20260101,20261231", "20260231,20261231")


def m_malformed_time(files: dict[str, str]) -> None:
    replace(files, "stop_times.txt", "T1,08:00:00,08:00:00", "T1,1:2:3,1:2:3")


def m_nan_coordinate(files: dict[str, str]) -> None:
    replace(files, "stops.txt", "S1,Start,41.000000,29.000000", "S1,Start,NaN,29.000000")


def m_field_case(files: dict[str, str]) -> None:
    replace(files, "stops.txt", "stop_id,stop_name", "STOP_ID,stop_name")


def m_url_escaping(files: dict[str, str]) -> None:
    replace(files, "agency.txt", "https://example.com,Europe/Istanbul", "https://example.com/a b,Europe/Istanbul")


def m_unescaped_quote(files: dict[str, str]) -> None:
    replace(files, "stops.txt", "S1,Start,41.000000", 'S1,12" Street,41.000000')


def m_shape_distance_regression(files: dict[str, str]) -> None:
    replace(files, "stop_times.txt", "S1,1,0\nT1,08:10:00,08:10:00,S2,2,10", "S1,1,10\nT1,08:10:00,08:10:00,S2,2,5")


def m_invalid_currency(files: dict[str, str]) -> None:
    replace(files, "fare_attributes.txt", "2.50,USD,0,0,A", "2.50,ZZZ,0,0,A")


def m_broken_route_reference(files: dict[str, str]) -> None:
    replace(files, "trips.txt", "R1,WK,T1,SH1", "MISSING,WK,T1,SH1")


def m_invalid_language_tag(files: dict[str, str]) -> None:
    replace(files, "feed_info.txt", "https://example.com,en,20260101", "https://example.com,123,20260101")


def m_html_tag(files: dict[str, str]) -> None:
    replace(files, "stops.txt", "S1,Start,41.000000", "S1,<script>Start</script>,41.000000")


def m_stop_sequence_regression(files: dict[str, str]) -> None:
    replace(files, "stop_times.txt", "S2,2,10", "S2,0,10")


CASES = [
    ("invalid_calendar_date", "calendar.txt start_date is 20260231", m_invalid_calendar_date),
    ("malformed_time", "stop_times time uses 1:2:3 instead of [H]H:MM:SS", m_malformed_time),
    ("nan_coordinate", "stops.stop_lat is NaN", m_nan_coordinate),
    ("field_case", "required stops.stop_id header is upper-case STOP_ID", m_field_case),
    ("url_escaping", "agency_url contains an unescaped space", m_url_escaping),
    ("unescaped_quote", "unquoted stop_name contains a quotation mark", m_unescaped_quote),
    ("shape_distance_regression", "stop sequence increases while shape_dist_traveled decreases", m_shape_distance_regression),
    ("invalid_currency", "fare currency_type is unknown ISO 4217 code ZZZ", m_invalid_currency),
    ("broken_route_reference", "trips.route_id references a missing route", m_broken_route_reference),
    ("invalid_language_tag", "feed_info.feed_lang is invalid BCP 47 tag 123", m_invalid_language_tag),
    ("html_tag", "stop_name contains an HTML script element", m_html_tag),
    ("stop_sequence_regression", "stop_times.stop_sequence decreases", m_stop_sequence_regression),
]


def write_zip(name: str, files: dict[str, str]) -> Path:
    path = FIXTURES / f"{name}.zip"
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for filename, content in files.items():
            zf.writestr(filename, content)
    return path


def main() -> None:
    if FIXTURES.exists():
        shutil.rmtree(FIXTURES)
    FIXTURES.mkdir(parents=True)

    write_zip("baseline", dict(BASE))
    manifest = [{"id": "baseline", "description": "unmodified control feed"}]

    for case_id, description, mutation in CASES:
        files = dict(BASE)
        mutation(files)
        write_zip(case_id, files)
        manifest.append({"id": case_id, "description": description})

    (FIXTURES / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"generated {len(manifest)} fixtures in {FIXTURES}")


if __name__ == "__main__":
    main()
