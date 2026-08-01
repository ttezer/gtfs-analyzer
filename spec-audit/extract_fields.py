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

    CAPTURE_COLS = (0, 1, 2)  # Field Name · Type · Presence (Description alınmaz)

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.table_depth = 0
        self.cell_index = -1        # satırdaki hücre sırası (dış tabloda)
        self.capturing = False      # şu an bir hücrenin metnini topluyor muyuz
        self.buf: list[str] = []
        self.header: list[str] = [] # dış tablonun <th> metinleri
        self.in_header_cell = False
        self.row: dict[int, str] = {}
        self.rows: list[dict[int, str]] = []  # dış satırların {sütun: metin}

    def handle_starttag(self, tag, attrs):
        if tag == "table":
            self.table_depth += 1
        elif self.table_depth == 1 and tag == "tr":
            if self.row:
                self.rows.append(self.row)
            self.row = {}
            self.cell_index = -1
        elif self.table_depth == 1 and tag in ("td", "th"):
            self.cell_index += 1
            if tag == "th":
                self.in_header_cell = True
                self.buf = []
            elif self.cell_index in self.CAPTURE_COLS:
                self.capturing = True
                self.buf = []

    def handle_endtag(self, tag):
        if tag == "table":
            self.table_depth -= 1
            if self.table_depth == 0 and self.row:
                self.rows.append(self.row)
                self.row = {}
        elif tag == "th" and self.in_header_cell:
            self.in_header_cell = False
            self.header.append(" ".join("".join(self.buf).split()))
        elif tag == "td" and self.capturing:
            self.capturing = False
            self.row[self.cell_index] = " ".join("".join(self.buf).split())

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


def fields_in(section: str) -> list[dict[str, str]]:
    """Bölümdeki "Field Name" başlıklı tabloların Field/Type/Presence sütunları."""
    found: list[dict[str, str]] = []
    seen: set[str] = set()
    for table in outer_tables(section):
        p = FieldTableParser()
        p.feed(table)
        if not p.header or p.header[0].lower() not in ("field name", "field"):
            continue
        for row in p.rows:
            name = row.get(0, "").strip("`").strip()
            if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_.]*", name) or name in seen:
                continue
            seen.add(name)
            found.append({
                "name": name,
                "type": row.get(1, "").strip(),
                "presence": row.get(2, "").strip(),
            })
    return found


def primary_key_of(section: str) -> list[str]:
    """Bölüm başındaki "Primary key (…)" satırı → alan listesi.

    `(*)` = tüm satır, `(none)` = anahtar yok. İkisi de olduğu gibi korunur;
    yorumu tüketiciye bırakılır (DQ_021 bu ayrımı zaten kullanıyor).
    """
    text = html.unescape(re.sub(r"<[^>]+>", " ", section))
    # Spec'in KENDİSİ tutarsız: çoğu bölümde "Primary key", fare_leg_join_rules.txt'te
    # "Primary Key". Büyük/küçük harfe duyarlı arama o dosyayı anahtarsız gösteriyordu.
    m = re.search(r"Primary key \(([^)]*)\)", text, re.I)
    if not m:
        return []
    raw = m.group(1).strip()
    if raw in ("*", "none"):
        return [raw]
    return [p.strip().strip("`") for p in raw.split(",") if p.strip()]


def main() -> int:
    doc = fetch(sys.argv)
    table: dict[str, dict] = {}
    for name, start, stop in section_bounds(doc):
        section = doc[start:stop]
        fields = fields_in(section)
        if fields:
            table[name] = {"primary_key": primary_key_of(section), "fields": fields}
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
    total = sum(len(v["fields"]) for v in table.values())
    no_type = sum(1 for v in table.values() for f in v["fields"] if not f["type"])
    no_pres = sum(1 for v in table.values() for f in v["fields"] if not f["presence"])
    print(f"yazıldı: {OUT.name} ({len(table)} dosya, {total} alan)")
    if no_type or no_pres:
        print(f"  ⚠️ type boş: {no_type} · presence boş: {no_pres} (spec düzeni değişmiş olabilir)",
              file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
