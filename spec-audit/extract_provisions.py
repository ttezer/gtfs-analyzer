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
import re as _re
import json
import os
import pathlib
import re
import subprocess
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
# 🔴 MODAL TAŞIMAYAN ama NORMATİF olan kalıplar (issue #81).
# Spec bazı hükümleri modal olmadan kurar: *"All file and field names are case-sensitive."*
# Bu cümlede `must`/`shall` yoktur, dolayısıyla modal-güdümlü tarama onu HİÇ görmüyordu ve
# katalogda yer almadığı için ne adjudike edilebiliyor ne de öksüz sayılabiliyordu —
# yani "147/147" bilinen sert düzyazı hükümlerinin TAMAMI hakkında bir cümle değildi.
#
# ⚠️ Bu küme `measure_modalless.py::SIGNALS` ile AYNI olguyu tanımlar ve ikisi ayrışırsa
# aynı boşluk geri gelir. `spec-audit/signal_parity.py` bunu CI'da denetler.
MODALLESS_STRONG = re.compile(r"\bcase[- ]sensitive\b", re.I)

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

# ⚠️ ÜÇÜNCÜ CASE KAÇAĞI (2026-08-06, 506 kalıntının tek tek okunması).
# Yukarıdaki iki regex de cümle ORTASINDAKİ küçük harfli modalı ve TAMAMI BÜYÜK biçimi
# arıyordu. Spec ise alan açıklamalarını sık sık modalla BAŞLATIYOR — özne bir önceki
# cümlededir: *"Must be unique in areas.txt."* · *"Should not be a duplicate of stop_name."*
# Ayrıca yasağı KÜÇÜK harfle de yazıyor (*"Values greater than 24:00:00 are forbidden"*)
# ve bazen fiil çekimiyle (*"…requires two records…"*). Ölçüldü: **20 cümle** böyle kaçmıştı.
#
# Neden hâlâ `re.I` YOK: `Required`/`Forbidden` presence ETİKETLERİdir; `re.I` sıradan
# düzyazıdaki "required"/"forbidden" kelimelerini de hüküm sayardı. Kaçağın biçimi
# dar ve ölçülmüş olduğu için desen de dar tutuldu.
#
# `^(Must|Shall)` enum etiketiyle çakışmaz: enum satırları "3 - Must coordinate…" biçiminde
# başlar, çıplak `Must` ile değil (8 eşleşmenin 8'i de gerçek hüküm çıktı).
LEAD_STRONG = re.compile(r"^(Must|Shall)\b|\b(is|are)\s+forbidden\b|\brequires\b")
LEAD_SOFT = re.compile(r"^Should\b")

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


# Spec'in ÜRETİLDİĞİ kaynak: gtfs.org bu markdown'ı render eder.
UPSTREAM_REPO = "google/transit"
UPSTREAM_PATH = "gtfs/spec/en/reference.md"
UPSTREAM_API = (
    f"https://api.github.com/repos/{UPSTREAM_REPO}/commits"
    f"?path={UPSTREAM_PATH}&per_page=1"
)


