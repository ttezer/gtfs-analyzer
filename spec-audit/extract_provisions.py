#!/usr/bin/env python3
"""GTFS Schedule Reference'tan NORMATİF CÜMLE kataloğunu çıkarır (T5 çapası).

Neden ayrı bir betik: `extract_fields.py` spec'in ALAN TABLOSUNU çıkarır ve bugünkü
302 hüküm atomunun kaynağı odur. Ama spec'in normatif yükü tabloda bitmiyor —
`file-requirements` bölümünün tamamı düzyazıdır ("All files must be saved as
comma-delimited text"), ve alan tablosunun Description hücreleri presence
etiketinin ifade edemediği koşulları taşır. Bu düzyazı hükümler ölçümün TAMAMEN
dışındaydı; bu betik onları sayılabilir hale getirir.

Çalıştır:
    python3 spec-audit/extract_provisions.py            # ağdan indirir
    python3 spec-audit/extract_provisions.py spec.html  # yerel kopyadan

Python'un urllib'i sertifika deposu kurulu olmayan kurulumlarda `CERTIFICATE_VERIFY_FAILED`
verir (`extract_fields.py` için de geçerli). O durumda önce indirip yerel yoldan çalıştır:
    curl -sSo spec.html https://gtfs.org/documentation/schedule/reference/

⚠️ BU BETİK HÜKÜM DEĞİL, **ADAY** ÜRETİR. Cümle bölme kabadır: bir cümle birden çok
hüküm taşıyabilir ("The files must reside at the root level directly, not in a
subfolder" iki şey söyler), ve bazı `must`'lar betimleyicidir ("Consuming software
must be able to…" gibi tüketici tarafını anlatan cümleler bizim doğrulayabileceğimiz
bir şey değildir). Triyaj insan işidir; betik yalnız paydayı görünür kılar.

⚠️ SERT/YUMUŞAK AYRIMI KATALOĞUN EN ÖNEMLİ ALANIDIR. `PTH_017` vakası (2026-08-03)
bir `should`'u norm diye dayatıyordu ve geçerli feed'i yayından alıkoyabiliyordu;
`ARC_031` vakası bunun tersiydi. Bu yüzden `strength` alanı ayrı tutulur ve
`soft` adaylar da kataloğa GİRER — görülmeyen bir tavsiye yanlış sınıflanır.

Kimlik: `id` cümlenin normalize edilmiş metninin hash'idir, sıra numarası DEĞİL.
Spec'in başka bir yerine cümle eklendiğinde mevcut adayların kimliği kaymasın diye;
triyaj defteri bu kimliklere dayanacak.
"""
import hashlib
import html
import json
import pathlib
import re
import sys
import urllib.request

SPEC_URL = "https://gtfs.org/documentation/schedule/reference/"
OUT = pathlib.Path(__file__).resolve().parent / "spec_provisions.json"

# Sert normatif işaretler. `Required`/`Forbidden` büyük harfle spec'in presence
# etiketleridir ama düzyazıda da geçerler; ayrımı triyaj yapar.
# `may not` yasak bildirir ama `may or may not` İZİN bildirir ve tam tersidir;
# geriye bakış olmadan `term-definitions`'ın "It may or may not have field values
# defined" cümlesi sert hüküm sayılıyordu.
STRONG = re.compile(
    r"\b(must not|must|shall not|shall|cannot|(?<!or )may not|is not allowed|are not allowed"
    r"|is prohibited|Conditionally Required|Conditionally Forbidden|Required|Forbidden"
    r"|is required|are required)\b"
)
# Yumuşak işaretler — hüküm DEĞİL, tavsiye. Quality sınıfına gider.
SOFT = re.compile(r"\b(should not|should|recommended|Recommended|is encouraged|preferably)\b")

# ⚠️ BÜYÜK HARF RFC 2119 ANAHTARLARI AYRI TUTULUR (2026-08-06, 764 triyajı).
# Spec'in `document-conventions` bölümü anahtar kelimeleri BÜYÜK HARFLE yazar ve
# `transfers.txt`'in sefer devamlılığı bölümü bunu bilfiil kullanır. Yukarıdaki regex'ler
# `re.I` taşımaz, bu yüzden o bölümün SEKİZ cümlesi katalogdan tamamen düşmüştü —
# üçü sert hüküm ve HİÇBİR kural tarafından ölçülmüyordu.
#
# Neden `re.I` EKLENMEDİ: `Required`/`Forbidden` spec'in presence ETİKETLERİdir ve
# büyük harfle anlamlıdır; `re.I` sıradan düzyazıdaki "required"/"forbidden"
# kelimelerini de hüküm sayardı. `Must coordinate with driver` gibi başlık-harfli enum
# etiketleri de aynı şekilde yanlış pozitif olurdu. Yalnız TAMAMI BÜYÜK biçim alınır.
UPPER_STRONG = re.compile(r"\b(MUST NOT|MUST|SHALL NOT|SHALL|REQUIRED|FORBIDDEN)\b")
UPPER_SOFT = re.compile(r"\b(SHOULD NOT|SHOULD|RECOMMENDED)\b")
# `MAY`/`OPTIONAL` bilinçli olarak DIŞARIDA: izin bildirirler, hüküm değildirler ve
# doğrulanacak bir şey söylemezler (`may not` yasağı zaten STRONG'da).

