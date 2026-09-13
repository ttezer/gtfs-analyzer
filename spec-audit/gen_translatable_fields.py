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

# `translations.txt::table_name` yalnız bu dokuz tabloyu kabul eder.
TABLES = [
    "agency",
    "stops",
    "routes",
    "trips",
    "stop_times",
    "feed_info",
    "attributions",
    "pathways",
    "levels",
]

TRANSLATABLE_TYPES = {"Text", "URL", "Email", "Phone number"}


def build():
    spec = json.loads(SRC.read_text(encoding="utf-8"))
    files = spec["files"]
    rows = []
    for table in TABLES:
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
        if cur_rows == new_rows:
            print(f"translatable_fields OK — {len(rows)} alan, {yes} çevrilebilir")
            return 0
        added = sorted(new_rows - cur_rows)
        gone = sorted(cur_rows - new_rows)
        print("🔴 TRANSLATABLE FIELDS DRIFT")
        if added:
            print(f"  eklenen/değişen: {added[:12]}")
        if gone:
            print(f"  kaybolan/eski  : {gone[:12]}")
        print("  → python3 spec-audit/gen_translatable_fields.py ile tabloyu yenileyin.")
        return 1
    OUT.write_text(text, encoding="utf-8")
    print(f"yazıldı: {OUT.name} ({len(rows)} alan · {yes} çevrilebilir)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