def upstream_commit() -> dict:
    """`reference.md`'ye dokunan EN YENİ upstream commit'i çözer (issue #72).

    ⚠️ Bu, `sha256`'nın YERİNE GEÇMEZ — ikisi FARKLI şeyleri sabitler:
      · `sha256`          → bizim AYRIŞTIRDIĞIMIZ baytlar (render edilmiş HTML)
      · `upstream_commit` → o metnin KAYNAK sürümü (markdown, google/transit)
    Katalog markdown'dan değil HTML'den üretilir; commit "hangi spec sürümü" sorusunu
    yanıtlar, "hangi baytlar" sorusunu DEĞİL. İkisi de kayıtta durur, biri diğerini
    gereksiz kılmaz.

    ⚠️ Çözülemezse SESSİZCE null yazılmaz — `main()` hata verip durur. Kayıtta boş
    duran bir alan, zamanla "hiç doldurulmamış" ile "doldurulamadı"yı ayırt edilemez
    kılar (`AGN_001` dersi). Ağsız üretim için `--no-upstream` bayrağı vardır ve o
    zaman kayıt bunu AÇIKÇA yazar.
    """
    # ⚠️ `urllib` DEĞİL `curl` — bu betiğin kendi docstring'i ve `zip_peek.py` aynı tuzağı
    # not ediyor: geliştirme makinesinde Python'un sertifika deposu yok, `urlopen`
    # CERTIFICATE_VERIFY_FAILED veriyor. `curl` her iki ortamda da çalışıyor.
    cmd = ["curl", "-sSL", "--max-time", "60", "-H", "Accept: application/vnd.github+json",
           "-H", "User-Agent: gtfs-analyzer"]
    # ⚠️ CI'da KİMLİKLİ istek şart: kimliksiz kota IP başına saatte 60'tır ve GitHub
    # koşucuları o IP'yi paylaşır — 2026-08-07'de ilk koşum tam bu yüzden düştü.
    # Yerelde token yoksa kimliksiz devam eder (kota bir kişi için yeter).
    if token := (os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")):
        cmd += ["-H", f"Authorization: Bearer {token}"]
    p = subprocess.run(cmd + [UPSTREAM_API], capture_output=True)
    if p.returncode != 0:
        raise RuntimeError(f"curl hatası: {p.stderr.decode()[:120]}")
    data = json.loads(p.stdout.decode("utf-8"))
    if isinstance(data, dict):  # API hata gövdesi (rate limit vb.) da JSON'dur
        raise RuntimeError(f"API beklenmeyen yanıt: {str(data)[:120]}")
    if not data:
        raise RuntimeError(f"{UPSTREAM_PATH} için commit dönmedi")
    return {
        "upstream_repo": UPSTREAM_REPO,
        "upstream_path": UPSTREAM_PATH,
        "upstream_commit": data[0]["sha"],
        "upstream_commit_date": data[0]["commit"]["committer"]["date"],
    }


def provisions_sha256(rows: list[dict]) -> str:
    """Kataloğun KENDİSİNİN parmak izi — sayfanın değil.

    🔴 2026-08-07'de ÖLÇÜLDÜ: `sha256` (render edilmiş HTML'in özeti) YENİDEN ÜRETİLEMEZ.
    Sayfa arka arkaya üç kez indirildi, üç FARKLI özet çıktı; fark yalnız Cloudflare'in
    enjekte ettiği iki satırdı (`data-cfemail` token'ı + `__CF$cv$params` ray id).
    Yani "hash tutmuyorsa katalog başka bir metinden üretilmiştir" iddiası YANLIŞTI:
    hash HİÇBİR ZAMAN tutmaz, saniyeler sonra bile.

    Bu alan o iddianın gerçekten dayanabileceği yerdir: adaylar üzerinden hesaplanır,
    site iskeletinden bağımsızdır ve üçüncü bir taraf aynı sayfayı indirip AYNI değeri
    elde eder. Sürüklenme tespiti (`spec_drift.py`) ve issue tekilleştirmesi buna dayanır.
    """
    canon = json.dumps(rows, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(canon.encode("utf-8")).hexdigest()


def source_fingerprint(doc: str, origin: str, upstream: dict, rows: list[dict]) -> dict:
    """Kataloğun hangi KAYNAK METİNDEN üretildiğini kanıtlanabilir kılar.

    ⚠️ 2026-08-06 dış denetimi: JSON yalnız `_source` URL'i taşıyordu. gtfs.org CANLI bir
    sayfa; yarın değişirse bugünkü 299 adayın aynı metinden üretildiği KANITLANAMAZDI.
    Artık indirilen HTML'in SHA-256'sı ve spec'in kendi "Revised …" satırı da yazılır:
    ikisi birden tutuyorsa katalog bit-bit yeniden üretilebilir.

    ⚠️ 2026-08-07 (issue #72): buna upstream commit eklendi — bkz. `upstream_commit()`.
    """
    m = _re.search(r"Revised\s+([A-Z][a-z]+\s+\d{1,2},\s+\d{4})", strip_tags(doc))
    return {
        "url": origin,
        # ⚠️ Bu özet YENİDEN ÜRETİLEMEZ (Cloudflare her yanıtta iki satırı değiştirir);
        # "bu baytları gördük" kaydıdır, doğrulanabilir bir çapa DEĞİL. Çapa
        # `provisions_sha256`.
        "sha256": hashlib.sha256(doc.encode("utf-8")).hexdigest(),
        "bytes": len(doc.encode("utf-8")),
        "provisions_sha256": provisions_sha256(rows),
        "spec_revision": m.group(1) if m else None,
        **upstream,
    }


def fetch(argv: list[str]) -> str:
    paths = [a for a in argv[1:] if not a.startswith("--")]
    if paths:
        return pathlib.Path(paths[0]).read_text(encoding="utf-8")
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
    if (STRONG.search(sentence) or UPPER_STRONG.search(sentence)
            or LEAD_STRONG.search(sentence) or MODALLESS_STRONG.search(sentence)):
        return "strong"
    if SOFT.search(sentence) or UPPER_SOFT.search(sentence) or LEAD_SOFT.search(sentence):
        return "soft"
    return ""


class ProvisionCollision(Exception):
    """İki FARKLI kanonik anahtar aynı kimliğe düştü — katalog üretimi DURUR."""


def canonical_key(section: str, sentence: str) -> str:
    """Kimliğin türetildiği kanonik anahtar. Kimlik bunun hash'idir; karşılaştırma
    HER ZAMAN bu anahtar üzerinden yapılır, kısaltılmış hash üzerinden değil."""
    return f"{section}|{re.sub(r'[^a-z0-9]+', '', sentence.lower())}"


def ident(section: str, sentence: str) -> str:
    return "P" + hashlib.sha1(canonical_key(section, sentence).encode("utf-8")).hexdigest()[:8]


def claim_id(seen: dict[str, str], pid: str, key: str) -> bool:
    """Kimliği kaydet. `True` → yeni hüküm, `False` → AYNI hükmün tekrarı (atlanır).

    🔴 ÇAKIŞMA FAIL-OPEN İDİ (issue #83). Eski kod `if pid in seen: continue` diyordu:
    kimlik 32 bitlik kısaltılmış SHA-1 olduğu için iki FARKLI cümle aynı kimliğe düşerse
    ikincisi katalogdan SESSİZCE düşerdi — ve paydayı üreten mekanizma budur. Adjudike
    edilmemiş hüküm kapısı bunu göremez: katalogda hiç görünmeyen bir hükmün öksüz
    olduğu da anlaşılamaz. Yüzde eksik payda üzerinden "%100" demeye devam ederdi.

    ⚠️ Kimlik 8 hex olarak KALIYOR (genişletmek defterdeki 299 kararın hepsini
    geçersiz kılardı — o bir MİGRASYON kararıdır, bir hata düzeltmesi değil). Bunun
    yerine çakışma AÇIKÇA denetlenir ve üretim fail-CLOSED durur.
    """
    prev = seen.get(pid)
    if prev is None:
        seen[pid] = key
        return True
    if prev == key:
        return False  # aynı cümle iki kez tarandı — normal
    raise ProvisionCollision(
        f"kimlik çakışması {pid}: iki FARKLI kanonik anahtar aynı kimliğe düştü.\n"
        f"  1) {prev[:120]}\n  2) {key[:120]}\n"
        f"  → Kimlik şeması bir SÖZLEŞMEDİR (triyaj defteri bu kimliklere işaret eder). "
        f"Çözüm bir MİGRASYONDUR: hash genişletilir ve PROVISION_TRIAGE.md aynı commit'te "
        f"güncellenir. Sessizce düşürmek paydayı bozar."
    )


def _selftest() -> int:
    """Çakışma yolunu ZORLAR ve fail-closed olduğunu kanıtlar (issue #83).

    Ağ istemez, CI'da koşar. `claim_id`'i doğrudan çağırır: iki farklı anahtar aynı
    kimliğe elle verilir. Kapıyı yazmak kurmak değildir — bozup görmek gerekir.
    """
    seen: dict[str, str] = {}
    assert claim_id(seen, "Pdeadbeef", "a|birinci cumle"), "ilk kayıt yeni olmalı"
    assert not claim_id(seen, "Pdeadbeef", "a|birinci cumle"), "aynı anahtar tekrarı atlanmalı"
    try:
        claim_id(seen, "Pdeadbeef", "a|ikinci FARKLI cumle")
    except ProvisionCollision as e:
        print(f"selftest OK — çakışma yakalandı:\n  {str(e).splitlines()[0]}")
        return 0
    print("selftest BAŞARISIZ: çakışma sessizce geçti — katalog fail-open.", file=sys.stderr)
    return 1


def main() -> int:
    if "--selftest" in sys.argv:
        return _selftest()
    doc = fetch(sys.argv)
    # issue #72: kaynak sürüm pini. Çözülemezse SESSİZ GEÇMEZ.
    if "--no-upstream" in sys.argv:
        upstream = {
            "upstream_repo": UPSTREAM_REPO,
            "upstream_path": UPSTREAM_PATH,
            "upstream_commit": None,
            "upstream_commit_date": None,
            "upstream_commit_note": "--no-upstream ile üretildi: commit ÇÖZÜLMEDİ, "
                                    "'çözülemedi' ile 'hiç denenmedi' karışmasın diye "
                                    "burada açıkça yazılıdır.",
        }
        print("UYARI: --no-upstream — katalog kaynak sürümüne PİNLENMEDİ.", file=sys.stderr)
    else:
        try:
            upstream = upstream_commit()
        except Exception as e:  # ağ/API hatası
            print(f"HATA: upstream commit çözülemedi ({type(e).__name__}: {e}). "
                  f"Ağsız üretmek için: --no-upstream", file=sys.stderr)
            return 1
    rows: list[dict] = []
    seen: dict[str, str] = {}  # pid → kanonik anahtar (çakışma denetimi için)
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
            section_key = f"{anchor}|{field}" if field else anchor
            pid = ident(section_key, sentence)
            if not claim_id(seen, pid, canonical_key(section_key, sentence)):
                continue
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
                "_source": source_fingerprint(doc, SPEC_URL, upstream, rows),
                "_note": "Üretilmiş dosya — elle düzenlemeyin. Yeniden üretmek için: "
                         "python3 spec-audit/extract_provisions.py. Satırlar ADAYDIR, "
                         "hüküm değil; triyaj spec-audit/PROVISION_TRIAGE.md'de. "
                         "`_source.provisions_sha256` kataloğun DOĞRULANABİLİR çapasıdır; tutmuyorsa "
                         "yüzde başka bir metne aittir. (`sha256` yeniden ÜRETİLEMEZ — sayfa her "
                         "yanıtta Cloudflare satırları yüzünden değişir; o alan yalnız kayıttır.) "
                         "`_source.upstream_commit` ise o metnin KAYNAK sürümünü (google/transit "
                         "reference.md) sabitler — sha256'nın yerine geçmez, farklı soruyu "
                         "yanıtlar: 'hangi spec sürümü' vs 'hangi baytlar'.",
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
    try:
        raise SystemExit(main())
    except ProvisionCollision as exc:
        # Traceback yerine TEŞHİS bas: okuyanın ne yapacağı mesajın içinde yazıyor.
        print(f"HATA: {exc}", file=sys.stderr)
        raise SystemExit(1) from None