# Blok düzeyi elemanlar: her biri ayrı bir metin parçası verir. `<td>` bilinçli
# olarak burada YOK — tablo hücreleri ayrı yoldan (table_desc) toplanır.
BLOCK = re.compile(r"</(p|li|h[1-6]|blockquote|dd|dt)>", re.I)

# Cümle sonu bölücü. Kısaltmalar ve URL'ler bölmeyi bozar; en sık geçenler korunur.
ABBREV = re.compile(r"\b(e\.g|i\.e|etc|vs|cf|St|Mr|Ms|Dr|approx|no|No)\.$")

# Dosya bölümlerinin başındaki "File: Required" satırı. Bu bir hüküm AMA bu kataloğa
# ait değil: `extract_fields.py::file_presence_of()` onu zaten atom olarak çıkarıyor ve
# yedi koşullu dosya hükmü `FILE_LEVEL_PROVISIONS.md`'de elle adjudike edildi. Burada
# tutmak aynı hükmü iki deftere yazmak olur ve rozet paydasını şişirir.
FILE_HEADER = re.compile(r"^File:\s*(Required|Optional|Conditionally (Required|Forbidden))\.?$")


def fetch(argv: list[str]) -> str:
    if len(argv) > 1:
        return pathlib.Path(argv[1]).read_text(encoding="utf-8")
    with urllib.request.urlopen(SPEC_URL, timeout=60) as r:
        return r.read().decode("utf-8")


def strip_tags(fragment: str) -> str:
    text = html.unescape(re.sub(r"<[^>]+>", " ", fragment))
    return re.sub(r"\s+", " ", text).strip()


def outer_tables(section: str) -> list[tuple[int, int]]:
    """Bölümdeki DIŞ `<table>` bloklarının (başlangıç, bitiş) aralıkları.

    İç içe tablolar var (`fare_transfer_type`'ın değer tablosu gibi), bu yüzden
    derinlik sayılır — `extract_fields.py` ile aynı gerekçe.
    """
    out: list[tuple[int, int]] = []
    depth, start = 0, None
    for m in re.finditer(r"</?table\b[^>]*>", section):
        if m.group(0).startswith("</"):
            depth -= 1
            if depth == 0 and start is not None:
                out.append((start, m.end()))
                start = None
        else:
            if depth == 0:
                start = m.start()
            depth += 1
    return out


def prose_of(section: str) -> str:
    """Bölümün tablo DIŞI kalan HTML'i."""
    spans = outer_tables(section)
    if not spans:
        return section
    parts, cursor = [], 0
    for start, stop in spans:
        parts.append(section[cursor:start])
        cursor = stop
    parts.append(section[cursor:])
    return "".join(parts)


def description_cells(section: str) -> list[tuple[str, str]]:
    """Alan tablolarının (alan adı, Description hücresi) çiftleri.

    Presence sütunu bilinçli olarak alınmaz: o bir ETİKET, cümle değil, ve zaten
    `extract_fields.py` tarafından atom olarak sayılıyor. Description ise presence
    etiketinin ifade edemediği koşulu taşır ("Required if trips.txt includes …").

    ⚠️ ALAN ADI ŞART. Açıklama cümleleri alansız okunduğunda triyaj edilemez:
    `stop_times.txt`'in "Required for timepoint=1" cümlesi hem `arrival_time` hem
    `departure_time` için ayrı ayrı geçer ve ikisinin karşılığı FARKLI kurallardır
    (`STM_015/016` vs `STM_034/047`). İlk turda alan adı çıkarılmıyordu ve 36
    stop_times adayının hiçbiri bu hâliyle adjudike edilemedi.
    """
    cells: list[tuple[str, str]] = []
    for start, stop in outer_tables(section):
        table = section[start:stop]
        head = strip_tags(table[: table.find("</tr>") + 5]) if "</tr>" in table else ""
        if "field name" not in head.lower():
            continue
        # Satır satır gez: 1. hücre alan adı, 4. hücre açıklama. İç tabloların
        # hücreleri bu regex'e de takılır ama Description'ın PARÇASI oldukları için
        # kayıp değil, gürültü.
        for row in re.findall(r"<tr\b[^>]*>(.*?)</tr>", table, re.S | re.I):
            tds = re.findall(r"<td\b[^>]*>(.*?)</td>", row, re.S | re.I)
            if len(tds) >= 4:
                name = strip_tags(tds[0]).strip("`").strip()
                if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_.]*", name):
                    name = ""
                cells.append((name, tds[3]))
    return cells


