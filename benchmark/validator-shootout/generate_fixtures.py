#!/usr/bin/env python3
from __future__ import annotations

import json
import shutil
import zipfile
from pathlib import Path
from typing import Callable

ROOT = Path(__file__).resolve().parent
FIXTURES = ROOT / "fixtures"

BASE = {
    "agency.txt": """agency_id,agency_name,agency_url,agency_timezone,agency_lang
A,Test Transit,https://example.com,Europe/Istanbul,en
""",
    "stops.txt": """stop_id,stop_name,stop_lat,stop_lon
S1,Start,41.000000,29.000000
S2,End,41.000050,29.000050
""",
    "routes.txt": """route_id,agency_id,route_short_name,route_long_name,route_type
R1,A,1,Test Route,3
""",
    "trips.txt": """route_id,service_id,trip_id,shape_id
R1,WK,T1,SH1
""",
    "stop_times.txt": """trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled
T1,08:00:00,08:00:00,S1,1,0
T1,08:10:00,08:10:00,S2,2,10
""",
    "calendar.txt": """service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date
WK,1,1,1,1,1,1,1,20260101,20261231
""",
    "shapes.txt": """shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled
SH1,41.000000,29.000000,1,0
SH1,41.000050,29.000050,2,10
""",
    "feed_info.txt": """feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date,feed_version,feed_contact_email,feed_contact_url
Test Publisher,https://example.com,en,20260101,20261231,1,contact@example.com,https://example.com/contact
""",
    "fare_attributes.txt": """fare_id,price,currency_type,payment_method,transfers,agency_id
F1,2.50,USD,0,0,A
""",
}

def replace(files: dict[str, str], filename: str, old: str, new: str) -> None:
    assert old in files[filename], (filename, old)
    files[filename] = files[filename].replace(old, new, 1)

def append_column(files: dict[str, str], filename: str, header: str, values: list[str]) -> None:
    lines = files[filename].rstrip("\n").split("\n")
    assert len(lines) - 1 == len(values), (filename, len(lines) - 1, len(values))
    lines[0] += f",{header}"
    for i, value in enumerate(values, 1):
        lines[i] += f",{value}"
    files[filename] = "\n".join(lines) + "\n"

def mutation(kind: str, args: tuple) -> Callable[[dict[str, str]], None]:
    if kind == "replace":
        filename, old, new = args
        return lambda files: replace(files, filename, old, new)
    if kind == "append_column":
        filename, header, values = args
        return lambda files: append_column(files, filename, header, list(values))
    if kind == "add_file":
        filename, content = args
        return lambda files: files.__setitem__(filename, content)
    raise ValueError(kind)

