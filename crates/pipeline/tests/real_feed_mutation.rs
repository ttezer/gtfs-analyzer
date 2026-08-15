//! Gerçek feed üzerinde MUTASYON kanıtı (issue #127).
//!
//! Soru: korpusta hiç ateşlemeyen bir kural "doğru ama doğal olarak sessiz" mi, yoksa
//! "predikatının kör noktası var" mı? Fixture bu ayrımı yapamaz — fixture kuralın
//! ateşleyebildiğini kanıtlar, gerçek okuyucu yolunda doğru davrandığını değil. (#75'te
//! `ARC_022`'nin fixture'ı yeşilken gerçek büyük feed'ler streaming yolundan geçiyordu ve
//! kural susuyordu; İsviçre feed'inde 8,4M satır sessizce geçmişti.)
//!
//! Yöntem: gerçek korpus feed'ini AL, asgari kusuru ENJEKTE ET, mutasyonlu ve mutasyonsuz
//! hâli aynı `config` ve aynı `TODAY` ile koş, **kural id kümesinin FARKINA** bak.
//!
//! ⚠️ Fark iddiası "yalnız hedef kural" DEĞİLDİR. Gerçek feed'e satır enjekte etmek entity
//! map'i, feed düzeyi özet kurallarını ve kaskad bastırmasını da oynatabilir. Ölçüt: hedef
//! kural farkta GÖRÜNMELİ, farkta çıkan başka her kural id'si test içinde AÇIKLANMIŞ
//! olmalı. Yasaklamak, mutasyonu kaskad üretmeyecek vakalara iter ve testi zayıflatır.
//!
//! Feed'ler repoda DEĞİL (`notgit/` git dışı) — bu yüzden testler `#[ignore]`. Koşmak için:
//!   python3 spec-audit/corpus-evidence/verify_corpus.py     # indirir + sha256 doğrular
//!   cargo test -p gtfs-pipeline --test real_feed_mutation -- --ignored
//!
//! Kimlik zinciri: `verify_corpus.py` indirirken **sha256**'yı doğrular; bu test ek bir
//! tel olarak manifest'teki **bayt uzunluğunu** kontrol eder. Testin içinde sha256
//! hesaplamak yeni bir crate bağımlılığı (`sha2`) gerektirirdi ve indirme yolu zaten
//! hash'i denetlediği için eklenmedi.

use std::collections::BTreeSet;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use gtfs_pipeline::{validate_bytes, ValidateResult, ValidatorConfig};

/// Sabit tarih — takvim kuralları tarihe göreli, deterministik olsun.
const TODAY: u32 = 20_260_515;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo kökü bulunamadı")
}

/// Manifest'te kayıtlı bayt uzunluğu.
fn manifest_bytes(feed: &str) -> u64 {
    let path = repo_root().join("spec-audit/corpus-evidence/corpus_manifest.csv");
    let text = std::fs::read_to_string(&path).expect("corpus_manifest.csv okunamadı");
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().expect("manifest boş").split(',').collect();
    let bi = header
        .iter()
        .position(|h| *h == "bytes")
        .expect("manifest'te 'bytes' sütunu yok");
    for line in lines {
        let cols: Vec<&str> = line.split(',').collect();
        if cols.first() == Some(&feed) {
            return cols[bi].parse().expect("bytes sayı değil");
        }
    }
    panic!("{feed} manifest'te yok");
}

/// Korpus arşivini oku; yoksa NASIL getirileceğini söyleyerek düş.
fn corpus_zip(feed: &str) -> Vec<u8> {
    let path = repo_root().join(format!("notgit/corpus/zips/{feed}.zip"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "{} okunamadı ({e}).\n\
             Korpus repoda değil. Getirmek için:\n  \
             python3 spec-audit/corpus-evidence/verify_corpus.py",
            path.display()
        )
    });
    let expected = manifest_bytes(feed);
    assert_eq!(
        bytes.len() as u64,
        expected,
        "{feed} bayt uzunluğu manifest ile uyuşmuyor ({} vs {expected}). \
         Yayıncı feed'i tazelemiş olabilir; kanıt olarak KULLANILAMAZ — \
         manifest'i güncelleyip ölçümü yeniden yapın.",
        bytes.len()
    );
    bytes
}