def sentences(text: str) -> list[str]:
    """Kaba cümle bölme. Kısaltmalarda ve URL'lerde bölmemeye çalışır."""
    out: list[str] = []
    buf = ""
    for chunk in re.split(r"(?<=[.!?])\s+", text):
        buf = f"{buf} {chunk}".strip() if buf else chunk
        if ABBREV.search(buf) or buf.endswith(("http:", "https:")) or re.search(r"\(\w\.\w\.$", buf):
            continue
        out.append(buf)
        buf = ""
    if buf:
        out.append(buf)
    return [s for s in (x.strip() for x in out) if len(s) > 12]


def blocks(fragment: str) -> list[str]:
    """Blok elemanlara göre böl, sonra her bloğu cümlelere ayır."""
    out: list[str] = []
    for piece in BLOCK.split(fragment):
        if piece.lower() in ("p", "li", "blockquote", "dd", "dt") or re.fullmatch(r"h[1-6]", piece, re.I):
            continue
        text = strip_tags(piece)
        if text:
            out.extend(sentences(text))
    return out


def section_bounds(doc: str) -> list[tuple[str, str, int, int]]:
    """(bölüm_adı, düzey, başlangıç, bitiş) — h2 ve h3 çapaları birlikte.

    h3'ler h2'nin içindedir; ikisini tek listede sıralayıp bir sonraki BAŞLIĞA kadar
    kesmek, her metin parçasının EN YAKIN başlığa ait olmasını sağlar. Yoksa
    `field-definitions` h2'si 100 KB'lık gövdenin tamamını yutardı.
    """
    heads = [
        (m.group(2), m.group(1), m.start(), m.end())
        for m in re.finditer(r'<h([23])[^>]*id="([^"]+)"[^>]*>', doc)
    ]
    out = []
    for i, (anchor, level, start, end) in enumerate(heads):
        stop = heads[i + 1][2] if i + 1 < len(heads) else len(doc)
        out.append((anchor, f"h{level}", end, stop))
    return out


def classify(sentence: str) -> str:
    if STRONG.search(sentence) or UPPER_STRONG.search(sentence):
        return "strong"
    if SOFT.search(sentence) or UPPER_SOFT.search(sentence):
        return "soft"
    return ""


def ident(section: str, sentence: str) -> str:
    key = f"{section}|{re.sub(r'[^a-z0-9]+', '', sentence.lower())}"
    return "P" + hashlib.sha1(key.encode("utf-8")).hexdigest()[:8]


def main() -> int:
    doc = fetch(sys.argv)
    rows: list[dict] = []
    seen: set[str] = set()
    for anchor, level, start, stop in section_bounds(doc):
        section = doc[start:stop]
        items: list[tuple[str, str, str]] = [
            ("prose", "", s) for s in blocks(prose_of(section))
        ]
        items += [
            ("table_desc", name, s)
            for name, cell in description_cells(section)
            for s in sentences(strip_tags(cell))
        ]
        for source, field, sentence in items:
            if FILE_HEADER.match(sentence):
                continue
            strength = classify(sentence)
            if not strength:
                continue
            # Kimlik alanı da kapsar: aynı cümle (ör. "Required for timepoint=1")
            # iki ayrı alanın açıklamasında geçer ve İKİ AYRI hükümdür.
            #
            # ⚠️ ALANSIZ SATIRLARDA AYRAÇ EKLENMEZ. Kimlik şeması bir sözleşmedir:
            # triyaj defteri bu kimliklere işaret eder. Alan adı eklendiğinde ayracı
            # koşulsuz koymak düzyazı satırlarının kimliğini de kaydırdı ve ilk turun
            # 27 kaydının HEPSİ çözümlenemez oldu. Boş alan = eski anahtar.
            pid = ident(f"{anchor}|{field}" if field else anchor, sentence)
            if pid in seen:
                continue
            seen.add(pid)
            rows.append({
                "id": pid,
                "section": anchor,
                "field": field,
                "level": level,
                "source": source,
                "strength": strength,
                "sentence": sentence,
            })
    if len(rows) < 150:
        print(f"HATA: yalnız {len(rows)} aday bulundu; spec düzeni değişmiş olabilir.",
              file=sys.stderr)
        return 1
    OUT.write_text(
        json.dumps(
            {
                "_source": SPEC_URL,
                "_note": "Üretilmiş dosya — elle düzenlemeyin. Yeniden üretmek için: "
                         "python3 spec-audit/extract_provisions.py. Satırlar ADAYDIR, "
                         "hüküm değil; triyaj spec-audit/PROVISION_TRIAGE.md'de.",
                "provisions": rows,
            },
            ensure_ascii=False,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    strong = sum(1 for r in rows if r["strength"] == "strong")
    prose = sum(1 for r in rows if r["source"] == "prose")
    print(f"yazıldı: {OUT.name} ({len(rows)} aday · sert {strong} · yumuşak {len(rows) - strong})")
    print(f"  kaynak: düzyazı {prose} · tablo açıklaması {len(rows) - prose}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