# Each case is a single intended mutation relative to BASE.
# `group` is reported separately so repeated boundary variants do not inflate semantic breadth.
CASES_RAW = [
    ('invalid_calendar_date', 'calendar_date', 'calendar.txt start_date is 20260231', 'CAL_003', 'replace', ('calendar.txt', '20260101,20261231', '20260231,20261231')),
    ('malformed_time', 'time_format', 'stop_times time uses 1:2:3 instead of [H]H:MM:SS', 'STM_003', 'replace', ('stop_times.txt', 'T1,08:00:00,08:00:00', 'T1,1:2:3,1:2:3')),
    ('nan_coordinate', 'stop_lat', 'stops.stop_lat is NaN', 'STP_004', 'replace', ('stops.txt', 'S1,Start,41.000000,29.000000', 'S1,Start,NaN,29.000000')),
    ('field_case', 'field_case', 'required stops.stop_id header is upper-case STOP_ID', 'ARC_025', 'replace', ('stops.txt', 'stop_id,stop_name', 'STOP_ID,stop_name')),
    ('url_escaping', 'url', 'agency_url contains an unescaped space', 'AGN_003', 'replace', ('agency.txt', 'https://example.com,Europe/Istanbul', 'https://example.com/a b,Europe/Istanbul')),
    ('unescaped_quote', 'csv_quote', 'unquoted stop_name contains a quotation mark', 'ARC_033', 'replace', ('stops.txt', 'S1,Start,41.000000', 'S1,12" Street,41.000000')),
    ('shape_distance_regression', 'shape_distance', 'stop_sequence increases while shape_dist_traveled decreases', 'STM_056', 'replace', ('stop_times.txt', 'S1,1,0\nT1,08:10:00,08:10:00,S2,2,10', 'S1,1,10\nT1,08:10:00,08:10:00,S2,2,5')),
    ('invalid_currency', 'currency', 'fare currency_type is unknown ISO 4217 code ZZZ', 'FAR_003', 'replace', ('fare_attributes.txt', '2.50,USD,0,0,A', '2.50,ZZZ,0,0,A')),
    ('broken_route_reference', 'foreign_key', 'trips.route_id references a missing route', 'TRP_002', 'replace', ('trips.txt', 'R1,WK,T1,SH1', 'MISSING,WK,T1,SH1')),
    ('invalid_language_tag', 'language_tag', 'feed_info.feed_lang is invalid BCP 47 tag 123', 'FIN_003', 'replace', ('feed_info.txt', 'https://example.com,en,20260101', 'https://example.com,123,20260101')),
    ('html_tag', 'html_escape', 'stop_name contains an HTML script element', 'ARC_032', 'replace', ('stops.txt', 'S1,Start,41.000000', 'S1,<script>Start</script>,41.000000')),
    ('duplicate_stop_sequence', 'stop_sequence', 'two stop_times rows in one trip use the same stop_sequence', 'STM_032', 'replace', ('stop_times.txt', 'S2,2,10', 'S2,1,10')),
    ('agency_name_empty', 'required_field', 'agency_name is empty', 'AGN_002', 'replace', ('agency.txt', 'A,Test Transit,', 'A,,')),
    ('invalid_agency_timezone', 'timezone', 'agency_timezone is not an IANA timezone', 'AGN_004', 'replace', ('agency.txt', 'Europe/Istanbul,en', 'Mars/Olympus,en')),
    ('invalid_agency_language', 'language_tag', 'agency_lang is invalid BCP 47', 'AGN_006', 'replace', ('agency.txt', 'Europe/Istanbul,en', 'Europe/Istanbul,123')),
    ('route_names_empty', 'conditional_required', 'route_short_name and route_long_name are both empty', 'RTS_003', 'replace', ('routes.txt', 'R1,A,1,Test Route,3', 'R1,A,,,3')),
    ('invalid_route_type', 'route_type', 'route_type is outside the GTFS enum ranges', 'RTS_004', 'replace', ('routes.txt', 'Test Route,3', 'Test Route,99')),
    ('invalid_route_color', 'route_color', 'route_color is not a six-digit hexadecimal color', 'RTS_006', 'append_column', ('routes.txt', 'route_color', ['GGGGGG'])),
    ('negative_route_sort_order', 'route_sort_order', 'route_sort_order is negative', 'RTS_029', 'append_column', ('routes.txt', 'route_sort_order', ['-1'])),
    ('stop_id_empty', 'required_field', 'a stops.txt row has an empty stop_id', 'STP_002', 'replace', ('stops.txt', 'S1,Start,41.000000,29.000000', ',Start,41.000000,29.000000')),
    ('stop_lon_out_of_range', 'stop_lon', 'stop_lon is 181 degrees', 'STP_005', 'replace', ('stops.txt', 'S1,Start,41.000000,29.000000', 'S1,Start,41.000000,181.000000')),
    ('invalid_location_type', 'location_type', 'location_type is outside 0..4', 'STP_008', 'append_column', ('stops.txt', 'location_type', ['9', '0'])),
    ('invalid_stop_timezone', 'timezone', 'stop_timezone is not an IANA timezone', 'STP_014', 'append_column', ('stops.txt', 'stop_timezone', ['Mars/Olympus', 'Europe/Istanbul'])),
    ('trip_id_empty', 'required_field', 'a trips.txt row has an empty trip_id', 'TRP_001', 'replace', ('trips.txt', 'R1,WK,T1,SH1', 'R1,WK,,SH1')),
    ('trip_route_id_empty', 'required_field', 'a trips.txt row has an empty route_id', 'TRP_031', 'replace', ('trips.txt', 'R1,WK,T1,SH1', ',WK,T1,SH1')),
    ('trip_service_id_empty', 'required_field', 'a trips.txt row has an empty service_id', 'TRP_035', 'replace', ('trips.txt', 'R1,WK,T1,SH1', 'R1,,T1,SH1')),
    ('invalid_direction_id', 'direction_id', 'direction_id is 2 instead of 0 or 1', 'TRP_005', 'append_column', ('trips.txt', 'direction_id', ['2'])),
    ('invalid_calendar_day', 'calendar_day', 'calendar monday field is 2 instead of 0 or 1', 'CAL_002', 'replace', ('calendar.txt', 'WK,1,1,1,1,1,1,1,', 'WK,2,1,1,1,1,1,1,')),
    ('calendar_end_before_start', 'calendar_order', 'calendar end_date precedes start_date', 'CAL_005', 'replace', ('calendar.txt', '20260101,20261231', '20261231,20260101')),
    ('frequency_headway_zero', 'frequency', 'frequencies headway_secs is zero', 'FRQ_008', 'add_file', ('frequencies.txt', 'trip_id,start_time,end_time,headway_secs,exact_times\nT1,08:00:00,09:00:00,0,0\n')),
    ('calendar_date_month_13', 'calendar_date', 'calendar start_date has month 13', 'CAL_003', 'replace', ('calendar.txt', '20260101,20261231', '20261301,20261231')),
    ('calendar_date_day_00', 'calendar_date', 'calendar start_date has day 00', 'CAL_003', 'replace', ('calendar.txt', '20260101,20261231', '20260200,20261231')),
    ('calendar_date_day_32', 'calendar_date', 'calendar start_date has day 32', 'CAL_003', 'replace', ('calendar.txt', '20260101,20261231', '20260132,20261231')),
    ('calendar_date_short', 'calendar_date', 'calendar start_date has 7-digit date', 'CAL_003', 'replace', ('calendar.txt', '20260101,20261231', '2026011,20261231')),
    ('calendar_date_alpha', 'calendar_date', 'calendar start_date has non-numeric date', 'CAL_003', 'replace', ('calendar.txt', '20260101,20261231', '2026AB01,20261231')),
    ('time_minute_60', 'time_format', 'arrival/departure time has minute 60', 'STM_003', 'replace', ('stop_times.txt', 'T1,08:00:00,08:00:00', 'T1,08:60:00,08:60:00')),
    ('time_second_60', 'time_format', 'arrival/departure time has second 60', 'STM_003', 'replace', ('stop_times.txt', 'T1,08:00:00,08:00:00', 'T1,08:00:60,08:00:60')),
    ('time_negative_hour', 'time_format', 'arrival/departure time has negative hour', 'STM_003', 'replace', ('stop_times.txt', 'T1,08:00:00,08:00:00', 'T1,-1:00:00,-1:00:00')),
    ('time_alpha', 'time_format', 'arrival/departure time has alphabetic hour', 'STM_003', 'replace', ('stop_times.txt', 'T1,08:00:00,08:00:00', 'T1,AA:00:00,AA:00:00')),
    ('time_missing_seconds', 'time_format', 'arrival/departure time has missing seconds', 'STM_003', 'replace', ('stop_times.txt', 'T1,08:00:00,08:00:00', 'T1,08:00,08:00')),
    ('time_decimal', 'time_format', 'arrival/departure time has fractional seconds', 'STM_003', 'replace', ('stop_times.txt', 'T1,08:00:00,08:00:00', 'T1,08:00:00.5,08:00:00.5')),
    ('stop_lat_91', 'stop_lat', 'stops.stop_lat has latitude > 90', 'STP_004', 'replace', ('stops.txt', 'S1,Start,41.000000,29.000000', 'S1,Start,91.000000,29.000000')),
    ('stop_lat_minus91', 'stop_lat', 'stops.stop_lat has latitude < -90', 'STP_004', 'replace', ('stops.txt', 'S1,Start,41.000000,29.000000', 'S1,Start,-91.000000,29.000000')),
    ('stop_lat_inf', 'stop_lat', 'stops.stop_lat has infinite latitude', 'STP_004', 'replace', ('stops.txt', 'S1,Start,41.000000,29.000000', 'S1,Start,inf,29.000000')),
    ('stop_lat_text', 'stop_lat', 'stops.stop_lat has non-numeric latitude', 'STP_004', 'replace', ('stops.txt', 'S1,Start,41.000000,29.000000', 'S1,Start,north,29.000000')),
    ('stop_lon_minus181', 'stop_lon', 'stops.stop_lon has longitude < -180', 'STP_005', 'replace', ('stops.txt', 'S1,Start,41.000000,29.000000', 'S1,Start,41.000000,-181.000000')),
    ('stop_lon_nan', 'stop_lon', 'stops.stop_lon has NaN longitude', 'STP_005', 'replace', ('stops.txt', 'S1,Start,41.000000,29.000000', 'S1,Start,41.000000,NaN')),
    ('stop_lon_inf', 'stop_lon', 'stops.stop_lon has infinite longitude', 'STP_005', 'replace', ('stops.txt', 'S1,Start,41.000000,29.000000', 'S1,Start,41.000000,inf')),
    ('stop_lon_text', 'stop_lon', 'stops.stop_lon has non-numeric longitude', 'STP_005', 'replace', ('stops.txt', 'S1,Start,41.000000,29.000000', 'S1,Start,41.000000,east')),
    ('agency_url_bad_percent', 'url', 'agency_url has malformed percent escape', 'AGN_003', 'replace', ('agency.txt', 'https://example.com,Europe/Istanbul', 'https://example.com/%ZZ,Europe/Istanbul')),
    ('agency_url_backslash', 'url', 'agency_url contains a raw backslash', 'AGN_003', 'replace', ('agency.txt', 'https://example.com,Europe/Istanbul', 'https://example.com/a\\b,Europe/Istanbul')),
    ('agency_url_nonascii', 'url', 'agency_url contains raw non-ASCII instead of percent encoding', 'AGN_003', 'replace', ('agency.txt', 'https://example.com,Europe/Istanbul', 'https://example.com/güzergah,Europe/Istanbul')),
    ('agency_url_no_scheme', 'url', 'agency_url has no http/https scheme', 'AGN_003', 'replace', ('agency.txt', 'https://example.com,Europe/Istanbul', 'example.com,Europe/Istanbul')),
    ('feed_publisher_url_space', 'url', 'feed_publisher_url contains unescaped space', None, 'replace', ('feed_info.txt', 'Test Publisher,https://example.com,en', 'Test Publisher,https://example.com/a b,en')),
    ('feed_contact_url_space', 'url', 'feed_contact_url contains unescaped space', None, 'replace', ('feed_info.txt', 'contact@example.com,https://example.com/contact', 'contact@example.com,https://example.com/a b')),
    ('feed_lang_singleton', 'language_tag', 'feed_lang has dangling extension singleton', 'FIN_003', 'replace', ('feed_info.txt', 'https://example.com,en,20260101', 'https://example.com,en-a,20260101')),
    ('feed_lang_empty_extension', 'language_tag', 'feed_lang has extension singleton without valid payload', 'FIN_003', 'replace', ('feed_info.txt', 'https://example.com,en,20260101', 'https://example.com,en-a-b-c,20260101')),
    ('feed_lang_repeat_singleton', 'language_tag', 'feed_lang has repeated extension singleton', 'FIN_003', 'replace', ('feed_info.txt', 'https://example.com,en,20260101', 'https://example.com,en-a-bbb-a-ccc,20260101')),
    ('feed_lang_bad_char', 'language_tag', 'feed_lang has underscore instead of BCP47 hyphen', 'FIN_003', 'replace', ('feed_info.txt', 'https://example.com,en,20260101', 'https://example.com,en_XX,20260101')),
    ('agency_lang_singleton', 'language_tag', 'agency_lang has dangling extension singleton', 'AGN_006', 'replace', ('agency.txt', 'Europe/Istanbul,en', 'Europe/Istanbul,en-a')),
    ('agency_lang_repeat', 'language_tag', 'agency_lang has repeated extension singleton', 'AGN_006', 'replace', ('agency.txt', 'Europe/Istanbul,en', 'Europe/Istanbul,en-a-bbb-a-ccc')),
    ('agency_lang_bad_char', 'language_tag', 'agency_lang has underscore instead of BCP47 hyphen', 'AGN_006', 'replace', ('agency.txt', 'Europe/Istanbul,en', 'Europe/Istanbul,en_XX')),
    ('agency_lang_double_hyphen', 'language_tag', 'agency_lang has empty subtag', 'AGN_006', 'replace', ('agency.txt', 'Europe/Istanbul,en', 'Europe/Istanbul,en--US')),
    ('html_b_tag', 'html_escape', 'stop_name contains HTML b tag', 'ARC_032', 'replace', ('stops.txt', 'S1,Start,41.000000', 'S1,<b>Start</b>,41.000000')),
    ('html_comment', 'html_escape', 'stop_name contains HTML comment', 'ARC_032', 'replace', ('stops.txt', 'S1,Start,41.000000', 'S1,<!--x-->Start,41.000000')),
    ('html_entity_copy', 'html_escape', 'stop_name contains HTML5 named entity &copy;', 'ARC_032', 'replace', ('stops.txt', 'S1,Start,41.000000', 'S1,Start &copy;,41.000000')),
    ('html_entity_reg', 'html_escape', 'stop_name contains HTML5 named entity &reg;', 'ARC_032', 'replace', ('stops.txt', 'S1,Start,41.000000', 'S1,Start &reg;,41.000000')),
    ('html_entity_euro', 'html_escape', 'stop_name contains HTML5 named entity &euro;', 'ARC_032', 'replace', ('stops.txt', 'S1,Start,41.000000', 'S1,Start &euro;,41.000000')),
    ('html_entity_copy_no_semicolon', 'html_escape', 'stop_name contains HTML5 semicolon-less entity form', 'ARC_032', 'replace', ('stops.txt', 'S1,Start,41.000000', 'S1,Start &copy,41.000000')),
    ('quote_after_closing', 'csv_quote', 'characters follow a closing quote in stop_name', 'ARC_033', 'replace', ('stops.txt', 'S1,Start,41.000000', 'S1,"Start"X,41.000000')),
    ('unclosed_quote', 'csv_quote', 'stop_name starts a quoted field that never closes', 'ARC_013', 'replace', ('stops.txt', 'S1,Start,41.000000', 'S1,"Start,41.000000')),
    ('bare_quote_second_row', 'csv_quote', 'second stop_name contains a bare quotation mark', 'ARC_033', 'replace', ('stops.txt', 'S2,End,41.000050', 'S2,En"d,41.000050')),
    ('currency_AAA', 'currency', "currency_type is invalid value 'AAA'", None, 'replace', ('fare_attributes.txt', '2.50,USD,0,0,A', '2.50,AAA,0,0,A')),
    ('currency_123', 'currency', "currency_type is invalid value '123'", None, 'replace', ('fare_attributes.txt', '2.50,USD,0,0,A', '2.50,123,0,0,A')),
    ('currency_lowercase_usd', 'currency', "currency_type is invalid value 'usd'", None, 'replace', ('fare_attributes.txt', '2.50,USD,0,0,A', '2.50,usd,0,0,A')),
    ('currency_empty', 'currency', "currency_type is invalid value ''", None, 'replace', ('fare_attributes.txt', '2.50,USD,0,0,A', '2.50,,0,0,A')),
    ('route_type_8', 'route_type', 'route_type is invalid value 8', 'RTS_004', 'replace', ('routes.txt', 'Test Route,3', 'Test Route,8')),
    ('route_type_10', 'route_type', 'route_type is invalid value 10', 'RTS_004', 'replace', ('routes.txt', 'Test Route,3', 'Test Route,10')),
    ('route_type_13', 'route_type', 'route_type is invalid value 13', 'RTS_004', 'replace', ('routes.txt', 'Test Route,3', 'Test Route,13')),
    ('route_type_negative', 'route_type', 'route_type is invalid value -1', 'RTS_004', 'replace', ('routes.txt', 'Test Route,3', 'Test Route,-1')),
    ('route_color_hash', 'route_color', "route_color is invalid hex string '#FFFFFF'", 'RTS_006', 'append_column', ('routes.txt', 'route_color', ['#FFFFFF'])),
    ('route_color_short', 'route_color', "route_color is invalid hex string 'FFFFF'", 'RTS_006', 'append_column', ('routes.txt', 'route_color', ['FFFFF'])),
    ('route_color_long', 'route_color', "route_color is invalid hex string 'FFFFFFF'", 'RTS_006', 'append_column', ('routes.txt', 'route_color', ['FFFFFFF'])),
    ('route_color_bad_hex', 'route_color', "route_color is invalid hex string 'ABCDEG'", 'RTS_006', 'append_column', ('routes.txt', 'route_color', ['ABCDEG'])),
    ('direction_negative', 'direction_id', "direction_id is invalid value '-1'", 'TRP_005', 'append_column', ('trips.txt', 'direction_id', ['-1'])),
    ('direction_text', 'direction_id', "direction_id is invalid value 'north'", 'TRP_005', 'append_column', ('trips.txt', 'direction_id', ['north'])),
    ('direction_three', 'direction_id', "direction_id is invalid value '3'", 'TRP_005', 'append_column', ('trips.txt', 'direction_id', ['3'])),
    ('calendar_monday_negative', 'calendar_day', 'calendar monday is -1', 'CAL_002', 'replace', ('calendar.txt', 'WK,1,1,1,1,1,1,1,', 'WK,-1,1,1,1,1,1,1,')),
    ('calendar_monday_true', 'calendar_day', 'calendar monday is literal true', 'CAL_002', 'replace', ('calendar.txt', 'WK,1,1,1,1,1,1,1,', 'WK,true,1,1,1,1,1,1,')),
    ('calendar_tuesday_2', 'calendar_day', 'calendar tuesday is 2', 'CAL_002', 'replace', ('calendar.txt', 'WK,1,1,1,1,1,1,1,', 'WK,1,2,1,1,1,1,1,')),
    ('calendar_sunday_9', 'calendar_day', 'calendar sunday is 9', 'CAL_002', 'replace', ('calendar.txt', 'WK,1,1,1,1,1,1,1,', 'WK,1,1,1,1,1,1,9,')),
    ('location_type_5', 'location_type', "location_type is invalid value '5'", 'STP_008', 'append_column', ('stops.txt', 'location_type', ['5', '0'])),
    ('location_type_negative', 'location_type', "location_type is invalid value '-1'", 'STP_008', 'append_column', ('stops.txt', 'location_type', ['-1', '0'])),
    ('location_type_text', 'location_type', "location_type is invalid value 'station'", 'STP_008', 'append_column', ('stops.txt', 'location_type', ['station', '0'])),
    ('location_type_99', 'location_type', "location_type is invalid value '99'", 'STP_008', 'append_column', ('stops.txt', 'location_type', ['99', '0'])),
    ('agency_timezone_nope', 'timezone', 'agency_timezone is nonexistent IANA id Europe/Nope', 'AGN_004', 'replace', ('agency.txt', 'Europe/Istanbul,en', 'Europe/Nope,en')),
    ('agency_timezone_empty', 'timezone', 'required agency_timezone is empty', None, 'replace', ('agency.txt', 'https://example.com,Europe/Istanbul,en', 'https://example.com,,en')),
    ('stop_timezone_nope', 'timezone', 'stop_timezone is nonexistent IANA id Europe/Nope', 'STP_014', 'append_column', ('stops.txt', 'stop_timezone', ['Europe/Nope', 'Europe/Istanbul'])),
    ('stop_timezone_offset', 'timezone', 'stop_timezone uses raw UTC offset instead of IANA id', 'STP_014', 'append_column', ('stops.txt', 'stop_timezone', ['UTC+03:00', 'Europe/Istanbul'])),
    ('route_sort_order_text', 'route_sort_order', 'route_sort_order is non-integer text', 'RTS_029', 'append_column', ('routes.txt', 'route_sort_order', ['abc'])),
]
assert len(CASES_RAW) == 100, len(CASES_RAW)
assert len({row[0] for row in CASES_RAW}) == 100

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
    manifest = [{
        "id": "baseline",
        "group": "baseline",
        "description": "unmodified control feed",
        "expected_analyzer_rule": None,
        "control": True,
    }]

    # Paired valid controls for code paths that produced process-level failures in the 30-case pilot.
    valid_location = dict(BASE)
    append_column(valid_location, "stops.txt", "location_type", ["0", "0"])
    write_zip("control_valid_location_type", valid_location)
    manifest.append({
        "id": "control_valid_location_type",
        "group": "control",
        "description": "valid location_type column (0,0)",
        "expected_analyzer_rule": None,
        "control": True,
    })

    valid_frequency = dict(BASE)
    valid_frequency["frequencies.txt"] = """trip_id,start_time,end_time,headway_secs,exact_times
T1,08:00:00,09:00:00,600,0
"""
    write_zip("control_valid_frequency", valid_frequency)
    manifest.append({
        "id": "control_valid_frequency",
        "group": "control",
        "description": "valid frequencies.txt with headway_secs=600",
        "expected_analyzer_rule": None,
        "control": True,
    })

    for case_id, group, description, expected_rule, kind, args in CASES_RAW:
        files = dict(BASE)
        mutation(kind, args)(files)
        write_zip(case_id, files)
        manifest.append({
            "id": case_id,
            "group": group,
            "description": description,
            "expected_analyzer_rule": expected_rule,
            "control": False,
        })

    (FIXTURES / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    groups = sorted({x["group"] for x in manifest if not x["control"]})
    print(f"generated 100 mutant fixtures + baseline + 2 paired controls in {FIXTURES}")
    print(f"semantic groups: {len(groups)} -> {', '.join(groups)}")

if __name__ == "__main__":
    main()