/// Arşivdeki tek bir üyeyi yeniden yazar, kalan üyeleri aynen kopyalar.
fn rewrite_member(zip: &[u8], target: &str, f: impl FnOnce(String) -> String) -> Vec<u8> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(zip)).expect("arşiv açılamadı");
    let names: Vec<String> = archive.file_names().map(str::to_string).collect();
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();

    let mut hit = false;
    let mut mutate = Some(f);
    for name in &names {
        let mut entry = archive.by_name(name).expect("üye okunamadı");
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).expect("üye içeriği okunamadı");
        drop(entry);

        out.start_file(name.as_str(), opts).expect("üye yazılamadı");
        if Path::new(name).file_name().and_then(|s| s.to_str()) == Some(target) {
            hit = true;
            let text = String::from_utf8(buf).expect("hedef dosya UTF-8 değil");
            let m = mutate.take().expect("hedef dosya birden fazla kez eşleşti");
            out.write_all(m(text).as_bytes()).expect("mutasyon yazılamadı");
        } else {
            out.write_all(&buf).expect("üye kopyalanamadı");
        }
    }
    assert!(hit, "{target} arşivde bulunamadı");
    out.finish().expect("arşiv kapanmadı").into_inner()
}

/// Feed'i koş, üretilen kural id kümesini döndür.
fn rule_ids(zip: &[u8]) -> BTreeSet<String> {
    match validate_bytes(zip, &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => vr.notices.iter().map(|n| n.rule_id.to_string()).collect(),
        ValidateResult::Fatal(e) => panic!("Fatal: {:?} — {}", e.code, e.message),
    }
}

