//! Kural KİMLİĞİ denetimi — "bir rule_id kaç ayrı OLGU için emit ediliyor?"
//!
//! `emit_coverage.rs` "kural kodda geçiyor mu" (ölü kural) der.
//! `emit_proof.rs` "kural ateşleyebiliyor mu" der.
//! `spec_conformance.rs` "geçerli veri sessiz mi" der.
//! Hiçbiri şunu sormuyordu: **bir rule_id, registry'nin anlattığından BAŞKA bir olgu için de
//! kullanılıyor mu?**
//!
//! Bu kör noktayı ATR_006/ATR_007 gösterdi (issue #62): ATR_006 hem `is_authority` enum'unu
//! (k2) hem `attributions.route_id` çapraz referansını (k4) emit ediyor; registry başlığı
//! yalnız birincisini anlatıyor. Bozuk bir `route_id` taşıyan feed, başlığı "is_authority
//! geçersiz" olan ama çözümü "Geçerli bir route_id kullanın" diyen bir bulgu alıyor. Var olan
//! bir rule_id'ye ikinci emit noktası eklemek mevcut kapıların HİÇBİRİNE görünmüyordu.
//!
//! ## Ölçüt: AYRIK alan kümeleri
//! Bir kuralın birden çok emit noktası olması normaldir (eksik/geçersiz dalları, aynı olgunun
//! birden çok dosyada ölçülmesi). Ayırt edici olan, noktaların **hiç ortak alanı olmaması**:
//!   `CAL_003` → `[start_date]` ve `[end_date, start_date]`  → kesişiyor, aynı olgu, SESSİZ
//!   `ATR_006` → `[is_authority]` ve `[route_id]`            → AYRIK, iki farklı olgu, RAPOR
//! Ölçüldü: ayrıklık filtresi olmadan 32 satır (çoğu pencere taşması), filtreyle 8.
//!
//! ## Denenip ELENEN ikinci kontrol
//! "Registry başlığı bir alan adı içeriyorsa emit noktasında o ad geçmeli" de denendi:
//! 47 uyarı üretti, neredeyse hepsi gürültü (başlıktaki `feed_info.txt` dosya adından gelen
//! `feed_info` token'ı; `agency_cemv_support` başlığına karşı `cemv_support` alanı; pencere
//! taşması). Gerçekten yakaladığı ATR_006/ATR_007'yi bu kontrol zaten yakalıyor → ikinci bir
//! gürültü kaynağı eklemenin karşılığı YOK, eklenmedi.
//!
//! KAPI DEĞİL, DEFTER: meşru örnekler var (DQ_018 yedi metin alanında aynı olguyu ölçer).
//! Defter yeni satırı reddeder; her yeni satır adjudike edilmelidir.
//!
//! ## Defterin ADJUDİKASYONU (2026-08-01) — gerekçeler burada, çünkü defter yeniden üretilir
//! | Kural | Karar |
//! |---|---|
//! | ~~ATR_006~~ | ✅ **ÇÖZÜLDÜ (issue #62)** — `route_id` FK'sı `ATR_011`'e ayrıldı; ATR_006 artık yalnız `is_authority` enum'unu ölçer. Defterden düştü. |
//! | ~~ATR_007~~ | ✅ **ÇÖZÜLDÜ (issue #62)** — `trip_id` FK'sı `ATR_012`'ye ayrıldı; ATR_007 artık yalnız `attribution_url`'i ölçer. Defterden düştü. |
//! | **STP_003** | ⚠️ Aynı biçim ama **başlıkta AÇIKÇA beyan edilmiş**: *"stop_name eksik veya stop_lat/stop_lon aralık dışı (aynı ID altında iki ayrı koşul)"*. Kullanıcı yanlış etiketlenmiş bulgu almıyor. Bölünmesi ayrı bir karar; #62'de not düşüldü. |
//! | AGN_013 | ✅ Tek olgu, iki alan: *"Feed dili ve ajans dili uyuşmuyor"* (`feed_lang` ↔ `agency_lang`) |
//! | BKR_001 | ✅ Tek olgu: `prior_notice_start_day` / `prior_notice_last_day` aynı yasak bağlamı |
//! | DQ_018 · DQ_019 | ✅ Tek olgu (all-caps / kontrol karakteri), yedi metin alanında ölçülür |
//! | OPR_010 | ✅ Başlık ikisini de adlandırır: *"erişilebilirlik VEYA bisiklet politikası çelişiyor"* |
//! | SHP_009 · SHP_020 | ✅ **Sezgisel gürültü:** yakalanan literaller alan değil, `details` haritası anahtarları (`affected_shapes`, `repeated_shapes`) |
//! | TRF_017 | ✅ Tek olgu, aktarmanın iki ucu: `from_route_id` / `to_route_id` |
//! | XFL_021 | ✅ Başlık zaten çifti adlandırır: *"(from_trip_id/to_trip_id, stop_id) çifti"* |

use gtfs_rules::RULES;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

/// `#[cfg(test)]` bloklarını brace-eşleştirerek çıkarır (emit_coverage.rs ile aynı yöntem).
fn strip_test_mods(src: &str) -> String {
    let marker = "#[cfg(test)]";
    let (mut out, mut rest) = (String::new(), src);
    while let Some(pos) = rest.find(marker) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos..];
        let Some(brace) = after.find('{') else { rest = ""; break };
        let bytes = after.as_bytes();
        let (mut depth, mut k) = (0i32, brace);
        while k < bytes.len() {
            match bytes[k] {
                b'{' => depth += 1,
                b'}' => { depth -= 1; if depth == 0 { k += 1; break; } }
                _ => {}
            }
            k += 1;
        }
        rest = &after[k..];
    }
    out.push_str(rest);
    out
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() { rs_files(&p, out); }
        else if p.extension().and_then(|x| x.to_str()) == Some("rs") { out.push(p); }
    }
}

