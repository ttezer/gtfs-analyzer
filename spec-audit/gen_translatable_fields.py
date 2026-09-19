#!/usr/bin/env python3
"""translations.txt::field_name için spec TÜRÜNÜ taşıyan tabloyu üretir.

GTFS'in tek hükmü tür eksenindedir (`P7c7134fe`, soft): *"Fields with other types
should not be translated."* Çevrilebilir türler Text, URL, Email ve Phone number'dır.
`TRN_011` bu tabloya sorar; el yapımı ad sezgisi (alan adında "name"/"url" aramak)
spec'i temsil etmiyordu ve `stops::stop_code` gibi Text alanlarda yanlış konuşuyordu.

Kaynak `spec-audit/spec_fields.json` — o da üretilmiş bir dosyadır
(`spec-audit/extract_fields.py`), yani zincir spec referansına dayanır.

Kullanım:
  python3 spec-audit/gen_translatable_fields.py           # tabloyu yaz
  python3 spec-audit/gen_translatable_fields.py --check    # drift kapısı
"""

import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "spec-audit/spec_fields.json"
OUT = ROOT / "crates/pipeline/src/k2/translatable_fields_generated.rs"

# `translations.txt::table_name` resmi GTFS şemasındaki tablo adına karşılık gelir.
# `translations.txt` kendi satırlarını çevrilecek kaynak tablo olarak sunmaz; diğer
# tablolar katalogdan dinamik olarak alınır. Böylece yeni resmi tablolar (ör.
# `location_groups.txt`) ayrıca elle whitelist'e eklenmek zorunda kalmaz.
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
    text = f"""//! ÜRETİLMİŞ DOSYA — elle düzenlemeyin.
//!
//! Kaynak  : `spec-audit/spec_fields.json` (o da üretilmiş: `spec-audit/extract_fields.py`)
//! Üretim  : `python3 spec-audit/gen_translatable_fields.py`
//! Drift   : `python3 spec-audit/gen_translatable_fields.py --check`
//!
//! Spec'in `translations.txt::field_name` hakkındaki TEK hükmü tür eksenindedir
//! (`P7c7134fe`, soft): *"Fields with other types should not be translated."*
//! Çevrilebilir türler: {", ".join(sorted(TRANSLATABLE_TYPES))}.
//!
//! ⚠️ Tabloda OLMAYAN bir alan hakkında bu dosya hüküm vermez. Uzantı alanları
//! (`jp_trip_desc_symbol` gibi) spec kataloğunda yoktur ve türleri bilinmez;
//! `spec_field_is_translatable` orada `None` döner ve çağıran SUSMALIDIR.

/// `translations.txt::table_name` için resmi GTFS tablo adları.
pub(crate) const SPEC_TRANSLATION_TABLES: &[&str] = &[
{chr(10).join(f'    "{t}",' for t in tables)}
];

/// (tablo, alan, çevrilebilir tür mü) — (tablo, alan)'a göre SIRALI;
/// `binary_search_by` bunu varsayar.
pub(crate) const SPEC_TRANSLATION_FIELDS: &[(&str, &str, bool)] = &[
{body}
];

/// `Some(true)` çevrilebilir tür · `Some(false)` bilinen ama çevrilemez tür ·
/// `None` spec kataloğunda yok (uzantı alanı) → hüküm verilemez.
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
            print(f"translatable_fields OK — {len(rows)} alan, {yes} çevrilebilir")
            return 0
        added = sorted(new_rows - cur_rows)
        gone = sorted(cur_rows - new_rows)
        print("🔴 TRANSLATABLE FIELDS DRIFT")
        if added:
            print(f"  eklenen/değişen: {added[:12]}")
        if gone:
            print(f"  kaybolan/eski  : {gone[:12]}")
        added_tables = sorted(new_tables - cur_tables)
        gone_tables = sorted(cur_tables - new_tables)
        if added_tables:
            print(f"  eklenen tablolar: {added_tables[:12]}")
        if gone_tables:
            print(f"  kaldırılan tablolar: {gone_tables[:12]}")
        print("  → python3 spec-audit/gen_translatable_fields.py ile tabloyu yenileyin.")
        return 1
    OUT.write_text(text, encoding="utf-8")
    print(f"yazıldı: {OUT.name} ({len(rows)} alan · {yes} çevrilebilir)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
