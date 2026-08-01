#!/usr/bin/env python3
"""GTFS Schedule Reference'tan dosya → alan tablosunu çıkarır (WP-2 çapası).

Neden: `GtfsSpec` otoriteli bir kural, spec'te var OLMAYAN bir alanı ölçüyorsa
sınıfı yanlıştır (RCT_004 `min_age` vakası — bkz. CHANGELOG 2026-08-01). Elle
tutulan bir alan listesi bayatlar; bu betik listeyi spec'in KENDİSİNDEN üretir,
`spec_fields.json`'a yazar ve o dosya repoda sabitlenir (CI ağa çıkmaz).

Çalıştır:
    python3 spec-audit/extract_fields.py            # ağdan indirir
    python3 spec-audit/extract_fields.py spec.html  # yerel kopyadan

Ayrıştırma DÜZYAZIDAN DEĞİL, HTML TABLOSUNDAN yapılır: her dosya bölümünde
başlığı "Field Name" olan tabloların ilk sütunu alınır. Düzyazı regex'i
`accompanying` gibi kelimeleri alan sanıyordu — bu yanlış-YEŞİL yönünde hata,
yani kapıyı sessizce zayıflatır.

⚠️ TABLOLAR İÇ İÇEDİR ve regex ile ayrıştırılamaz: `fare_transfer_type` gibi
alanların açıklaması kendi değer tablosunu taşır. `<table>.*?</table>` iç tablonun
kapanışında durup dış tabloyu kesiyordu — `fare_transfer_rules.fare_product_id`
listeden düşmüştü. Bu yüzden gerçek bir HTML ayrıştırıcı (stdlib `HTMLParser`)
kullanılır ve yalnız DIŞ tablonun DIŞ satırları sayılır.
"""
import html
import json
import pathlib
import re
import sys
import urllib.request
from html.parser import HTMLParser

SPEC_URL = "https://gtfs.org/documentation/schedule/reference/"
OUT = pathlib.Path(__file__).resolve().parent / "spec_fields.json"


def fetch(argv: list[str]) -> str:
    if len(argv) > 1:
        return pathlib.Path(argv[1]).read_text(encoding="utf-8")
    with urllib.request.urlopen(SPEC_URL, timeout=60) as r:
        return r.read().decode("utf-8")


class FieldTableParser(HTMLParser):
    """Yalnız DIŞ tabloların DIŞ satırlarının İLK hücresini toplar.

    `table_depth`: iç içe tabloları saymak için. Bir alanın açıklaması kendi
    değer tablosunu taşıyabilir (`fare_transfer_type` → 0/1/2); o satırlar
    alan DEĞİLDİR ve derinlik 2'de kaldıkları için atlanır.
    """

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.table_depth = 0
        self.cell_index = -1        # satırdaki hücre sırası (dış tabloda)
        self.capturing = False      # şu an ilk hücrenin metnini topluyor muyuz
        self.buf: list[str] = []
        self.header: list[str] = [] # dış tablonun <th> metinleri
        self.in_header_cell = False
        self.rows: list[str] = []   # dış satırların ilk hücre metinleri

    def handle_starttag(self, tag, attrs):
        if tag == "table":
            self.table_depth += 1
        elif self.table_depth == 1 and tag == "tr":
            self.cell_index = -1
        elif self.table_depth == 1 and tag in ("td", "th"):
            self.cell_index += 1
            if tag == "th":
                self.in_header_cell = True
                self.buf = []
            elif self.cell_index == 0:
                self.capturing = True
                self.buf = []

    def handle_endtag(self, tag):
        if tag == "table":
            self.table_depth -= 1
        elif tag == "th" and self.in_header_cell:
            self.in_header_cell = False
            self.header.append("".join(self.buf).strip())
        elif tag == "td" and self.capturing:
            self.capturing = False
            self.rows.append("".join(self.buf).strip())

    def handle_data(self, data):
        if self.capturing or self.in_header_cell:
            self.buf.append(data)


def section_bounds(doc: str) -> list[tuple[str, int, int]]:
    """(dosya_adı, başlangıç, bitiş) — h3 çapalarından dosya bölümleri."""
    heads = [
        (m.group(1), m.start(), m.end())
        for m in re.finditer(r'<h3[^>]*id="([^"]+)"[^>]*>', doc)
    ]
    out = []
    for i, (anchor, start, end) in enumerate(heads):
        stop = heads[i + 1][1] if i + 1 < len(heads) else len(doc)
        if anchor.endswith("txt"):
            name = anchor[:-3] + ".txt"
        elif anchor.endswith("geojson"):
            name = anchor[: -len("geojson")] + ".geojson"
        else:
            continue
        out.append((name, end, stop))
    return out


def outer_tables(section: str) -> list[str]:
    """Bölümdeki DIŞ `<table>…</table>` bloklarını derinlik sayarak ayırır."""
    out, depth, start = [], 0, None
    for m in re.finditer(r"</?table\b[^>]*>", section):
        if m.group(0).startswith("</"):
            depth -= 1
            if depth == 0 and start is not None:
                out.append(section[start:m.end()])
                start = None
        else:
            if depth == 0:
                start = m.start()
            depth += 1
    return out


def fields_in(section: str) -> list[str]:
    """Bölümdeki "Field Name" başlıklı tabloların ilk sütunu."""
    found: list[str] = []
    for table in outer_tables(section):
        p = FieldTableParser()
        p.feed(table)
        if not p.header or p.header[0].lower() not in ("field name", "field"):
            continue
        for name in p.rows:
            name = name.strip("`").strip()
            if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_.]*", name):
                found.append(name)
    # sıra korunarak tekilleştir
    return list(dict.fromkeys(found))


def main() -> int:
    doc = fetch(sys.argv)
    table: dict[str, list[str]] = {}
    for name, start, stop in section_bounds(doc):
        fields = fields_in(doc[start:stop])
        if fields:
            table[name] = fields
    if len(table) < 25:
        print(f"HATA: yalnız {len(table)} dosya bulundu; spec düzeni değişmiş olabilir.",
              file=sys.stderr)
        return 1
    OUT.write_text(
        json.dumps(
            {
                "_source": SPEC_URL,
                "_note": "Üretilmiş dosya — elle düzenlemeyin. Yeniden üretmek için: "
                         "python3 spec-audit/extract_fields.py",
                "files": table,
            },
            ensure_ascii=False,
            indent=2,
            sort_keys=False,
        )
        + "\n",
        encoding="utf-8",
    )
    total = sum(len(v) for v in table.values())
    print(f"yazıldı: {OUT.name} ({len(table)} dosya, {total} alan)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