/// Bir string literal GTFS alan adı gibi mi görünüyor? (snake_case, dosya adı/cümle değil)
fn looks_like_field(s: &str) -> bool {
    s.contains('_')
        && !s.contains('.')
        && !s.contains(' ')
        && s.len() <= 40
        && s.bytes().all(|b| b.is_ascii_lowercase() || b == b'_' || b.is_ascii_digit())
}

/// Pencere: rule_id literalinden sonraki satır sayısı. Emit çağrılarının argüman listesi
/// bundan kısadır; başka bir rule_id literali görülünce erken kesilir.
const WINDOW: usize = 14;

/// rule_id → her emit noktasında görülen alan-benzeri literaller.
fn emit_field_sets() -> BTreeMap<String, Vec<(String, BTreeSet<String>)>> {
    let mut files = Vec::new();
    rs_files(&repo_root().join("crates/pipeline/src"), &mut files);
    let known: BTreeSet<&str> = RULES.iter().map(|r| r.id).collect();
    let mut out: BTreeMap<String, Vec<(String, BTreeSet<String>)>> = BTreeMap::new();

    for f in &files {
        // k7 rapor görünüm mantığı emit kaynağı değil (emit_coverage.rs ile aynı istisna).
        if f.file_name().and_then(|n| n.to_str()) == Some("k7_reporting.rs") { continue; }
        let text = strip_test_mods(&fs::read_to_string(f).unwrap());
        let lines: Vec<&str> = text.lines().collect();
        let fname = f.file_name().unwrap().to_string_lossy().to_string();

        for (i, line) in lines.iter().enumerate() {
            for id in known.iter() {
                if !line.contains(&format!("\"{id}\"")) { continue; }
                let mut fields = BTreeSet::new();
                for l in lines.iter().take((i + WINDOW).min(lines.len())).skip(i) {
                    if *l != *line
                        && known.iter().any(|o| o != id && l.contains(&format!("\"{o}\"")))
                    {
                        break;
                    }
                    for lit in l.split('"').skip(1).step_by(2) {
                        if lit != *id && looks_like_field(lit) { fields.insert(lit.to_string()); }
                    }
                }
                if !fields.is_empty() {
                    out.entry((*id).to_string()).or_default().push((fname.clone(), fields));
                }
            }
        }
    }
    out
}

#[test]
fn rule_ids_emitted_for_disjoint_fields_match_ledger() {
    let mut found: Vec<String> = Vec::new();
    for (id, sites) in emit_field_sets() {
        if sites.len() < 2 { continue; }
        // Herhangi iki nokta ORTAK alan taşıyorsa aynı olgunun dalları sayılır.
        let overlap = sites.iter().enumerate().any(|(a, (_, sa))| {
            sites.iter().skip(a + 1).any(|(_, sb)| !sa.is_disjoint(sb))
        });
        if overlap { continue; }
        let mut all: Vec<String> = sites.iter()
            .map(|(f, s)| format!("{f}:{}", s.iter().cloned().collect::<Vec<_>>().join("+")))
            .collect();
        all.sort_unstable();
        all.dedup();
        if all.len() < 2 { continue; }
        found.push(format!("{id}  {}", all.join("  ")));
    }
    found.sort_unstable();

    let ledger_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests").join("emit_identity_ledger.txt");
    let ledger_raw = fs::read_to_string(&ledger_path).unwrap_or_default();
    let ledger: Vec<String> = ledger_raw.lines().map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_string).collect();

    if found != ledger {
        if std::env::var("UPDATE_LEDGER").is_ok() {
            let header = format!(
                "# Bir rule_id'nin BİRBİRİYLE HİÇ ORTAK ALANI OLMAYAN emit noktaları. {} kural.\n\
                 # Biçim: RULE_ID  dosya:alan+alan  dosya:alan\n\
                 #\n\
                 # Ayrık küme = kural muhtemelen İKİ FARKLI OLGU için kullanılıyor. Bazıları\n\
                 # MEŞRUDUR (aynı olgu birden çok alanda ölçülür); bazıları KİMLİK ÇAKIŞMASIDIR\n\
                 # (registry başlığı yalnız birini anlatır → kullanıcı yanlış etiketli bulgu alır).\n\
                 # Her satır adjudike edilmelidir; gerekçesi aşağıya yazılır.\n\
                 #\n\
                 # Yeniden üretmek: UPDATE_LEDGER=1 cargo test -p gtfs-rules --test emit_identity \\\n\
                 #   rule_ids_emitted_for_disjoint_fields_match_ledger\n",
                found.len());
            fs::write(&ledger_path, format!("{header}{}\n", found.join("\n"))).unwrap();
            return;
        }
        let added: Vec<&String> = found.iter().filter(|f| !ledger.contains(f)).collect();
        let removed: Vec<&String> = ledger.iter().filter(|l| !found.contains(l)).collect();
        panic!(
            "emit kimlik defteri güncel değil ({} hesaplanan, {} defter).\n\
             YENİ ayrık-alan kuralı (KİMLİK ÇAKIŞMASI olabilir — adjudicate et!): {:#?}\n\
             Defterde fazla (emit değişti/kalktı): {:#?}\n\
             Düzeltmek için: UPDATE_LEDGER=1 cargo test -p gtfs-rules --test emit_identity \
             rule_ids_emitted_for_disjoint_fields_match_ledger",
            found.len(), ledger.len(), added, removed,
        );
    }
}