/// Bir dosyanın DQ_016 kök notice'ının bastırdığını BEYAN ETTİĞİ kurallar.
///
/// Bastırmayı "fark boş kaldı" diye ÇIKARSAMAK zayıftır — mutasyon hiç uygulanmamış olsa
/// da fark boş kalırdı. K7 kök notice'a `suppressed_derivative_rules` yazar; ölçüt odur.
fn suppressed_rules_for(zip: &[u8], file: &str) -> Vec<String> {
    match validate_bytes(zip, &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => vr
            .notices
            .iter()
            .filter(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some(file))
            .filter_map(|n| n.details.as_ref()?.get("suppressed_derivative_rules"))
            .flat_map(|v| v.split(',').map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        ValidateResult::Fatal(e) => panic!("Fatal: {:?}", e.code),
    }
}

/// Mutasyonun EKLEDİĞİ ve KALDIRDIĞI kural id'leri.
fn delta(before: &BTreeSet<String>, after: &BTreeSet<String>) -> (Vec<String>, Vec<String>) {
    (
        after.difference(before).cloned().collect(),
        before.difference(after).cloned().collect(),
    )
}

/// Bir sütunun `n`. veri satırındaki değerini değiştirir (n = 1 → ilk veri satırı).
fn set_field(text: &str, column: &str, row_ordinal: usize, value: &str) -> String {
    let mut lines: Vec<String> = text
        .lines()
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect();
    let header = split_csv_line(&lines[0]);
    let ci = header
        .iter()
        .position(|h| h == column)
        .unwrap_or_else(|| panic!("{column} sütunu yok: {header:?}"));
    let li = row_ordinal;
    assert!(li < lines.len(), "{row_ordinal}. veri satırı yok");
    let mut cols = split_csv_line(&lines[li]);
    assert_eq!(cols.len(), header.len(), "alan sayısı başlıkla uyuşmuyor");
    cols[ci] = value.to_string();
    lines[li] = join_csv_fields(&cols);
    lines.join("\n") + "\n"
}

/// Farkı denetler: hedef GÖRÜNMELİ, kalan her id `explained` içinde AÇIKLANMIŞ olmalı.
fn assert_delta(added: &[String], removed: &[String], target: &str, explained: &[&str]) {
    assert!(
        added.iter().any(|r| r == target),
        "{target} mutasyondan sonra ÇIKMADI. Eklenen: {added:?}"
    );
    let unexplained: Vec<&String> = added
        .iter()
        .filter(|r| *r != target && !explained.contains(&r.as_str()))
        .collect();
    assert!(
        unexplained.is_empty(),
        "{target} için AÇIKLANMAMIŞ yan etki: {unexplained:?}\n\
         Bunlar hata olmayabilir (kaskad/özet kuralları) ama teste gerekçesiyle YAZILMALI."
    );
    assert!(
        removed.is_empty(),
        "{target} mutasyonu bulgu KALDIRDI: {removed:?} — bastırma etkisi incelenmeli."
    );
}

/// İlk veri satırını (başlıktan sonraki) döndürür; `\r` kırpılır.
fn first_data_line(text: &str) -> &str {
    text.lines()
        .nth(1)
        .expect("dosyada veri satırı yok")
        .trim_end_matches('\r')
}

/// Metnin sonuna satır ekler (sondaki yeni satırı normalize ederek).
fn append_line(text: &str, line: &str) -> String {
    let mut s = text.trim_end_matches(['\n', '\r']).to_string();
    s.push('\n');
    s.push_str(line);
    s.push('\n');
    s
}

/// RFC 4180 alan bölme. Naif `split(',')` ŞART DEĞİL, ÖLÇÜLMÜŞ bir gereklilik:
/// `mdb-3141`'in `calendar.txt`'inde `service_id` değeri `"1,2,3,4,5,6,7"` — tırnak içinde
/// altı virgül. Naif bölme 10 alanlık satırı 16 alan sanıp bozuk satır üretirdi.
fn split_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let mut it = line.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '"' if in_q && it.peek() == Some(&'"') => {
                cur.push('"');
                it.next();
            }
            '"' => in_q = !in_q,
            ',' if !in_q => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// Alanları geri birleştirir; virgül/tırnak/yeni satır taşıyan alanı yeniden tırnaklar.
fn join_csv_fields(fields: &[String]) -> String {
    fields
        .iter()
        .map(|f| {
            if f.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", f.replace('"', "\"\""))
            } else {
                f.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Satırın ilk alanını boşaltır (alan sayısı başlıkla uyuşmalı).
fn blank_first_field(header: &str, line: &str) -> String {
    let n = split_csv_line(header.trim_end_matches('\r')).len();
    let mut cols = split_csv_line(line);
    assert_eq!(cols.len(), n, "alan sayısı başlıkla uyuşmuyor: {line}");
    cols[0] = String::new();
    join_csv_fields(&cols)
}

// ─────────────────────────────────────────────────────────────────────────────
// SHP_005 — shape_dist_traveled shape_pt_sequence ile artmalı
// ─────────────────────────────────────────────────────────────────────────────

const SHP_FEED: &str = "mdb-2927";

/// İlk shape'in SON noktasının mesafesini bir öncekinin ALTINA (ya da EŞİTİNE) çeker.
///
/// ⚠️ İlk sürüm İKİNCİ noktayı `first - 1.0` yapıyordu ve DÜŞTÜ: `mdb-2927`'de ilk noktanın
/// mesafesi `0`, `(0.0-1.0).max(0.0)` yine `0` → azalma değil EŞİTLİK üretiyordu ve
/// beklenen SHP_005 yerine SHP_028 çıkıyordu. Kuralın değil MUTASYONUN hatasıydı; son
/// noktayı hedeflemek mesafenin pozitif olmasını garanti eder.
fn break_last_distance(text: &str, equal_instead: bool) -> String {
    let mut lines: Vec<String> = text
        .lines()
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect();
    let header = split_csv_line(&lines[0]);
    let id_i = header.iter().position(|h| h == "shape_id").expect("shape_id yok");
    let d_i = header
        .iter()
        .position(|h| h == "shape_dist_traveled")
        .expect("shape_dist_traveled yok");

    let first_id = split_csv_line(&lines[1])[id_i].clone();
    // Aynı shape'in son iki satırının indeksleri.
    let idxs: Vec<usize> = lines
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(_, l)| split_csv_line(l).get(id_i) == Some(&first_id))
        .map(|(i, _)| i)
        .collect();
    assert!(idxs.len() >= 2, "ilk shape'in en az iki noktası olmalı");
    let (prev_i, last_i) = (idxs[idxs.len() - 2], idxs[idxs.len() - 1]);

    let prev_d: f64 = split_csv_line(&lines[prev_i])[d_i]
        .parse()
        .expect("önceki mesafe sayı değil");
    assert!(prev_d > 0.0, "mutasyon için önceki mesafe pozitif olmalı");

    let mut cols = split_csv_line(&lines[last_i]);
    cols[d_i] = if equal_instead {
        format!("{prev_d}")
    } else {
        format!("{}", prev_d - 1.0)
    };
    lines[last_i] = join_csv_fields(&cols);
    lines.join("\n") + "\n"
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn shp_005_fires_when_shape_distance_decreases_on_a_real_feed() {
    let zip = corpus_zip(SHP_FEED);
    let before = rule_ids(&zip);
    assert!(
        !before.contains("SHP_005"),
        "{SHP_FEED} mutasyondan ÖNCE zaten SHP_005 üretiyor — kontrol feed'i olamaz"
    );

    let mutated = rewrite_member(&zip, "shapes.txt", |t| break_last_distance(&t, false));
    let after = rule_ids(&mutated);
    let (added, removed) = delta(&before, &after);

    // AÇIKLANAN yan etkiler (ölçüldü, yasaklanmadı):
    //   SHP_024 — "duraktan şekle mesafe shape_dist_traveled ile tutarsız". Son noktanın
    //             mesafesini geri almak, o noktaya eşlenen durağın mesafe tutarlılığını da
    //             bozar. Mutasyonun DOĞRUDAN sonucu; kaskadı yasaklamak testi zayıflatırdı.
    //   SHP_028/029/020 — mesafe artmadan konum değişmesi ailesi; eşitliğe düşen
    //             varyantlarda ortaya çıkabilir.
    assert_delta(
        &added,
        &removed,
        "SHP_005",
        &["SHP_024", "SHP_028", "SHP_029", "SHP_020"],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn shp_005_stays_silent_on_equal_distances_because_three_other_rules_measure_that() {
    // SINIR DEĞERİ. Spec "must increase" der, yani EŞİTLİK de bir ihlaldir. Ama eşitliği
    // SHP_005'e eklemek aynı olguyu ölçen BEŞİNCİ kural olurdu — koordinat eksenine göre
    // olgu zaten TAM BÖLÜNMÜŞ durumda:
    //   SHP_023 eşit mesafe + AYNI koordinat      (Orta   · Quality   · korpusta 38 feed)
    //   SHP_028 eşit mesafe + FARKLI koordinat    (Yüksek · Quality   · korpusta  9 feed)
    //   SHP_029 eşit mesafe + çok yakın koordinat (Bilgi  · Quality   · korpusta 24 feed)
    //   SHP_020 tekrarlayan nokta                 (Bilgi  · Analytics · korpusta 152 feed)
    // Bu test mevcut davranışı ÇAPALAR: eşitlik SHP_005 üretmez, başka kural üretir.
    // Geriye kalan tek soru PREDİKAT değil SINIF: bir Spec normunun ihlali yalnız Quality
    // olarak raporlanıyor. Yayın kapısını (R1 = Spec ∧ Kritik) etkilemez, çünkü hiçbiri
    // Kritik değil.
    let zip = corpus_zip(SHP_FEED);
    let before = rule_ids(&zip);

    let mutated = rewrite_member(&zip, "shapes.txt", |t| break_last_distance(&t, true));
    let after = rule_ids(&mutated);
    let (added, _) = delta(&before, &after);

    assert!(
        !added.iter().any(|r| r == "SHP_005"),
        "eşit mesafe SHP_005 üretti — davranış değişmiş, kart ve #127 güncellenmeli"
    );
    assert!(
        added
            .iter()
            .any(|r| ["SHP_023", "SHP_028", "SHP_029", "SHP_020"].contains(&r.as_str())),
        "eşit mesafe HİÇBİR kural üretmedi — olgu artık ölçülmüyor. Eklenen: {added:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Yineleme kuralları — CAL_001 · FAR_001 · LVL_001
// ─────────────────────────────────────────────────────────────────────────────

/// Ortak gövde: ilk veri satırını AYNEN tekrarla → yineleme kuralı ateşlemeli.
fn duplicate_row_case(feed: &str, file: &str, target: &str, explained: &[&str]) {
    let zip = corpus_zip(feed);
    let before = rule_ids(&zip);
    assert!(
        !before.contains(target),
        "{feed} mutasyondan ÖNCE zaten {target} üretiyor — kontrol feed'i olamaz"
    );

    let mutated = rewrite_member(&zip, file, |t| {
        let dup = first_data_line(&t).to_string();
        append_line(&t, &dup)
    });
    let (added, removed) = delta(&before, &rule_ids(&mutated));
    assert_delta(&added, &removed, target, explained);
}

/// Ortak gövde: kimliği BOŞ iki satır ekle → yineleme kuralı ateşlememeli (boş anahtar
/// `continue` ile atlanır), ama KÖK zorunlu-alan kuralı ateşlemeli.
fn empty_key_boundary(feed: &str, file: &str, dup_rule: &str, root_rule: &str) {
    let zip = corpus_zip(feed);
    let before = rule_ids(&zip);

    let mutated = rewrite_member(&zip, file, |t| {
        let header = t.lines().next().expect("başlık yok").to_string();
        let blank = blank_first_field(&header, first_data_line(&t));
        append_line(&append_line(&t, &blank), &blank)
    });
    let (added, _) = delta(&before, &rule_ids(&mutated));

    assert!(
        !added.iter().any(|r| r == dup_rule),
        "boş kimlik {dup_rule} üretti — kod `is_empty()` ile atlıyordu, davranış değişmiş"
    );
    assert!(
        added.iter().any(|r| r == root_rule) || before.contains(root_rule),
        "boş kimlik KÖK kural {root_rule}'i de üretmedi → olgu HİÇ raporlanmıyor, \
         bu gerçek bir boşluktur. Eklenen: {added:?}"
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn cal_001_fires_on_a_duplicated_calendar_row() {
    duplicate_row_case("mdb-3141", "calendar.txt", "CAL_001", &[]);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn cal_001_ignores_duplicate_empty_service_ids_because_cal_022_reports_the_root() {
    empty_key_boundary("mdb-3141", "calendar.txt", "CAL_001", "CAL_022");
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn far_001_fires_on_a_duplicated_fare_attribute_row() {
    duplicate_row_case("mdb-1996", "fare_attributes.txt", "FAR_001", &[]);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn far_001_ignores_duplicate_empty_fare_ids_because_far_012_reports_the_root() {
    empty_key_boundary("mdb-1996", "fare_attributes.txt", "FAR_001", "FAR_012");
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn lvl_001_fires_on_a_duplicated_level_row() {
    duplicate_row_case("mdb-3357", "levels.txt", "LVL_001", &[]);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn lvl_001_ignores_duplicate_empty_level_ids_because_lvl_008_reports_the_root() {
    empty_key_boundary("mdb-3357", "levels.txt", "LVL_001", "LVL_008");
}

// ─────────────────────────────────────────────────────────────────────────────
// TRN_005 — birebir yinelenen çeviri satırı
// ─────────────────────────────────────────────────────────────────────────────

// ⚠️ Feed seçimi ÖLÇÜLEREK yapıldı: issue'da önerilen `mdb-1158`'in translations.txt'i
// ESKİ formatta (`trans_id,lang,translation`) ve SIFIR satır — mutasyona uygun değil.
// `mdb-2638` modern formatta, hem `record_id` hem `field_value` sütunu var; ikinci
// sütun TRN_005'in field_value tabanlı kimlik yolunu sınamak için gerekli.
const TRN_FEED: &str = "mdb-2638";

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn trn_005_fires_on_an_exactly_duplicated_translation_row() {
    duplicate_row_case(TRN_FEED, "translations.txt", "TRN_005", &[]);
}

/// Kimliği `field_value`'dan alan feed. ⚠️ Aday ÖLÇÜLEREK seçildi: `mdb-2638`'de
/// `field_value` SÜTUNU var ama 128 satırın hepsinde BOŞ — o feed bu yolu sınayamaz.
/// `mdb-3176` `record_id` sütununu hiç taşımaz, kimlik tamamen `field_value`'dur.
const TRN_FIELD_VALUE_FEED: &str = "mdb-3176";

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn trn_005_uses_field_value_as_identity_when_there_is_no_record_id() {
    // SINIR DEĞERİ. `has_translation_identity` table+field+language ister ve kimliği
    // record_id VEYA field_value'dan alır. Kimliğin TAMAMINI silmek yanlış şeyi sınardı
    // (bozuk-satır kapısını, yineleme tespitini değil) — bu yüzden kimliği yalnız
    // field_value'dan gelen bir satır birebir tekrarlanır.
    let zip = corpus_zip(TRN_FIELD_VALUE_FEED);
    let before = rule_ids(&zip);
    assert!(
        !before.contains("TRN_005"),
        "{TRN_FIELD_VALUE_FEED} zaten TRN_005 üretiyor"
    );

    let mutated = rewrite_member(&zip, "translations.txt", |t| {
        let header = split_csv_line(t.lines().next().expect("başlık yok").trim_end_matches('\r'));
        assert!(
            !header.iter().any(|h| h == "record_id"),
            "bu feed record_id taşıyor — field_value kimlik yolu izole edilemez"
        );
        let fv = header
            .iter()
            .position(|h| h == "field_value")
            .expect("field_value sütunu yok — aday seçimi geçersiz");

        let row = t
            .lines()
            .skip(1)
            .map(|l| l.trim_end_matches('\r'))
            .find(|l| {
                let c = split_csv_line(l);
                c.len() == header.len() && !c[fv].is_empty()
            })
            .expect("field_value'su dolu satır yok")
            .to_string();
        append_line(&t, &row)
    });

    let (added, removed) = delta(&before, &rule_ids(&mutated));
    assert_delta(&added, &removed, "TRN_005", &[]);
}

// ─────────────────────────────────────────────────────────────────────────────
// Tipli alan kuralları — STM_003 · STM_004 · STP_004 · STP_005 (issue #129)
// ─────────────────────────────────────────────────────────────────────────────

// ⚠️ Aday feed İÇERİKLE seçildi, boyutla değil (batch 1'in dersi): `mdb-1903`'te
// `arrival_time`/`departure_time` ilk 4000 satırın hepsinde DOLU, `stops.txt`'in 362
// satırının hepsinde `stop_lat`/`stop_lon` DOLU, ve dört hedef kuralın dördü de taban
// çizgisinde SIFIR.
//
// 🔑 Kardeş kural kontrolü mutasyondan ÖNCE yapıldı ve soruyu daralttı: ayrıştırılmış
// saati tüketen STM_012 (37 feed) · STM_036 (31) · STM_035 (21) ve ayrıştırılmış
// koordinatı tüketen SHP_009 (162) · SHP_020 (152) · STM_045 (79) korpusta ateşliyor.
// Yani BAŞARI dalı zaten kanıtlı; bu testlerin ölçtüğü şey HATA dalının bağlı olup
// olmadığıdır.
const TYPED_FEED: &str = "mdb-1903";

/// Hedef görünmeli; eklenen VE kaldırılan yan etkilerin her biri açıklanmış olmalı.
fn assert_delta_both(
    added: &[String],
    removed: &[String],
    target: &str,
    explained_added: &[&str],
    explained_removed: &[&str],
) {
    assert!(
        added.iter().any(|r| r == target),
        "{target} mutasyondan sonra ÇIKMADI. Eklenen: {added:?}"
    );
    let bad_add: Vec<&String> = added
        .iter()
        .filter(|r| *r != target && !explained_added.contains(&r.as_str()))
        .collect();
    assert!(bad_add.is_empty(), "{target}: AÇIKLANMAMIŞ yeni bulgu {bad_add:?}");
    let bad_rem: Vec<&String> = removed
        .iter()
        .filter(|r| !explained_removed.contains(&r.as_str()))
        .collect();
    assert!(bad_rem.is_empty(), "{target}: AÇIKLANMAMIŞ kaybolan bulgu {bad_rem:?}");
}

fn typed_field_case(
    file: &str,
    column: &str,
    value: &str,
    target: &str,
    explained_added: &[&str],
    explained_removed: &[&str],
) {
    let zip = corpus_zip(TYPED_FEED);
    let before = rule_ids(&zip);
    assert!(
        !before.contains(target),
        "{TYPED_FEED} mutasyondan ÖNCE zaten {target} üretiyor — kontrol feed'i olamaz"
    );
    let (col, val) = (column.to_string(), value.to_string());
    let mutated = rewrite_member(&zip, file, move |t| set_field(&t, &col, 1, &val));
    let (added, removed) = delta(&before, &rule_ids(&mutated));
    assert_delta_both(&added, &removed, target, explained_added, explained_removed);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn stm_003_fires_on_a_malformed_arrival_time() {
    // AÇIKLANAN yan etkiler (ölçüldü): ayrıştırılamayan saat aşağı akışta YOK sayılır.
    //   STM_015 — mutasyon bir seferin İLK durak satırına düştü → "ilk durakta
    //             arrival_time eksik". Doğrudan sonuç.
    //   STM_034 — varış/kalkıştan yalnız biri tanımlı kaldı (Interop).
    typed_field_case(
        "stop_times.txt", "arrival_time", "abc", "STM_003",
        &["STM_015", "STM_034"], &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn stm_004_fires_on_a_malformed_departure_time() {
    // STM_034 aynı sebeple: kalkış bozulunca varış/kalkıştan yalnız biri tanımlı kalır.
    // STM_015 burada ÇIKMAZ — o kural arrival_time'a bakar; asimetri kasıtlıdır
    // (spec ilk/son durakta arrival_time'ı şart koşar, departure_time'da bu madde YOK).
    typed_field_case(
        "stop_times.txt", "departure_time", "abc", "STM_004",
        &["STM_034"], &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn stp_004_fires_on_a_non_numeric_stop_lat() {
    // Yan etki YOK (ölçüldü): tek durağın koordinatı bozulunca hiçbir toplu/kaskad
    // kural sınıf değiştirmiyor.
    typed_field_case("stops.txt", "stop_lat", "abc", "STP_004", &[], &[]);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn stp_005_fires_on_an_invalid_stop_lon_without_whitespace() {
    // KONTRASTIN BİRİNCİ YARISI. Boşluk YOK → DQ_016 kökü doğmaz → bastırma devreye
    // girmez → STP_005 görünür. Bu, predikatın kendisinin çalıştığını kanıtlar.
    typed_field_case("stops.txt", "stop_lon", "abc", "STP_005", &[], &[]);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn stp_005_is_born_and_suppressed_when_the_value_only_wears_whitespace() {
    // KONTRASTIN İKİNCİ YARISI — issue #129'un asıl sorusu.
    //
    // STP_005'in korpustaki sessizliği "hiç doğmadı" DEĞİL, en azından kısmen "doğdu ve
    // bastırıldı"dır: `mdb-2691`'de `stop_lon=" -4.732529"` STP_005'i ÜRETİR ve K7'nin
    // whitespace-türev kaskadı onu DQ_016 kökü altında bastırır. Bu mekanizma
    // `silent_rules.tsv`'de MODELLENEMEZ (tablo yalnız registry'de İLAN EDİLEN kaskadı
    // görür), bu yüzden kanıt buraya yazılır.
    //
    // Tek başına hiçbir yarı bir şey söylemez: üstteki test boşluksuz geçersiz değerin
    // ateşlediğini, bu test trim sonrası GEÇERLİ olan değerin ateşlemediğini gösterir.
    // İkisi birlikte "bastırma mı, kırık predikat mı" sorusunu keser.
    let zip = corpus_zip(TYPED_FEED);
    let before = rule_ids(&zip);
    assert!(!before.contains("STP_005"));
    // Taban çizgisi ZATEN bir DQ_016 taşıyor (stops.txt / stop_name), bu yüzden kök
    // notice farkta görünmez; beklenen fark BOŞ kümedir.
    assert!(before.contains("DQ_016"), "taban çizgisi DQ_016 taşımalıydı");

    let lon = {
        let mut a = zip::ZipArchive::new(std::io::Cursor::new(&zip)).unwrap();
        let name = a
            .file_names()
            .find(|n| Path::new(n).file_name().and_then(|s| s.to_str()) == Some("stops.txt"))
            .unwrap()
            .to_string();
        let mut e = a.by_name(&name).unwrap();
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut e, &mut buf).unwrap();
        let header = split_csv_line(buf.lines().next().unwrap().trim_end_matches('\r'));
        let i = header.iter().position(|h| h == "stop_lon").unwrap();
        split_csv_line(first_data_line(&buf))[i].clone()
    };
    let padded = format!(" {lon}");
    let mutated = rewrite_member(&zip, "stops.txt", move |t| {
        set_field(&t, "stop_lon", 1, &padded)
    });
    let (added, removed) = delta(&before, &rule_ids(&mutated));

    assert!(
        !added.iter().any(|r| r == "STP_005"),
        "trim sonrası GEÇERLİ bir değer STP_005 üretti — whitespace kaskadı artık \
         bastırmıyor demektir; DQ_016 kartı ve #129 güncellenmeli. Eklenen: {added:?}"
    );
    assert!(
        added.is_empty() && removed.is_empty(),
        "beklenen fark BOŞ kümeydi (kök DQ_016 zaten vardı). eklenen={added:?} kaldırılan={removed:?}"
    );

    // 🔑 ASIL İDDİA. Boş fark tek başına "bastırıldı" demez — mutasyon hiç uygulanmamış
    // olsa da fark boş kalırdı. K7 kök notice'a bastırdığı kuralları YAZAR; testin
    // dayandığı şey o beyandır. Korpusta bağımsız örnek: `mdb-2691`'in stops.txt kökü
    // `suppressed_derivative_rules = STP_005` taşıyor.
    let suppressed = suppressed_rules_for(&mutated, "stops.txt");
    assert!(
        suppressed.iter().any(|r| r == "STP_005"),
        "DQ_016 kökü STP_005'i bastırdığını BEYAN ETMİYOR → boş fark bastırmadan değil, \
         mutasyonun hiç doğmamasından geliyor olabilir. Beyan edilen: {suppressed:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Çapraz referans + Flex — PTH_001 · PTH_026 · TRF_021 · STM_058 (issue #130)
// ─────────────────────────────────────────────────────────────────────────────

// Batch 1/2 K3 yineleme ve K2 tip yollarını kapsadı; bu parti bilinçli olarak KOD YOLU
// değiştiriyor: K4 çapraz referans (PTH_026, TRF_021) ve K2 Flex akışı (STM_058).
//
// 🔑 Kardeş kontrolü (mutasyondan önce, bedava): pathways okunuyor (`PTH_007` · `PTH_011` ·
// `PTH_012`), transfers okunuyor (`TRF_016` 5204 bulgu · `TRF_019` · `TRF_022/023`),
// Flex yolu okunuyor (`STM_056` 3,97M bulgu · `XFL_002` 1315 · `LOC_006`). Üçünde de
// dosyanın işlendiği kanıtlı; ölçülen şey yine HATA/İHLAL dalı.

/// Arşivdeki bir üyenin metnini döndürür.
fn member_text(zip: &[u8], file: &str) -> String {
    let mut a = zip::ZipArchive::new(std::io::Cursor::new(zip)).expect("arşiv açılamadı");
    let name = a
        .file_names()
        .find(|n| Path::new(n).file_name().and_then(|s| s.to_str()) == Some(file))
        .unwrap_or_else(|| panic!("{file} arşivde yok"))
        .to_string();
    let mut e = a.by_name(&name).expect("üye okunamadı");
    let mut buf = String::new();
    std::io::Read::read_to_string(&mut e, &mut buf).expect("üye UTF-8 değil");
    buf
}

/// `stops.txt`'te verilen `location_type` değerine sahip ilk `stop_id`.
fn stop_id_with_location_type(zip: &[u8], want: &str) -> String {
    let text = member_text(zip, "stops.txt");
    let header = split_csv_line(text.lines().next().unwrap().trim_end_matches('\r'));
    let id_i = header.iter().position(|h| h == "stop_id").expect("stop_id yok");
    let lt_i = header
        .iter()
        .position(|h| h == "location_type")
        .expect("location_type sütunu yok");
    text.lines()
        .skip(1)
        .map(|l| split_csv_line(l.trim_end_matches('\r')))
        .find(|c| c.len() == header.len() && c[lt_i].trim() == want)
        .map(|c| c[id_i].clone())
        .unwrap_or_else(|| panic!("location_type={want} olan durak yok"))
}

/// `key_col`'u boş OLMAYAN ilk veri satırının sıra numarası (1 = ilk veri satırı).
fn first_row_with_value(text: &str, key_col: &str) -> usize {
    let header = split_csv_line(text.lines().next().unwrap().trim_end_matches('\r'));
    let ki = header
        .iter()
        .position(|h| h == key_col)
        .unwrap_or_else(|| panic!("{key_col} sütunu yok"));
    text.lines()
        .enumerate()
        .skip(1)
        .find(|(_, l)| {
            let c = split_csv_line(l.trim_end_matches('\r'));
            c.len() == header.len() && !c[ki].trim().is_empty()
        })
        .map(|(i, _)| i)
        .unwrap_or_else(|| panic!("{key_col} dolu satır yok"))
}

const PTH_FEED: &str = "mdb-3357";
const TRF_FEED: &str = "mdb-2848";
const FLEX_FEED: &str = "mdb-3037";

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn pth_001_fires_on_a_duplicated_pathway_row() {
    duplicate_row_case(PTH_FEED, "pathways.txt", "PTH_001", &[]);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn pth_026_fires_when_a_pathway_endpoint_is_a_station() {
    // K4 çapraz referans. `from_stop_id` GERÇEKTEN VAR OLAN bir istasyona (location_type=1)
    // çevrilir — uydurma id kullanmak FK kuralını sınardı, uç-nokta TİPİ kuralını değil.
    let zip = corpus_zip(PTH_FEED);
    let before = rule_ids(&zip);
    assert!(!before.contains("PTH_026"), "{PTH_FEED} zaten PTH_026 üretiyor");
    let station = stop_id_with_location_type(&zip, "1");
    let mutated = rewrite_member(&zip, "pathways.txt", move |t| {
        set_field(&t, "from_stop_id", 1, &station)
    });
    let (added, removed) = delta(&before, &rule_ids(&mutated));
    assert_delta_both(&added, &removed, "PTH_026", &[], &[]);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn trf_021_fires_when_a_transfer_endpoint_is_neither_stop_nor_station() {
    // Aynı gerekçe: var olan bir GİRİŞ (location_type=2) hedeflenir.
    let zip = corpus_zip(TRF_FEED);
    let before = rule_ids(&zip);
    assert!(!before.contains("TRF_021"), "{TRF_FEED} zaten TRF_021 üretiyor");
    let entrance = stop_id_with_location_type(&zip, "2");
    let mutated = rewrite_member(&zip, "transfers.txt", move |t| {
        set_field(&t, "from_stop_id", 1, &entrance)
    });
    let (added, removed) = delta(&before, &rule_ids(&mutated));
    // AÇIKLANAN yan etki: `TRF_011` (aktarma tanımlandı ama mesafe uzak, Bilgi·Quality).
    // Aktarmayı ağın başka bir yerindeki girişe çevirmek iki ucu coğrafi olarak
    // uzaklaştırır; doğrudan geometrik sonuç.
    assert_delta_both(&added, &removed, "TRF_021", &["TRF_011"], &[]);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn stm_058_fires_on_a_malformed_flex_pickup_window() {
    // K2 akış yolu + Flex. ⚠️ Satır seçimi ÖLÇÜMLE yapılır: korpusun 19 feed'i bu başlığı
    // taşıyor ama YALNIZ 7'sinde değer var; boş bir pencereye mutasyon uygulamak hiçbir
    // şeyi sınamazdı. Bu yüzden penceresi DOLU ilk satır aranır.
    // STM_058 ayrıca `Err(_) => None` yutma sınıfının ilk üyesiydi (`778c462b`'de kapandı);
    // onarılmış bir parse yolunun hâlâ emit ettiğini kanıtlamak bu testin asıl amacı.
    let zip = corpus_zip(FLEX_FEED);
    let before = rule_ids(&zip);
    assert!(!before.contains("STM_058"), "{FLEX_FEED} zaten STM_058 üretiyor");
    let text = member_text(&zip, "stop_times.txt");
    let row = first_row_with_value(&text, "start_pickup_drop_off_window");
    let mutated = rewrite_member(&zip, "stop_times.txt", move |t| {
        set_field(&t, "start_pickup_drop_off_window", row, "abc")
    });
    let (added, removed) = delta(&before, &rule_ids(&mutated));
    // AÇIKLANAN yan etki — ve bu KAYBOLAN bir bulgu: `PDW_006` (aynı trip+zone'da örtüşen
    // pickup/drop-off penceresi, Orta·Analytics). Pencereyi ayrıştırılamaz yapmak örtüşme
    // analizinden o pencereyi DÜŞÜRÜR, dolayısıyla parçası olduğu örtüşme bulgusu da yok
    // olur. Meşru sonuç — ve testin kaldırmaları YASAKLAMAYIP açıklamasının sebebi tam bu:
    // yasaklasaydık bu mutasyon hiç yazılamazdı.
    assert_delta_both(&added, &removed, "STM_058", &[], &["PDW_006"]);
}
