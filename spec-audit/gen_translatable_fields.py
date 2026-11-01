#!/usr/bin/env python3
"""Generate the table that carries the spec TYPE for translations.txt::field_name.

GTFS's only rule concerns the type axis (`P7c7134fe`, soft): *"Fields with other types
should not be translated."* Translatable types are Text, URL, Email, and Phone number.
`TRN_011` queries this table; hand-written name heuristics (looking for "name"/"url" in
the field name) did not represent the spec and spoke incorrectly for Text fields such as
`stops::stop_code`.

Source: `spec-audit/spec_fields.json` — it is also a generated file
(`spec-audit/extract_fields.py`), so the chain is anchored to the spec reference.

Usage:
  python3 spec-audit/gen_translatable_fields.py           # write the table
  python3 spec-audit/gen_translatable_fields.py --check    # drift gate
"""

import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "spec-audit/spec_fields.json"
OUT = ROOT / "crates/pipeline/src/k2/translatable_fields_generated.rs"

# `translations.txt::table_name` maps to a table name in the official GTFS schema.
# `translations.txt` does not offer its own rows as source tables for translation; the other
# tables are loaded dynamically from the catalog. Thus new official tables (e.g.
# `location_groups.txt`) do not need to be added to the whitelist by hand.
EXCLUDED_TABLES = {"translations"}

TRANSLATABLE_TYPES = {"Text", "URL", "Email", "Phone number"}


def build():
    spec = json.loads(SRC.read_text(encoding="utf-8"))
    files = spec["files"]
    tables = sorted(
        name.removesuffix(".txt")
        for name in files
        if name.endswith(".txt") and name.removesuffix(".txt") not in EXCLUDED_TABLES
    )
    rows = []
    for table in tables:
        entry = files.get(f"{table}.txt")
        if entry is None:
            raise SystemExit(f"spec_fields.json içinde {table}.txt yok")
        for field in entry["fields"]:
            rows.append((table, field["name"], field["type"] in TRANSLATABLE_TYPES))
    rows.sort(key=lambda r: (r[0], r[1]))
    yes = sum(1 for r in rows if r[2])
    body = "\n".join(f'    ("{t}", "{f}", {str(ok).lower()}),' for t, f, ok in rows)
    text = f"""//! GENERATED FILE — do not edit by hand.
//!
//! Source  : `spec-audit/spec_fields.json` (it is also generated: `spec-audit/extract_fields.py`)
//! Generation: `python3 spec-audit/gen_translatable_fields.py`
//! Drift   : `python3 spec-audit/gen_translatable_fields.py --check`
//!
//! The spec's ONLY rule for `translations.txt::field_name` concerns the type axis
//! (`P7c7134fe`, soft): *"Fields with other types should not be translated."*
//! Translatable types: {", ".join(sorted(TRANSLATABLE_TYPES))}.
//!
//! ⚠️ This file makes no ruling about a field NOT IN THE TABLE. Extension fields
//! (such as `jp_trip_desc_symbol`) are absent from the spec catalog and their types are unknown;
//! `spec_field_is_translatable` returns `None` there and the caller MUST STAY SILENT.

/// Official GTFS table names for `translations.txt::table_name`.
pub(crate) const SPEC_TRANSLATION_TABLES: &[&str] = &[
{chr(10).join(f'    "{t}",' for t in tables)}
];

/// (table, field, translatable type?) — SORTED by (table, field);
/// `binary_search_by` relies on this.
pub(crate) const SPEC_TRANSLATION_FIELDS: &[(&str, &str, bool)] = &[
{body}
];

/// `Some(true)` translatable type · `Some(false)` known but non-translatable type ·
/// `None` absent from the spec catalog (extension field) → no ruling is possible.
pub(crate) fn spec_field_is_translatable(table: &str, field: &str) -> Option<bool> {{
    SPEC_TRANSLATION_FIELDS
        .binary_search_by(|(t, f, _)| (*t, *f).cmp(&(table, field)))
        .ok()
        .map(|i| SPEC_TRANSLATION_FIELDS[i].2)
}}

pub(crate) fn spec_translation_table_is_known(table: &str) -> bool {{
    SPEC_TRANSLATION_TABLES.binary_search(&table).is_ok()
}}
"""
    return text, rows, yes


def main():
    check = "--check" in sys.argv[1:]
    text, rows, yes = build()
    pattern = r'\("([a-z_]+)", "([A-Za-z0-9_]+)", (true|false)\)'
    if check:
        cur = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
        cur_rows = set(re.findall(pattern, cur))
        new_rows = set(re.findall(pattern, text))
        table_pattern = r'SPEC_TRANSLATION_TABLES:.*?= &\[\n(.*?)\n\];'
        table_row_pattern = r'^\s+"([a-z_]+)",$'
        cur_table_block = re.search(table_pattern, cur, flags=re.S)
        new_table_block = re.search(table_pattern, text, flags=re.S)
        cur_tables = set(re.findall(table_row_pattern, cur_table_block.group(1))) if cur_table_block else set()
        new_tables = set(re.findall(table_row_pattern, new_table_block.group(1))) if new_table_block else set()
        if cur_rows == new_rows and cur_tables == new_tables:
            print(f"translatable_fields OK — {len(rows)} fields, {yes} translatable")
            return 0
        added = sorted(new_rows - cur_rows)
        gone = sorted(cur_rows - new_rows)
        print("🔴 TRANSLATABLE FIELDS DRIFT")
        if added:
            print(f"  added/changed: {added[:12]}")
        if gone:
            print(f"  kaybolan/eski  : {gone[:12]}")
        added_tables = sorted(new_tables - cur_tables)
        gone_tables = sorted(cur_tables - new_tables)
        if added_tables:
            print(f"  eklenen tablolar: {added_tables[:12]}")
        if gone_tables:
            print(f"  removed tables: {gone_tables[:12]}")
        print("  → python3 spec-audit/gen_translatable_fields.py ile tabloyu yenileyin.")
        return 1
    OUT.write_text(text, encoding="utf-8")
    print(f"wrote: {OUT.name} ({len(rows)} fields · {yes} translatable)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
