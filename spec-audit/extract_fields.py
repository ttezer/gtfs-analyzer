#!/usr/bin/env python3
"""Extract the file → field table from the GTFS Schedule Reference (WP-2 anchor).

Why: if an authoritative `GtfsSpec` rule measures a field that does NOT exist in the spec,
its class is wrong (RCT_004 `min_age` case — see CHANGELOG 2026-08-01). A manually maintained
field list goes stale; this script generates the list from the spec ITSELF,
`spec_fields.json`'a yazar ve o dosya repoda sabitlenir (CI ağa çıkmaz).

Run:
    python3 spec-audit/extract_fields.py            # download from the network
    python3 spec-audit/extract_fields.py spec.html  # use a local copy

Parsing uses the HTML TABLE, NOT PROSE: in each file section,
başlığı "Field Name" olan tabloların ilk sütunu alınır. Düzyazı regex'i
`accompanying` gibi kelimeleri alan sanıyordu — bu yanlış-YEŞİL yönünde hata,
yani kapıyı sessizce zayıflatır.

⚠️ TABLES ARE NESTED and cannot be parsed with a regex: descriptions of fields such as
`fare_transfer_type` contain their own value table. `<table>.*?</table>` stopped at the
inner table's closing tag and cut the outer table — `fare_transfer_rules.fare_product_id`
was dropped from the list. Therefore use a real HTML parser (stdlib `HTMLParser`) and count
only the OUTER rows of the OUTER table.
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
    """Collect the FIRST cell of only the OUTER rows of OUTER tables.

    `table_depth`: counts nested tables. A field description may contain its own
    value table (`fare_transfer_type` → 0/1/2); those rows are NOT fields and are
    skipped because they remain at depth 2.
    """

    CAPTURE_COLS = (0, 1, 2)  # Field Name · Type · Presence (Description is not captured)

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.table_depth = 0
        self.cell_index = -1        # cell position in the row (outer table)
        self.capturing = False      # are we collecting a cell's text now?
        self.buf: list[str] = []
        self.header: list[str] = [] # <th> text from the outer table
        self.in_header_cell = False
        self.row: dict[int, str] = {}
        self.rows: list[dict[int, str]] = []  # outer rows as {column: text}

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
    """(filename, start, end) — file sections from h3 anchors."""
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
    """Separate the OUTER `<table>…</table>` blocks in a section by counting depth."""
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
    """Field/Type/Presence columns from the section's tables headed "Field Name"."""
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
    """The "Primary key (…)" line at the start of a section → field list.

    `(*)` = all rows, `(none)` = no key. Preserve both as-is; interpretation is
    left to the consumer (DQ_021 already uses this distinction).
    """
    text = html.unescape(re.sub(r"<[^>]+>", " ", section))
    # The spec ITSELF is inconsistent: "Primary key" in most sections, "Primary Key" in
    # fare_leg_join_rules.txt. Case-sensitive search made that file appear to have no key.
    m = re.search(r"Primary key \(([^)]*)\)", text, re.I)
    if not m:
        return []
    raw = m.group(1).strip()
    if raw in ("*", "none"):
        return [raw]
    return [p.strip().strip("`") for p in raw.split(",") if p.strip()]


def file_presence_of(section: str) -> str:
    """The "File: …" line at the start of a section → file-level presence.

    Normative provisions in the spec appear not only in field tables but also in file headers:
    `calendar.txt` "Conditionally Required", `networks.txt` "Conditionally Forbidden", for example.
    Until 2026-08-02 this line was never extracted, so seven conditional file provisions were
    COMPLETELY outside the scope measurement.

    Değerler spec'te dört tanedir: Required · Optional · Conditionally Required ·
    Conditionally Forbidden. Tanınmayan bir değer boş dönerse ölçüm o dosyayı atlar —
    sessiz kalmaktansa `main()` içindeki sağlık kontrolü sayıyı düşürüp fark ettirir.
    """
    text = html.unescape(re.sub(r"<[^>]+>", " ", section))
    m = re.search(
        r"File:\s*(Conditionally Required|Conditionally Forbidden|Required|Optional)",
        text,
    )
    return m.group(1) if m else ""


def main() -> int:
    doc = fetch(sys.argv)
    table: dict[str, dict] = {}
    for name, start, stop in section_bounds(doc):
        section = doc[start:stop]
        fields = fields_in(section)
        if fields:
            table[name] = {
                "presence": file_presence_of(section),
                "primary_key": primary_key_of(section),
                "fields": fields,
            }
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
