//! ÜRETİLMİŞ DOSYA — elle düzenlemeyin (issue #82).
//!
//! Kaynak    : https://www.iana.org/assignments/language-subtag-registry/language-subtag-registry
//! File-Date : 2026-06-14   (kayıt defterinin kendi tarihi)
//! SHA-256   : be1fad86a99e3a932d07b80c9b3c271ec2381a5909ce22420144e5077ab0a43a
//! Üretim    : `python3 spec-audit/gen_bcp47_grandfathered.py`
//! Drift     : `python3 spec-audit/gen_bcp47_grandfathered.py --check`
//!
//! RFC 5646 `grandfathered` etiketleri (26 adet). Bunlar normal `langtag`
//! üretimine uymayan ama GEÇERLİ olan sabit kayıtlardır.
//!
//! ⚠️ `i-` bir AD ALANI DEĞİLDİR: yalnız bu listedeki `i-*` etiketleri geçerlidir.
//! Karşılaştırma küçük harfe normalize edilerek yapılır (RFC 5646 §2.1.1: etiketler
//! büyük/küçük harfe duyarsızdır).

/// Alfabetik SIRALI, küçük harf — `binary_search` bunu varsayar.
pub(crate) const GRANDFATHERED: &[&str] = &[
    "art-lojban",
    "cel-gaulish",
    "en-gb-oed",
    "i-ami",
    "i-bnn",
    "i-default",
    "i-enochian",
    "i-hak",
    "i-klingon",
    "i-lux",
    "i-mingo",
    "i-navajo",
    "i-pwn",
    "i-tao",
    "i-tay",
    "i-tsu",
    "no-bok",
    "no-nyn",
    "sgn-be-fr",
    "sgn-be-nl",
    "sgn-ch-de",
    "zh-guoyu",
    "zh-hakka",
    "zh-min",
    "zh-min-nan",
    "zh-xiang",
];
