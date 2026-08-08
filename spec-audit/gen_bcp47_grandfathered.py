#!/usr/bin/env python3
"""RFC 5646 grandfathered etiketlerini IANA kayıt defterinden üret (issue #82).

🔴 Neden: `looks_like_bcp47` `i-` önekini AÇIK bir ad alanı gibi ele alıyordu —
`i-` + herhangi bir harf dizisi geçiyordu, yani `i-foo` ve `i-whatever` well-formed
sayılıyordu. RFC 5646'da `i-*` biçimleri **sabit bir kayıt defterinden** gelir
(`i-klingon`, `i-navajo`, `i-default`…); uydurma bir `i-foo` ne grandfathered bir etikettir
ne de normal bir `langtag`.

⚠️ Bu, kartta yazılı `zz-ZZ` kalıntısından FARKLI bir şeydir: `zz-ZZ` sözdizimsel olarak
well-formed ama kayıtlı değildir (bilinçli kabul). `i-foo` ise **sözdizimsel olarak
geçersizdir** — yani kalıntı değil, hataydı.

Kaynak: IANA Language Subtag Registry (RFC 5646 §3). `Type: grandfathered` kayıtları.

Çalıştır:
    python3 spec-audit/gen_bcp47_grandfathered.py            # ağdan üret
    python3 spec-audit/gen_bcp47_grandfathered.py --check    # denetle (1 = drift, 2 = bakılamadı)
    python3 spec-audit/gen_bcp47_grandfathered.py lsr.txt
"""
import hashlib
import pathlib
import re
import subprocess
import sys

SRC = "https://www.iana.org/assignments/language-subtag-registry/language-subtag-registry"
OUT = pathlib.Path(__file__).resolve().parent.parent / "crates/pipeline/src/k2/bcp47_grandfathered.rs"


def fetch(argv: list[str]) -> bytes:
    local = [a for a in argv[1:] if not a.startswith("--")]
    if local:
        return pathlib.Path(local[0]).read_bytes()
    p = subprocess.run(["curl", "-sSL", "--max-time", "90", SRC], capture_output=True)
    if p.returncode != 0:
        raise RuntimeError(f"curl hatası: {p.stderr.decode()[:120]}")
    return p.stdout


def parse(raw: bytes) -> tuple[str, list[str]]:
    text = raw.decode("utf-8", "replace")
    m = re.search(r"^File-Date:\s*(\S+)", text, re.M)
    file_date = m.group(1) if m else "?"
    tags: list[str] = []
    for record in text.split("\n%%\n"):
        if re.search(r"^Type:\s*grandfathered\s*$", record, re.M):
            t = re.search(r"^Tag:\s*(\S+)", record, re.M)
            if t:
                # ⚠️ Kayıt defteri etiketleri karışık yazımla tutar (`en-GB-oed`,
                # `zh-min-nan`); karşılaştırma RFC 5646 §2.1.1'e göre BÜYÜK/küçük harfe
                # DUYARSIZDIR, o yüzden tablo küçük harfe normalize edilir.
                tags.append(t.group(1).lower())
    if not tags:
        raise RuntimeError("kayıt defterinde grandfathered kaydı bulunamadı")
    return file_date, sorted(set(tags))


def render(file_date: str, digest: str, tags: list[str]) -> str:
    body = "\n".join(f'    "{t}",' for t in tags)
    return f'''//! ÜRETİLMİŞ DOSYA — elle düzenlemeyin (issue #82).
//!
//! Kaynak    : {SRC}
//! File-Date : {file_date}   (kayıt defterinin kendi tarihi)
//! SHA-256   : {digest}
//! Üretim    : `python3 spec-audit/gen_bcp47_grandfathered.py`
//! Drift     : `python3 spec-audit/gen_bcp47_grandfathered.py --check`
//!
//! RFC 5646 `grandfathered` etiketleri ({len(tags)} adet). Bunlar normal `langtag`
//! üretimine uymayan ama GEÇERLİ olan sabit kayıtlardır.
//!
//! ⚠️ `i-` bir AD ALANI DEĞİLDİR: yalnız bu listedeki `i-*` etiketleri geçerlidir.
//! Karşılaştırma küçük harfe normalize edilerek yapılır (RFC 5646 §2.1.1: etiketler
//! büyük/küçük harfe duyarsızdır).

/// Alfabetik SIRALI, küçük harf — `binary_search` bunu varsayar.
pub(crate) const GRANDFATHERED: &[&str] = &[
{body}
];
'''


def main() -> int:
    check = "--check" in sys.argv
    try:
        raw = fetch(sys.argv)
        file_date, tags = parse(raw)
    except Exception as e:
        print(f"ÖLÇÜLEMEDİ: {type(e).__name__}: {e}", file=sys.stderr)
        print("⚠️ Bu 'drift yok' DEĞİLDİR — kaynağa bakılamadı.", file=sys.stderr)
        return 2
    text = render(file_date, hashlib.sha256(raw).hexdigest(), tags)
    if check:
        cur = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
        cur_tags = set(re.findall(r'^    "([a-z0-9-]+)",$', cur, re.M))
        if cur_tags == set(tags):
            print(f"bcp47 grandfathered OK — {len(tags)} etiket (kayıt defteri {file_date})")
            return 0
        added, gone = sorted(set(tags) - cur_tags), sorted(cur_tags - set(tags))
        print(f"🔴 GRANDFATHERED DRIFT — eklenen {added[:6]} · kaybolan {gone[:6]}")
        print("  → python3 spec-audit/gen_bcp47_grandfathered.py")
        return 1
    OUT.write_text(text, encoding="utf-8")
    print(f"yazıldı: {OUT.name} ({len(tags)} etiket · kayıt defteri {file_date})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
