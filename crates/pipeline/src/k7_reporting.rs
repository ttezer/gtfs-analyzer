use std::collections::{BTreeMap, BTreeSet, HashMap};
use rustc_hash::FxHashSet;

use gtfs_core::{
    DedupLevel, EntityType, FeedMetrics, FileInfo, Notice, R1Report, R2Report, R3Report, R4Report, R5Report,
    R7Report, R8Report, R9Item, R9Label, R9Report, ReportId, ReportItem, ReportSet,
    RuleClass, Severity,
};
use gtfs_rules::get_rule;

use crate::k2::EntityRecords;
use crate::k5_derived::DerivedData;

// ── Çıktı tipi ────────────────────────────────────────────────────────────────

pub struct K7Result {
    pub notices: Vec<Notice>,
    pub reports: ReportSet,
    pub metrics: FeedMetrics,
}

// ── Ana fonksiyon ─────────────────────────────────────────────────────────────

pub fn report(
    all_notices: Vec<Notice>,
    records: &EntityRecords,
    derived: &DerivedData,
    file_stats: Vec<FileInfo>,
    already_deduped: bool,
    coverage_complete: bool,
) -> K7Result {
    use crate::timing::Timer;
    let all_notices      = {
        let _t = Timer::start("K7::suppress_whitespace_derivatives");
        suppress_whitespace_derivatives(all_notices, records)
    };
    let all_notices      = { let _t = Timer::start("K7::fill_service_ids"); fill_service_ids(all_notices, records) };
    let mut notices      = if already_deduped {
        all_notices
    } else {
        let _t = Timer::start("K7::dedup");
        dedup(all_notices)
    };
    // Determinizm: id'ler katmanların EMİSYON sırasından geliyordu ve o sıra HashMap
    // iterasyonlarına bağlı olduğu için koşudan koşuya kayıyordu (içerik aynı, id farklı).
    // Dedup çıktısı `notice_order_key` ile kararlı sıralı olduğundan, id'ler burada yeniden
    // atanınca deterministik olur. Katman öneki (k1/k2/…) korunur; raporlar bu listeden
    // sonra kurulduğu için referanslar tutarlı kalır.
    for (i, n) in notices.iter_mut().enumerate() {
        let prefix = n.id.split('/').next().unwrap_or("k").to_string();
        n.id = format!("{prefix}/{}#{}", n.rule_id, i + 1);
    }
    let resolution       = { let _t = Timer::start("K7::resolve_symptoms"); resolve_symptoms(&notices) };
    let reports          = { let _t = Timer::start("K7::build_reports");    build_report_set(&notices, &resolution, coverage_complete) };
    let mut metrics      = { let _t = Timer::start("K7::build_metrics");    build_metrics(&notices, records, derived, file_stats, coverage_complete) };
    metrics.overall_score = reports.r5.score;
    K7Result { notices, reports, metrics }
}

/// DQ_016 kök bulgusunu koruyup, yalnızca ham değerin çevresindeki boşluk nedeniyle oluşan
/// tip/FK semptomlarını rapor dışına alır. K2 hâlâ ham değeri parse eder; hiçbir PK/FK değeri
/// burada veya daha erken normalleştirilmez. Kök bulguya sayısal ve kural bazlı audit özeti
/// yazılır; böylece varsayılan rapor küçük kalırken strict/audit tüketicisi neyin bastırıldığını
/// görebilir.
fn suppress_whitespace_derivatives(
    mut notices: Vec<Notice>,
    records: &EntityRecords,
) -> Vec<Notice> {
    let references = WhitespaceReferences::from_records(records);
    let mut suppressed: BTreeMap<String, (u64, BTreeSet<String>)> = BTreeMap::new();
    notices.retain(|notice| {
        if !is_whitespace_derivative(notice, &references) {
            return true;
        }
        let file = notice.file.clone().unwrap_or_else(|| "<feed>".to_string());
        let entry = suppressed.entry(file).or_default();
        entry.0 += 1;
        entry.1.insert(notice.rule_id.clone());
        false
    });

    for root in notices.iter_mut().filter(|n| n.rule_id == "DQ_016") {
        let Some(file) = root.file.as_deref() else { continue };
        let Some((count, rules)) = suppressed.get(file) else { continue };
        let details = root.details.get_or_insert_with(BTreeMap::new);
        details.insert("suppressed_derivative_count".to_string(), count.to_string());
        details.insert(
            "suppressed_derivative_rules".to_string(),
            rules.iter().cloned().collect::<Vec<_>>().join(", "),
        );
    }
    notices
}

fn is_whitespace_derivative(notice: &Notice, references: &WhitespaceReferences<'_>) -> bool {
    let marked = notice.details.as_ref()
        .and_then(|d| d.get("whitespace_derived"))
        .is_some_and(|v| v == "true");
    if marked {
        return true;
    }

    // ⚠️ BURADA BİR "streaming K2 parse reddi" DALI VARDI — KALDIRILDI (#125, 2026-08-12).
    // Dal, gözlenen değerin çevresinde boşluk olmasını şart koşuyordu; ama stop_times ve
    // shapes'in akış yolundaki emit'lerin hepsi değeri `get_col` ile okur ve o `.trim()`
    // uygular (`k2/common.rs`). `parse_u32_raw`/`parse_f64_raw`/`parse_gtfs_time_raw`
    // adlarındaki "raw" RowMap yerine dilim üzerinden okuma anlamına gelir, HAM DEĞER
    // anlamına gelmez. Yani `observed_value` hiçbir zaman boşluk taşımıyordu, şart asla
    // sağlanmıyordu ve dal erişilemezdi. İki ölçüm: (1) boşluklu shape_pt_lat/
    // shape_dist_traveled içeren feed yalnızca DQ_016 üretti, bastırılacak türev yoktu;
    // (2) dal kapatılıp tam takım koşuldu, 921/921 geçti. Mesaj kalıbı listesinin K2'ninkiyle
    // ayrışmış olması da bu yüzden sonuçsuzdu — erişilemeyen dala kalıp eklenecekti.
    // Akış yolu ile RowMap yolunun FARKLI SERTLİKTE okuması ayrı bir konudur ve durur.

    // K4 yalnızca hedefte bulunmayan ham ID'yi raporlar. Aynı ID'nin trim edilmiş biçimi
    // hedef kümede varsa, bu tam olarak whitespace kaynaklı bir FK semptomudur; gerçekten
    // bilinmeyen "  UNKNOWN " değerleri görünür kalır.
    let candidate = notice.details.as_ref()
        .and_then(|d| d.get("whitespace_candidate"))
        .is_some_and(|v| v == "true");
    candidate && normalized_reference_exists(notice, references)
}

fn normalized_reference_exists(notice: &Notice, references: &WhitespaceReferences<'_>) -> bool {
    let Some(field) = notice.field.as_deref() else { return false };
    let Some(observed) = notice.observed_value.as_deref() else { return false };
    field.split('|')
        .zip(observed.split('|'))
        .any(|(field, value)| {
            let value = value.trim();
            !value.is_empty() && references.contains(field, value)
        })
}

struct WhitespaceReferences<'a> {
    agency_id: FxHashSet<&'a str>,
    stop_id: FxHashSet<&'a str>,
    route_id: FxHashSet<&'a str>,
    trip_id: FxHashSet<&'a str>,
    service_id: FxHashSet<&'a str>,
    fare_id: FxHashSet<&'a str>,
    area_id: FxHashSet<&'a str>,
    fare_product_id: FxHashSet<&'a str>,
    fare_media_id: FxHashSet<&'a str>,
    rider_category_id: FxHashSet<&'a str>,
    network_id: FxHashSet<&'a str>,
    shape_id: FxHashSet<&'a str>,
    zone_id: FxHashSet<&'a str>,
}

impl<'a> WhitespaceReferences<'a> {
    fn from_records(records: &'a EntityRecords) -> Self {
        let mut refs = Self {
            agency_id: FxHashSet::default(),
            stop_id: FxHashSet::default(),
            route_id: FxHashSet::default(),
            trip_id: FxHashSet::default(),
            service_id: FxHashSet::default(),
            fare_id: FxHashSet::default(),
            area_id: FxHashSet::default(),
            fare_product_id: FxHashSet::default(),
            fare_media_id: FxHashSet::default(),
            rider_category_id: FxHashSet::default(),
            network_id: FxHashSet::default(),
            shape_id: FxHashSet::default(),
            zone_id: FxHashSet::default(),
        };
        refs.agency_id.extend(records.agencies.iter().filter_map(|r| r.agency_id.as_deref()));
        refs.stop_id.extend(records.stops.iter().map(|r| r.stop_id.as_str()));
        refs.route_id.extend(records.routes.iter().map(|r| r.route_id.as_str()));
        refs.trip_id.extend(records.trips.iter().map(|r| r.trip_id.as_str()));
        refs.service_id.extend(records.calendars.iter().map(|r| r.service_id.as_str()));
        refs.service_id.extend(records.trip_interns.service_ids.iter().map(|v| v.as_str()));
        refs.fare_id.extend(records.fare_attributes.iter().map(|r| r.fare_id.as_str()));
        refs.fare_id.extend(records.fare_rules.iter().map(|r| r.fare_id.as_str()));
        refs.area_id.extend(records.areas.iter().map(|r| r.area_id.as_str()));
        refs.fare_product_id.extend(records.fare_products.iter().map(|r| r.fare_product_id.as_str()));
        refs.fare_media_id.extend(records.fare_media.iter().map(|r| r.fare_media_id.as_str()));
        refs.rider_category_id.extend(records.rider_categories.iter().map(|r| r.rider_category_id.as_str()));
        refs.network_id.extend(records.networks.iter().map(|r| r.network_id.as_str()));
        refs.shape_id.extend(records.trips.iter().filter_map(|r| records.trip_interns.shape_id(r)));
        refs.shape_id.extend(records.shapes.iter().map(|r| records.shape_interns.id(r)));
        refs.zone_id.extend(records.stops.iter()
            .filter_map(|r| r.row.get("zone_id").map(String::as_str)));
        refs
    }

    fn contains(&self, field: &str, value: &str) -> bool {
        match field {
            "agency_id" => self.agency_id.contains(value),
            "stop_id" => self.stop_id.contains(value),
            "route_id" => self.route_id.contains(value),
            "trip_id" => self.trip_id.contains(value),
            "service_id" => self.service_id.contains(value),
            "fare_id" => self.fare_id.contains(value),
            "area_id" => self.area_id.contains(value),
            "fare_product_id" => self.fare_product_id.contains(value),
            "fare_media_id" => self.fare_media_id.contains(value),
            "rider_category_id" => self.rider_category_id.contains(value),
            "network_id" => self.network_id.contains(value),
            "shape_id" => self.shape_id.contains(value),
            "zone_id" | "origin_id" | "destination_id" | "contains_id" => self.zone_id.contains(value),
            _ => false,
        }
    }
}

// ── Çalışma takvimi (service_id) doldurma ─────────────────────────────────────

/// Sefer-bazlı (entity_type=Trip) notice'lara, entity_id'deki trip_id'den service_id
/// ekler — R2'deki "Çalışma Takvimi" sütunu için. Zaten dolu olanlar (OPR_003/005 gibi
/// emit'te set edilenler) korunur. Trip dışı entity'ler (Route/Feed/Stop…) atlanır.
fn fill_service_ids(mut notices: Vec<Notice>, records: &EntityRecords) -> Vec<Notice> {
    let ti = &records.trip_interns;
    let trip_service: std::collections::HashMap<&str, &str> = records.trips.iter()
        .filter(|t| !ti.service_id(t).is_empty())
        .map(|t| (t.trip_id.as_str(), ti.service_id(t)))
        .collect();
    for n in &mut notices {
        if n.service_id.is_none() && matches!(n.entity_type, EntityType::Trip) {
            if let Some(eid) = n.entity_id.as_deref() {
                if let Some(&svc) = trip_service.get(eid) {
                    n.service_id = Some(svc.to_string());
                }
            }
        }
    }
    notices
}

// ── WP-10a: Dedup ─────────────────────────────────────────────────────────────

/// Notice'lar için thread-bağımsız, kararlı toplam sıralama anahtarı.
/// Threaded K2/K6 notice'ları nondeterministik sırada üretebilir; dedup keep-first
/// ve (wasm tarafında) dedup-sonrası cap bu sıraya duyarlı olduğundan, sıralamadan
/// önce kararlı düzen "aynı feed → aynı temsilciler/aynı sayı" sağlar.
pub fn notice_order_key(n: &Notice) -> (&str, &str, &str, u64, &str, &str, &str) {
    (
        n.rule_id.as_str(),
        n.entity_type.stable_name(),
        n.entity_id.as_deref().unwrap_or(""),
        n.line.unwrap_or(0),
        n.file.as_deref().unwrap_or(""),
        n.field.as_deref().unwrap_or(""),
        n.scope_key.as_deref().unwrap_or(""),
    )
}

pub fn dedup(notices: Vec<Notice>) -> Vec<Notice> {
    // Determinizm: keep-first temsilcisi thread-bağımsız olsun diye önce kararlı sırala.
    let mut notices = notices;
    // STABLE olmak ZORUNDA: eşit anahtarlı iki bulgudan hangisinin keep-first temsilcisi
    // olacağı aksi halde girdi permütasyonuna, o da HashMap iterasyonuna kalır (nondeterminizm).
    notices.sort_by(|a, b| notice_order_key(a).cmp(&notice_order_key(b)));
    let mut seen: FxHashSet<String> = FxHashSet::default();
    seen.reserve(notices.len());
    let mut out: Vec<Notice> = Vec::with_capacity(notices.len());
    // Reusable buffer: format!() yerine push_str/write! — per-notice String alloc kaldırıldı
    let mut key_buf = String::with_capacity(128);
    for n in notices {
        let level = get_rule(&n.rule_id)
            .map(|m| m.dedup_level)
            .unwrap_or(DedupLevel::Row);
        key_buf.clear();
        write_dedup_key(&mut key_buf, &n, level);
        if !seen.contains(key_buf.as_str()) {
            seen.insert(key_buf.clone());
            out.push(n);
        }
    }
    out
}

/// Notice'ları kararlı biçimde dedup ederken kural başına yalnızca gösterilecek
/// temsilcileri saklar. Böylece milyonlarca dedup edilmiş Notice için ikinci bir
/// büyük Vec ayrılmaz; `totals` yine cap öncesindeki gerçek distinct sayıları taşır.
pub fn dedup_and_cap_by_rule<F>(
    mut notices: Vec<Notice>,
    cap_for_rule: F,
) -> (Vec<Notice>, HashMap<String, u32>)
where
    F: Fn(&str) -> usize,
{
    // STABLE olmak ZORUNDA: eşit anahtarlı iki bulgudan hangisinin keep-first temsilcisi
    // olacağı aksi halde girdi permütasyonuna, o da HashMap iterasyonuna kalır (nondeterminizm).
    notices.sort_by(|a, b| notice_order_key(a).cmp(&notice_order_key(b)));

    let mut seen: FxHashSet<String> = FxHashSet::default();
    seen.reserve(notices.len());
    let mut kept = Vec::new();
    let mut totals: HashMap<String, u32> = HashMap::new();
    let mut key_buf = String::with_capacity(128);

    for notice in notices {
        let level = get_rule(&notice.rule_id)
            .map(|m| m.dedup_level)
            .unwrap_or(DedupLevel::Row);
        key_buf.clear();
        write_dedup_key(&mut key_buf, &notice, level);
        if !seen.insert(key_buf.clone()) {
            continue;
        }

        let total = totals.entry(notice.rule_id.clone()).or_insert(0);
        *total += 1;
        if (*total as usize) <= cap_for_rule(&notice.rule_id) {
            kept.push(notice);
        }
    }

    (kept, totals)
}

// Separator olarak \x00 kullanılır — GTFS ID'leri, dosya adları ve alan adları
// null byte içeremez; bu sayede entity_id/file/field değerlerindeki '|' gibi
// karakterlerden kaynaklanan çakışmalar önlenir.
fn write_dedup_key(buf: &mut String, n: &Notice, level: DedupLevel) {
    const SEP: char = '\x00';
    match level {
        DedupLevel::Feed => buf.push_str(&n.rule_id),

        DedupLevel::File => {
            buf.push_str(&n.rule_id);
            buf.push(SEP);
            buf.push_str(n.file.as_deref().unwrap_or(""));
        }

        DedupLevel::Entity => {
            buf.push_str(&n.rule_id);
            buf.push(SEP);
            buf.push_str(n.entity_type.stable_name());
            buf.push(SEP);
            buf.push_str(n.entity_id.as_deref().unwrap_or(""));
        }

        DedupLevel::Row => match n.file.as_deref() {
            Some(f) => {
                buf.push_str(&n.rule_id);
                buf.push(SEP);
                buf.push_str(f);
                buf.push(SEP);
                use std::fmt::Write as _;
                let _ = write!(buf, "{}", n.line.unwrap_or(0));
            }
            None => {
                buf.push_str(&n.rule_id);
                buf.push_str("\x00id:");
                buf.push_str(&n.id);
            }
        },

        DedupLevel::Field => match n.file.as_deref() {
            Some(f) => {
                buf.push_str(&n.rule_id);
                buf.push(SEP);
                buf.push_str(f);
                buf.push(SEP);
                use std::fmt::Write as _;
                let _ = write!(buf, "{}", n.line.unwrap_or(0));
                buf.push(SEP);
                buf.push_str(n.field.as_deref().unwrap_or(""));
            }
            None => {
                buf.push_str(&n.rule_id);
                buf.push_str("\x00id:");
                buf.push_str(&n.id);
            }
        },
    }
}

// ── WP-10b: Root Cause / Semptom Çözümü ──────────────────────────────────────

struct SymptomResolution {
    /// notices[i] bir semptom mu?
    is_symptom: Vec<bool>,
    /// rule_id → [(notice_idx, scope_key)] — realized_dep hesabı için
    rule_scope_index: HashMap<String, Vec<(usize, Option<String>)>>,
}

fn resolve_symptoms(notices: &[Notice]) -> SymptomResolution {
    let mut rule_scope_index: HashMap<String, Vec<(usize, Option<String>)>> = HashMap::new();
    for (i, n) in notices.iter().enumerate() {
        rule_scope_index
            .entry(n.rule_id.clone())
            .or_default()
            .push((i, n.scope_key.clone()));
    }

    let mut is_symptom = vec![false; notices.len()];
    for root in notices.iter() {
        for blocked_rule in &root.blocks {
            if let Some(candidates) = rule_scope_index.get(blocked_rule) {
                for (ci, scope) in candidates {
                    // None root → tüm feed'i etkiler, tüm scope'ları bastırır.
                    // Some(x) root → yalnızca aynı scope'u bastırır.
                    let matches = root.scope_key.is_none()
                        || scope.as_deref() == root.scope_key.as_deref();
                    if matches {
                        is_symptom[*ci] = true;
                    }
                }
            }
        }
    }

    SymptomResolution { is_symptom, rule_scope_index }
}

/// Blocker-eligible notice'ların rule başına gruplanmış yayın penaltısı ve
/// rule → katkı haritasını döndürür.
///
/// Tram gibi sabit raylı hatlarda aynı shape_id'den gelen tüm sefer/durak
/// hataları aynı fiziksel sorundan kaynaklanır; her instance'ı bağımsız
/// saymak pub_penalty'yi yapay şişirir.  instance_multiplier (max 2×) ile
/// kapsama alınır — fix_effort hesabıyla tutarlı.
fn build_pub_penalty_map(notices: &[Notice]) -> (f64, HashMap<String, f64>) {
    let mut rule_data: HashMap<&str, (f64, u32)> = HashMap::new();
    for n in notices.iter().filter(|n| is_pub_relevant(n)) {
        let e = rule_data.entry(n.rule_id.as_str()).or_insert((n.severity.weight(), 0));
        e.1 += 1;
    }
    let mut per_rule: HashMap<String, f64> = HashMap::new();
    let mut total = 0.0_f64;
    for (id, &(w, cnt)) in &rule_data {
        let contrib = w * instance_multiplier(cnt);
        per_rule.insert(id.to_string(), contrib);
        total += contrib;
    }
    (total, per_rule)
}

/// Bir notice grubunun realized_dependent_count'u: grup içindeki herhangi bir
/// notice'ın scope_key'iyle eşleşen en az 1 notice üretmiş blocks[] kural sayısı.
fn realized_dep_for_group(
    group_indices: &[usize],
    notices: &[Notice],
    rule_scope_index: &HashMap<String, Vec<(usize, Option<String>)>>,
) -> u32 {
    let all_blocks: FxHashSet<&str> = group_indices
        .iter()
        .flat_map(|&i| notices[i].blocks.iter().map(String::as_str))
        .collect();

    all_blocks
        .iter()
        .filter(|&&blocked_rule| {
            rule_scope_index
                .get(blocked_rule)
                .map(|candidates| {
                    group_indices.iter().any(|&gi| {
                        let root_scope = notices[gi].scope_key.as_deref();
                        // None root → tüm feed'i etkiler (resolve_symptoms ile aynı kural)
                        candidates.iter().any(|(_, scope)| {
                            root_scope.is_none() || scope.as_deref() == root_scope
                        })
                    })
                })
                .unwrap_or(false)
        })
        .count() as u32
}

/// R9 item'ı seçildiğinde kapanacak tüm notice index'lerini getirir (transitif closure).
///
/// Başlangıç kümesi `group_indices` (root cause notice'ları).  Her notice'ın
/// `blocks[]` listesindeki kurallar `rule_scope_index` üzerinden aranır; scope
/// eşleşmesi sağlayan blocked notice'lar kümeye eklenir ve bu işlem sabit noktaya
/// ulaşana dek (BFS ile) tekrar edilir.
///
/// Scope kuralı (resolve_symptoms ile aynı):
/// - root.scope_key == None → tüm feed'i etkiler; her scope_key eşleşir.
/// - root.scope_key == Some(x) → yalnızca aynı Some(x)'e sahip candidate eşleşir.
///
/// Döndürülen set hem root notice'ları hem de kapanan downstream notice'ları içerir.
fn closure_notice_indices_for_group(
    group_indices: &[usize],
    notices: &[Notice],
    resolution: &SymptomResolution,
) -> FxHashSet<usize> {
    let mut closure: FxHashSet<usize> = FxHashSet::default();
    let mut queue: Vec<usize> = Vec::new();

    // Başlangıç kümesi: root cause notice'lar
    for &i in group_indices {
        if closure.insert(i) {
            queue.push(i);
        }
    }

    // BFS: her notice'ın blocks[] zincirini scope-aware olarak genişlet
    let mut head = 0;
    while head < queue.len() {
        let idx = queue[head];
        head += 1;
        let root_notice = &notices[idx];
        let root_scope = root_notice.scope_key.as_deref();

        for blocked_rule in &root_notice.blocks {
            if let Some(candidates) = resolution.rule_scope_index.get(blocked_rule) {
                for &(ci, ref scope) in candidates {
                    if closure.contains(&ci) {
                        continue;
                    }
                    // Scope eşleşme kuralı
                    let matches = root_scope.is_none()
                        || scope.as_deref() == root_scope;
                    if matches {
                        closure.insert(ci);
                        queue.push(ci);
                    }
                }
            }
        }
    }

    closure
}

/// Closure set içindeki notice'ların gruplandırılmış yayın penaltısını hesaplar.
///
/// `build_pub_penalty_map` ile aynı hiperbolik-decay + instance_multiplier mantığını
/// kullanır; tek fark tüm notice yerine yalnızca `closure_indices` kümesindekiler
/// işlenir.  Closure set'indeki her rule_id için instance sayısı ayrı toplanır;
/// bu sayede birden fazla closure notice'ı olan rule'lar da doğru gruplandırılır.
fn build_closure_pub_penalty(
    closure_indices: &FxHashSet<usize>,
    notices: &[Notice],
) -> f64 {
    let mut rule_data: HashMap<&str, (f64, u32)> = HashMap::new();
    for &i in closure_indices {
        let n = &notices[i];
        if !is_pub_relevant(n) {
            continue;
        }
        let e = rule_data.entry(n.rule_id.as_str()).or_insert((n.severity.weight(), 0));
        e.1 += 1;
    }
    let mut total = 0.0_f64;
    for &(w, cnt) in rule_data.values() {
        total += w * instance_multiplier(cnt);
    }
    total
}

// ── WP-10c: R9 Priority Score + Bucket Sıralaması ────────────────────────────

fn instance_multiplier(count: u32) -> f64 {
    if count <= 5 {
        1.0
    } else if count <= 50 {
        1.5
    } else {
        2.0
    }
}

fn compute_priority_score(severity: Severity, realized_dep: u32, affected: u32, fix_effort: f64) -> f64 {
    let sw = severity.weight();
    let raw = sw * (1.0 + realized_dep as f64) * (1.0 + affected as f64).log2() / fix_effort;
    (raw * 100.0).round() / 100.0
}

fn r9_labels(severity: Severity, rule_class: RuleClass, realized_dep: u32, fix_effort: f64, affected: u32) -> Vec<R9Label> {
    let mut labels = vec![];

    // R1 ile hizalı: yalnız Spec+Kritik gerçek yayın-engeli (Blocker). Interop her
    // severity'de tüketici uyumluluk sinyali (Interop), yayın-engeli değil.
    match (severity, rule_class) {
        (Severity::Kritik, RuleClass::Spec) => {
            labels.push(R9Label::Blocker);
        }
        (_, RuleClass::Interop) => {
            labels.push(R9Label::Interop);
        }
        (_, RuleClass::Analytics) => {
            labels.push(R9Label::Analytics);
        }
        _ => {}
    }
    if realized_dep > 2 {
        labels.push(R9Label::Propagation);
    }
    // Kolay Düzeltme: düşük çaba, Bilgi hariç her önem
    if fix_effort <= 1.5 && !matches!(severity, Severity::Bilgi) {
        labels.push(R9Label::QuickWin);
    }
    // Kalite Öncelikli: Quality sınıfı, Orta veya üzeri
    if matches!(rule_class, RuleClass::Quality) && matches!(severity, Severity::Kritik | Severity::Yuksek | Severity::Orta) {
        labels.push(R9Label::Quality);
    }
    // Yaygın: 20'den fazla etkilenen instance
    if affected > 20 {
        labels.push(R9Label::Widespread);
    }
    // Tek Örnek: tam olarak 1 instance — kolay bulunur
    if affected == 1 {
        labels.push(R9Label::Single);
    }
    // Zor: yüksek çaba gerektiren düzeltme
    if fix_effort >= 4.0 {
        labels.push(R9Label::Hard);
    }
    labels
}

fn bucket_rank(labels: &[R9Label]) -> u8 {
    if labels.contains(&R9Label::Blocker) {
        0
    } else if labels.contains(&R9Label::Interop) {
        2
    } else if labels.contains(&R9Label::Propagation) {
        3
    } else if labels.contains(&R9Label::QuickWin) {
        4
    } else if labels.contains(&R9Label::Quality) {
        5
    } else if labels.contains(&R9Label::Widespread) {
        6
    } else {
        7
    }
}

fn build_r9(notices: &[Notice], resolution: &SymptomResolution) -> R9Report {
    // Sınıf başına toplam penaltı (R5 score_delta hesabı için)
    let mut class_penalty: HashMap<RuleClass, f64> = HashMap::new();
    for n in notices {
        *class_penalty.entry(n.rule_class).or_default() += n.severity.weight();
    }

    // Toplam yayın penaltısı (pub_score_delta için referans baz değer)
    let (total_pub_penalty, _) = build_pub_penalty_map(notices);

    // Sadece kök neden notice'lar R9'a girer; semptomlar gizlenir
    let mut groups: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, n) in notices.iter().enumerate() {
        if !resolution.is_symptom[i] {
            groups.entry(n.rule_id.as_str()).or_default().push(i);
        }
    }

    let mut items: Vec<R9Item> = groups
        .into_iter()
        .map(|(rule_id, indices)| {
            let first = &notices[indices[0]];
            let severity = first.severity;
            let rule_class = get_rule(rule_id)
                .map(|m| m.rule_class)
                .unwrap_or(first.rule_class);
            let base_effort = get_rule(rule_id)
                .map(|m| m.base_effort)
                .unwrap_or(first.base_effort);
            let affected = indices.len() as u32;
            let multiplier = instance_multiplier(affected);
            let fix_effort = base_effort as f64 * multiplier;
            let realized_dep =
                realized_dep_for_group(&indices, notices, &resolution.rule_scope_index);
            let mut labels = r9_labels(severity, rule_class, realized_dep, fix_effort, affected);
            let priority_score =
                compute_priority_score(severity, realized_dep, affected, fix_effort);

            // Closure set: bu R9 item seçilince kapanacak tüm notice'lar (root + downstream)
            let closure = closure_notice_indices_for_group(&indices, notices, resolution);

            // R5 genel skor iyileştirmesi — closure set üzerinden sınıf başına penaltı düşümü.
            // Closure set içindeki notice'lar sınıfa göre gruplanır; her sınıf için toplam
            // closure penaltısı bir kerede çıkarılarak delta hesaplanır. Bu yöntem aynı sınıftan
            // birden fazla closure notice olduğunda da doğru sonuç verir.
            let k = 50.0_f64;
            let score_delta = {
                // Sınıf → closure penaltısı toplamı
                let mut closure_class_penalty: HashMap<RuleClass, f64> = HashMap::new();
                for &ci in &closure {
                    let cn = &notices[ci];
                    *closure_class_penalty.entry(cn.rule_class).or_default() += cn.severity.weight();
                }
                let mut delta_sum = 0.0_f64;
                for (cn_class, cp_removed) in &closure_class_penalty {
                    let cn_weight = match cn_class {
                        RuleClass::Spec      => 0.4,
                        RuleClass::Interop   => 0.3,
                        RuleClass::Quality   => 0.2,
                        RuleClass::Analytics => 0.1,
                    };
                    let cur_cp = class_penalty.get(cn_class).copied().unwrap_or(0.0);
                    let cur_cs = k * 100.0 / (k + cur_cp);
                    let new_cs = k * 100.0 / (k + (cur_cp - cp_removed).max(0.0));
                    delta_sum += cn_weight * (new_cs - cur_cs);
                }
                round1(delta_sum)
            };

            // R5 yayın skoru iyileştirmesi — closure set'indeki pub-relevant notice'ların
            // gruplandırılmış hiperbolik-decay penaltısı kullanılır (build_pub_penalty_map ile tutarlı).
            let closure_pub_penalty = build_closure_pub_penalty(&closure, notices);
            let cur_pub = k * 100.0 / (k + total_pub_penalty);
            let new_pub = k * 100.0 / (k + (total_pub_penalty - closure_pub_penalty).max(0.0));
            let pub_score_delta = round1(new_pub - cur_pub);

            // Yüksek Etki: skor katkısı belirgin eşiğin üzerinde
            if score_delta >= 5.0 || pub_score_delta >= 3.0 {
                labels.push(R9Label::HighImpact);
            }

            R9Item {
                rule_id: rule_id.to_string(),
                labels,
                priority_score,
                score_delta,
                pub_score_delta,
                affected_instance_count: affected,
                realized_dependent_count: realized_dep,
                base_effort,
                fix_effort,
                notice_ids: indices.iter().map(|&i| notices[i].id.clone()).collect(),
            }
        })
        .collect();

    // Birincil: bucket rank; ikincil: priority_score azalan;
    // üçüncül: rule_id (R9'da benzersiz) — eşit skorlarda HashMap iterasyon
    // sırasından bağımsız, deterministik sıra garanti eder.
    items.sort_by(|a, b| {
        bucket_rank(&a.labels)
            .cmp(&bucket_rank(&b.labels))
            .then(
                b.priority_score
                    .partial_cmp(&a.priority_score)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then_with(|| a.rule_id.cmp(&b.rule_id))
    });

    R9Report { items }
}

// ── WP-10d: Rapor Projeksiyonları (R1–R8) ────────────────────────────────────

fn build_report_set(
    notices: &[Notice],
    resolution: &SymptomResolution,
    coverage_complete: bool,
) -> ReportSet {
    ReportSet {
        r1: build_r1(notices, coverage_complete),
        r2: build_r2(notices),
        r3: build_r3(notices),
        r4: build_r4(notices),
        r5: build_r5(notices),
        r7: build_r7(notices),
        r8: build_r8(notices),
        r9: build_r9(notices, resolution),
    }
}

/// R1 yayın kapısı.
///
/// 🔴 **`publishable` ARTIK `blockers.is_empty()` DEĞİL** (issue #133). Eski hâli, zorunlu
/// bir dosya okunamadığı için o dosyanın kuralları HİÇ koşmamışken bile `true` diyordu:
/// ölçüldü (`mdb-1903`), `stops.txt` çözülemez hâle getirilince 29 kontrol atlandı ve feed
/// yine `publishable=true` kaldı. Bulgu yokluğu, kanıt yokluğuyla aynı şey değildir.
///
/// ⚠️ `coverage_complete` YALNIZ zorunlu dosya kaybında düşer. Bozuk bir `attributions.txt`
/// da koşumu PARTIAL yapar (ölçüldü) ama kapıyı düşürmez — geçerli feed'i yayından men
/// etmek, normu tavsiye saymaktan çok daha ağır bir hatadır (`PTH_017` asimetrisi).
fn build_r1(notices: &[Notice], coverage_complete: bool) -> R1Report {
    let blockers: Vec<String> = notices
        .iter()
        .filter(|n| is_pub_relevant(n))
        .map(|n| n.id.clone())
        .collect();
    R1Report {
        publishable: blockers.is_empty() && coverage_complete,
        coverage_complete,
        blocker_notice_ids: blockers,
    }
}

fn build_r2(notices: &[Notice]) -> R2Report {
    R2Report {
        items: notices
            .iter()
            .map(|n| ReportItem {
                notice_id: n.id.clone(),
                report_id: ReportId::R2,
                display_label: n.rule_id.clone(),
                is_primary: true,
            })
            .collect(),
    }
}

fn build_r3(notices: &[Notice]) -> R3Report {
    R3Report {
        items: notices
            .iter()
            .filter(|n| n.rule_class == RuleClass::Analytics)
            .map(|n| ReportItem {
                notice_id: n.id.clone(),
                report_id: ReportId::R3,
                display_label: n.rule_id.clone(),
                is_primary: true,
            })
            .collect(),
    }
}

fn build_r4(notices: &[Notice]) -> R4Report {
    R4Report {
        items: notices
            .iter()
            .filter(|n| in_report(&n.rule_id, ReportId::R4))
            .map(|n| ReportItem {
                notice_id: n.id.clone(),
                report_id: ReportId::R4,
                display_label: r4_display_label(&n.rule_id),
                is_primary: true,
            })
            .collect(),
    }
}

/// Yayın-engeli notice: yalnız `Spec + Kritik` (resmi GTFS spec kapısı).
/// R1 yayın kararını ve pub_score/pub_score_delta'yı bu belirler.
/// `rule_class==Spec` ⟺ `authority==GtfsSpec` registry gate'iyle
/// (`spec_class_requires_gtfs_spec_authority`) garanti olduğundan runtime otorite kontrolü
/// gereksiz. Interop/Quality/Analytics ASLA yayın-engeli değil — tüketici/interop hazırlığı
/// R8 + interop_score'da raporlanır.
fn is_pub_relevant(n: &Notice) -> bool {
    matches!(n.severity, Severity::Kritik) && n.rule_class == RuleClass::Spec
}

fn build_r5(notices: &[Notice]) -> R5Report {
    let spec_score      = class_score(notices, RuleClass::Spec);
    let interop_score   = class_score(notices, RuleClass::Interop);
    let quality_score   = class_score(notices, RuleClass::Quality);
    let analytics_score = class_score(notices, RuleClass::Analytics);
    // Ağırlıklı ortalama: SPEC %40, INTEROP %30, QUALITY %20, ANALYTICS %10
    let overall = 0.4 * spec_score
        + 0.3 * interop_score
        + 0.2 * quality_score
        + 0.1 * analytics_score;
    // Yayın skoru: rule başına gruplandırılmış penaltı (build_pub_penalty_map ile tutarlı)
    let (pub_penalty, _) = build_pub_penalty_map(notices);
    let pub_score = 100.0 * 50.0 / (50.0 + pub_penalty);
    R5Report {
        score:           round1(overall),
        pub_score:       round1(pub_score),
        spec_score:      round1(spec_score),
        interop_score:   round1(interop_score),
        quality_score:   round1(quality_score),
        analytics_score: round1(analytics_score),
    }
}

/// Hiperbolik azalma: score = 100 × K / (K + penalty), K=50.
/// Her kural başına instance_multiplier (maks 2×) uygulanır — tek yüksek
/// sayımlı kural tüm skoru çökertmez. build_pub_penalty_map ile tutarlı.
fn class_score(notices: &[Notice], class: RuleClass) -> f64 {
    let mut per_rule: HashMap<&str, (f64, u32)> = HashMap::new();
    for n in notices.iter().filter(|n| n.rule_class == class) {
        let e = per_rule.entry(n.rule_id.as_str()).or_insert((n.severity.weight(), 0));
        e.1 += 1;
    }
    let penalty: f64 = per_rule.values()
        .map(|(w, cnt)| w * instance_multiplier(*cnt))
        .sum();
    100.0 * 50.0 / (50.0 + penalty)
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn build_r7(notices: &[Notice]) -> R7Report {
    R7Report {
        items: notices
            .iter()
            .filter(|n| in_report(&n.rule_id, ReportId::R7))
            .map(|n| ReportItem {
                notice_id: n.id.clone(),
                report_id: ReportId::R7,
                display_label: n.rule_id.clone(),
                is_primary: true,
            })
            .collect(),
    }
}

fn build_r8(notices: &[Notice]) -> R8Report {
    R8Report {
        items: notices
            .iter()
            .filter(|n| n.rule_class == RuleClass::Interop)
            .map(|n| ReportItem {
                notice_id: n.id.clone(),
                report_id: ReportId::R8,
                display_label: n.rule_id.clone(),
                is_primary: true,
            })
            .collect(),
    }
}

fn in_report(rule_id: &str, report_id: ReportId) -> bool {
    get_rule(rule_id)
        .map(|m| m.report_views.contains(&report_id))
        .unwrap_or(false)
}

fn r4_display_label(rule_id: &str) -> String {
    match rule_id {
        "SHP_014" => "GEO_010".to_string(),
        "SHP_016" => "GEO_011".to_string(),
        _ => rule_id.to_string(),
    }
}

// ── Metrikler ─────────────────────────────────────────────────────────────────

fn build_metrics(notices: &[Notice], records: &EntityRecords, derived: &DerivedData, file_stats: Vec<FileInfo>, coverage_complete: bool) -> FeedMetrics {
    let shape_count = records
        .shapes
        .iter()
        .map(|s| records.shape_interns.id(s))
        .collect::<FxHashSet<_>>()
        .len() as u32;

    let all_dates: FxHashSet<u32> = derived
        .calendar_bitmap
        .active_dates
        .values()
        .flat_map(|dates| dates.iter().copied())
        .collect();
    let active_service_days = all_dates.len() as u32;

    // Gerçek servis kapsama aralığı (YYYYMMDD min/max). all_dates değerleri zaten YYYYMMDD.
    let service_start_date = all_dates.iter().min().copied();
    let service_end_date = all_dates.iter().max().copied();

    // Feed geçerlilik penceresi (feed_info.txt) — (y,m,d) tuple → YYYYMMDD u32.
    let yyyymmdd = |(y, m, d): (u32, u32, u32)| y * 10000 + m * 100 + d;
    let feed_info = records.feed_info.first();
    let feed_start_date = feed_info.and_then(|fi| fi.feed_start_date).map(yyyymmdd);
    let feed_end_date = feed_info.and_then(|fi| fi.feed_end_date).map(yyyymmdd);

    // Benzersiz, boş olmayan trip_id → service_id; duplicate/empty satırlar metriğe yansımasın.
    let ti_k7 = &records.trip_interns;
    let mut unique_trips: HashMap<&str, &str> = HashMap::with_capacity(records.trips.len());
    for t in &records.trips {
        if !t.trip_id.is_empty() {
            unique_trips.entry(t.trip_id.as_str()).or_insert(ti_k7.service_id(t));
        }
    }

    // Frekans bazlı trip'lerin günlük run sayısını hesapla (floor((end-start)/headway))
    let mut freq_runs: HashMap<&str, u32> = HashMap::new();
    for freq in &records.frequencies {
        if freq.trip_id.is_empty() { continue; }
        let (Some(start), Some(end), Some(hw)) = (freq.start_time, freq.end_time, freq.headway_secs) else { continue };
        if hw == 0 { continue; }
        let start_s = start.0 * 3600 + start.1 * 60 + start.2;
        let end_s   = end.0   * 3600 + end.1   * 60 + end.2;
        if end_s <= start_s { continue; }
        *freq_runs.entry(freq.trip_id.as_str()).or_insert(0) += (end_s - start_s) / hw;
    }

    // trip_count: frekans trip'leri günlük run sayısıyla genişletilir
    let trip_count: u32 = unique_trips.keys()
        .map(|&tid| freq_runs.get(tid).copied().unwrap_or(1).max(1))
        .sum();

    // Her benzersiz trip'in service_id'si kaç gün aktifse, frekans çarpanıyla birlikte sayılır.
    let total_trip_days: usize = unique_trips.iter()
        .map(|(&tid, &svc)| {
            let active = derived.calendar_bitmap.active_dates
                .get(svc)
                .map(|dates| dates.len())
                .unwrap_or(0);
            let multiplier = freq_runs.get(tid).copied().unwrap_or(1).max(1) as usize;
            active * multiplier
        })
        .sum();
    let avg_daily_trips = if active_service_days > 0 {
        total_trip_days as f64 / active_service_days as f64
    } else {
        0.0
    };

    // GTFS-JP profili (geniş tespit): _jp uzantı dosyaları OPSİYONEL olduğundan, bir
    // Japon feed'i onlarsız da GTFS-JP konvansiyonlarını izleyebilir (örn. osaka-tokushima:
    // _jp dosyası yok ama feed_lang=ja + kana çevirileri var). Üç sinyalden biri yeterli:
    //   (a) herhangi bir *_jp.txt dosyası, (b) feed_lang ja* ile başlıyor,
    //   (c) translations'ta kana okuması (language=ja-Hrkt).
    let is_gtfs_jp = file_stats.iter()
        .any(|f| matches!(f.name.as_str(), "agency_jp.txt" | "routes_jp.txt" | "office_jp.txt"))
        || records.feed_info.first()
            .map(|fi| fi.feed_lang.to_lowercase().starts_with("ja"))
            .unwrap_or(false)
        || records.translations.iter()
            .any(|t| t.language.eq_ignore_ascii_case("ja-Hrkt"));

    FeedMetrics {
        coverage_complete,
        stop_count:   records.stops.len() as u32,
        route_count:  records.routes.len() as u32,
        trip_count,
        shape_count,
        active_service_days,
        avg_daily_trips,
        feed_start_date,
        feed_end_date,
        service_start_date,
        service_end_date,
        spec_notice_count:      notices.iter().filter(|n| n.rule_class == RuleClass::Spec).count() as u32,
        interop_notice_count:   notices.iter().filter(|n| n.rule_class == RuleClass::Interop).count() as u32,
        quality_notice_count:   notices.iter().filter(|n| n.rule_class == RuleClass::Quality).count() as u32,
        analytics_notice_count: notices.iter().filter(|n| n.rule_class == RuleClass::Analytics).count() as u32,
        overall_score: 0.0, // report() fonksiyonunda r5.score ile güncellenir
        file_stats,
        is_gtfs_jp,
    }
}

// ── WP-10e: Testler ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use gtfs_core::EntityType;

    fn notice(id: &str, rule_id: &str, severity: Severity, class: RuleClass) -> Notice {
        Notice {
            id: id.to_string(),
            rule_id: rule_id.to_string(),
            severity,
            rule_class: class,
            entity_type: EntityType::Feed,
            entity_id: None,
            scope_key: None,
            file: None,
            line: None,
            field: None,
            observed_value: None,
            expected_value: None,
            details: None,
            title: String::new(),
            message: String::new(),
            remediation: String::new(),
            blocks: vec![],
            base_effort: 1,
            service_id: None,
        }
    }

    // ── Dedup ─────────────────────────────────────────────────────────────────

    // ARC_001 → Feed dedup
    #[test]
    fn dedup_feed_level_keeps_first() {
        let mut n1 = notice("n1", "ARC_001", Severity::Kritik, RuleClass::Spec);
        let mut n2 = notice("n2", "ARC_001", Severity::Kritik, RuleClass::Spec);
        n1.file = Some("agency.txt".into());
        n2.file = Some("stops.txt".into());
        assert_eq!(dedup(vec![n1, n2]).len(), 1);
    }

    // ARC_002 → File dedup: farklı dosya → iki notice
    #[test]
    fn dedup_file_level_different_files() {
        let mut n1 = notice("n1", "ARC_002", Severity::Kritik, RuleClass::Spec);
        let mut n2 = notice("n2", "ARC_002", Severity::Kritik, RuleClass::Spec);
        n1.file = Some("agency.txt".into());
        n2.file = Some("stops.txt".into());
        assert_eq!(dedup(vec![n1, n2]).len(), 2);
    }

    // ARC_002 → File dedup: aynı dosya → tek notice
    #[test]
    fn dedup_file_level_same_file() {
        let mut n1 = notice("n1", "ARC_002", Severity::Kritik, RuleClass::Spec);
        let mut n2 = notice("n2", "ARC_002", Severity::Kritik, RuleClass::Spec);
        n1.file = Some("stops.txt".into());
        n2.file = Some("stops.txt".into());
        assert_eq!(dedup(vec![n1, n2]).len(), 1);
    }

    // AGN_002 → Entity dedup
    #[test]
    fn dedup_entity_level_same_entity() {
        let mut n1 = notice("n1", "AGN_002", Severity::Kritik, RuleClass::Spec);
        let mut n2 = notice("n2", "AGN_002", Severity::Kritik, RuleClass::Spec);
        n1.entity_type = EntityType::Agency;
        n2.entity_type = EntityType::Agency;
        n1.entity_id = Some("A1".into());
        n2.entity_id = Some("A1".into());
        assert_eq!(dedup(vec![n1, n2]).len(), 1);
    }

    #[test]
    fn dedup_entity_level_different_entities() {
        let mut n1 = notice("n1", "AGN_002", Severity::Kritik, RuleClass::Spec);
        let mut n2 = notice("n2", "AGN_002", Severity::Kritik, RuleClass::Spec);
        n1.entity_id = Some("A1".into());
        n2.entity_id = Some("A2".into());
        assert_eq!(dedup(vec![n1, n2]).len(), 2);
    }

    // ARC_018 → Row dedup
    #[test]
    fn dedup_row_level_same_row() {
        let mut n1 = notice("n1", "ARC_018", Severity::Yuksek, RuleClass::Spec);
        let mut n2 = notice("n2", "ARC_018", Severity::Yuksek, RuleClass::Spec);
        n1.file = Some("stops.txt".into()); n1.line = Some(5);
        n2.file = Some("stops.txt".into()); n2.line = Some(5);
        assert_eq!(dedup(vec![n1, n2]).len(), 1);
    }

    #[test]
    fn dedup_row_level_different_rows() {
        let mut n1 = notice("n1", "ARC_018", Severity::Yuksek, RuleClass::Spec);
        let mut n2 = notice("n2", "ARC_018", Severity::Yuksek, RuleClass::Spec);
        n1.file = Some("stops.txt".into()); n1.line = Some(5);
        n2.file = Some("stops.txt".into()); n2.line = Some(6);
        assert_eq!(dedup(vec![n1, n2]).len(), 2);
    }

    #[test]
    fn dedup_and_cap_keeps_real_totals_without_materializing_all_notices() {
        let mut notices = Vec::new();
        for line in [4, 2, 3, 2, 1] {
            let mut n = notice(
                &format!("n{line}"),
                "ARC_018",
                Severity::Yuksek,
                RuleClass::Spec,
            );
            n.file = Some("stops.txt".into());
            n.line = Some(line);
            notices.push(n);
        }

        let (kept, totals) = dedup_and_cap_by_rule(notices, |_| 2);

        assert_eq!(totals.get("ARC_018"), Some(&4));
        assert_eq!(kept.len(), 2);
        assert_eq!(kept.iter().map(|n| n.line).collect::<Vec<_>>(), vec![Some(1), Some(2)]);
    }

    // ARC_014 → Field dedup
    #[test]
    fn dedup_field_level_same_field() {
        let mut n1 = notice("n1", "ARC_014", Severity::Orta, RuleClass::Quality);
        let mut n2 = notice("n2", "ARC_014", Severity::Orta, RuleClass::Quality);
        n1.file = Some("stops.txt".into()); n1.line = Some(3); n1.field = Some("stop_name".into());
        n2.file = Some("stops.txt".into()); n2.line = Some(3); n2.field = Some("stop_name".into());
        assert_eq!(dedup(vec![n1, n2]).len(), 1);
    }

    #[test]
    fn dedup_field_level_different_fields() {
        let mut n1 = notice("n1", "ARC_014", Severity::Orta, RuleClass::Quality);
        let mut n2 = notice("n2", "ARC_014", Severity::Orta, RuleClass::Quality);
        n1.file = Some("stops.txt".into()); n1.line = Some(3); n1.field = Some("stop_name".into());
        n2.file = Some("stops.txt".into()); n2.line = Some(3); n2.field = Some("stop_lat".into());
        assert_eq!(dedup(vec![n1, n2]).len(), 2);
    }

    #[test]
    fn dedup_unknown_rule_falls_back_to_row() {
        let mut n1 = notice("n1", "UNKNOWN_999", Severity::Orta, RuleClass::Quality);
        let mut n2 = notice("n2", "UNKNOWN_999", Severity::Orta, RuleClass::Quality);
        n1.file = Some("stops.txt".into()); n1.line = Some(1);
        n2.file = Some("stops.txt".into()); n2.line = Some(1);
        assert_eq!(dedup(vec![n1, n2]).len(), 1);
    }

    // Feed dedup: entity_id dolu olsa bile yalnızca rule_id önemli
    #[test]
    fn dedup_feed_ignores_entity_id() {
        let mut n1 = notice("n1", "ARC_001", Severity::Kritik, RuleClass::Spec);
        let mut n2 = notice("n2", "ARC_001", Severity::Kritik, RuleClass::Spec);
        n1.entity_id = Some("A1".into());
        n2.entity_id = Some("A2".into()); // farklı entity_id ama Feed dedup → tek notice
        assert_eq!(dedup(vec![n1, n2]).len(), 1);
    }

    // Entity dedup: aynı entity_id ama farklı entity_type → iki ayrı notice
    #[test]
    fn dedup_entity_different_types_not_merged() {
        let mut n1 = notice("n1", "AGN_002", Severity::Kritik, RuleClass::Spec);
        let mut n2 = notice("n2", "AGN_002", Severity::Kritik, RuleClass::Spec);
        n1.entity_type = EntityType::Agency; n1.entity_id = Some("X".into());
        n2.entity_type = EntityType::Stop;   n2.entity_id = Some("X".into()); // aynı id, farklı tip
        assert_eq!(dedup(vec![n1, n2]).len(), 2);
    }

    // Row dedup: file=None → dedup yapılmamalı, her notice tekil kalmalı
    #[test]
    fn dedup_row_none_file_not_merged() {
        let n1 = notice("n1", "ARC_018", Severity::Yuksek, RuleClass::Spec);
        let n2 = notice("n2", "ARC_018", Severity::Yuksek, RuleClass::Spec);
        // file=None, line=None → tekil key
        assert_eq!(dedup(vec![n1, n2]).len(), 2, "file=None → dedup yapılmamalı");
    }

    // ── Semptom çözümü ────────────────────────────────────────────────────────

    #[test]
    fn symptom_resolution_marks_blocked_rule_with_same_scope() {
        let mut root = notice("root", "STP_001", Severity::Kritik, RuleClass::Spec);
        root.scope_key = Some("S1".into());
        root.blocks = vec!["GEO_002".to_string()];

        let mut symptom = notice("sym", "GEO_002", Severity::Orta, RuleClass::Analytics);
        symptom.scope_key = Some("S1".into());

        let resolution = resolve_symptoms(&[root, symptom]);
        assert!(!resolution.is_symptom[0], "kök neden semptom olmamalı");
        assert!(resolution.is_symptom[1], "eşleşen scope_key ile GEO_002 semptom olmalı");
    }

    #[test]
    fn symptom_resolution_different_scope_not_marked() {
        let mut root = notice("root", "STP_001", Severity::Kritik, RuleClass::Spec);
        root.scope_key = Some("S1".into());
        root.blocks = vec!["GEO_002".to_string()];

        let mut other = notice("other", "GEO_002", Severity::Orta, RuleClass::Analytics);
        other.scope_key = Some("S2".into()); // farklı scope

        let resolution = resolve_symptoms(&[root, other]);
        assert!(!resolution.is_symptom[1], "farklı scope_key → semptom değil");
    }

    #[test]
    fn symptom_resolution_no_blocks_nothing_marked() {
        let n1 = notice("n1", "ARC_001", Severity::Kritik, RuleClass::Spec);
        let n2 = notice("n2", "ARC_008", Severity::Kritik, RuleClass::Spec);
        let resolution = resolve_symptoms(&[n1, n2]);
        assert!(resolution.is_symptom.iter().all(|&s| !s));
    }

    // None scope'lu root, None scope'lu semptomu işaretlemeli
    #[test]
    fn symptom_resolution_none_scope_root_marks_none_scope_symptom() {
        let mut root = notice("root", "ARC_004", Severity::Kritik, RuleClass::Spec);
        root.scope_key = None;
        root.blocks = vec!["DQ_005".to_string()];

        let symptom = notice("sym", "DQ_005", Severity::Yuksek, RuleClass::Interop);
        // symptom.scope_key = None (default)

        let resolution = resolve_symptoms(&[root, symptom]);
        assert!(resolution.is_symptom[1]);
    }

    // None scope'lu root, Some(x) scope'lu semptomu da işaretlemeli (feed-level etki)
    #[test]
    fn symptom_resolution_none_scope_root_marks_entity_scope_symptom() {
        let mut root = notice("root", "ARC_004", Severity::Kritik, RuleClass::Spec);
        root.scope_key = None;
        root.blocks = vec!["TRP_003".to_string()];

        let mut symptom = notice("sym", "TRP_003", Severity::Kritik, RuleClass::Spec);
        symptom.scope_key = Some("trip_1".into());

        let resolution = resolve_symptoms(&[root, symptom]);
        assert!(resolution.is_symptom[1],
            "None root → entity-level semptom da bastırılmalı");
    }

    // Zincir A→B→C: hem B hem C semptom olmalı
    #[test]
    fn symptom_resolution_chain_a_blocks_b_blocks_c() {
        let mut a = notice("a", "STP_001", Severity::Kritik, RuleClass::Spec);
        a.scope_key = Some("S1".into());
        a.blocks = vec!["GEO_009".to_string()];

        let mut b = notice("b", "GEO_009", Severity::Kritik, RuleClass::Interop);
        b.scope_key = Some("S1".into());
        b.blocks = vec!["SHP_017".to_string()];

        let mut c = notice("c", "SHP_017", Severity::Yuksek, RuleClass::Interop);
        c.scope_key = Some("S1".into());

        let all = [a, b, c];
        let resolution = resolve_symptoms(&all);
        assert!(!resolution.is_symptom[0], "A kök neden olmalı");
        assert!(resolution.is_symptom[1],  "B semptom olmalı");
        assert!(resolution.is_symptom[2],  "C semptom olmalı");
    }

    // realized_dep_for_group: sayısal değer doğrulaması
    #[test]
    fn realized_dep_for_group_counts_fired_blocked_rules() {
        let mut root = notice("r", "ARC_001", Severity::Kritik, RuleClass::Spec);
        root.blocks = vec!["B1".to_string(), "B2".to_string(), "B3".to_string()];

        let b1 = notice("b1", "B1", Severity::Orta, RuleClass::Quality);
        let b2 = notice("b2", "B2", Severity::Orta, RuleClass::Quality);
        // B3 için notice yok → realized_dep = 2

        let all = vec![root, b1, b2];
        let resolution = resolve_symptoms(&all);
        let dep = realized_dep_for_group(&[0], &all, &resolution.rule_scope_index);
        assert_eq!(dep, 2, "B1 ve B2 ateşlendi, B3 yok → realized_dep=2");
    }

    // ── R1 ────────────────────────────────────────────────────────────────────

    #[test]
    fn r1_spec_kritik_makes_not_publishable() {
        let n = notice("n1", "X", Severity::Kritik, RuleClass::Spec);
        let r = build_r1(&[n], true);
        assert!(!r.publishable);
        assert_eq!(r.blocker_notice_ids.len(), 1);
    }

    #[test]
    fn r1_high_interop_is_publishable() {
        // Faz 4: Interop artık yayın-engeli değil (yalnız Spec+Kritik).
        let n = notice("n1", "X", Severity::Yuksek, RuleClass::Interop);
        let r = build_r1(&[n], true);
        assert!(r.publishable);
        assert!(r.blocker_notice_ids.is_empty());
    }

    #[test]
    fn r1_no_blockers_fully_publishable() {
        let n = notice("n1", "X", Severity::Orta, RuleClass::Quality);
        let r = build_r1(&[n], true);
        assert!(r.publishable);
    }

    #[test]
    fn r1_kritik_interop_is_publishable() {
        // Faz 4: Kritik Interop bile yayını engellemez (spec kapısı değil).
        let n = notice("n1", "X", Severity::Kritik, RuleClass::Interop);
        let r = build_r1(&[n], true);
        assert!(r.publishable);
        assert!(r.blocker_notice_ids.is_empty());
    }

    // ── R9 ────────────────────────────────────────────────────────────────────

    #[test]
    fn r9_excludes_symptoms() {
        let mut root = notice("root", "STP_001", Severity::Kritik, RuleClass::Spec);
        root.scope_key = Some("S1".into());
        root.blocks = vec!["GEO_002".to_string()];
        let mut sym = notice("sym", "GEO_002", Severity::Orta, RuleClass::Analytics);
        sym.scope_key = Some("S1".into());

        let all = vec![root, sym];
        let resolution = resolve_symptoms(&all);
        let r9 = build_r9(&all, &resolution);
        // Semptom (GEO_002) R9'da görünmemeli
        assert!(r9.items.iter().all(|i| i.rule_id != "GEO_002"),
            "semptom R9'da görünmemeli");
    }

    #[test]
    fn r9_bucket_order_blocker_before_interop() {
        // Faz 4: yalnız Spec+Kritik → Blocker. Interop (Kritik dahil) → Interop bucket;
        // bucket içinde priority_score'a göre (yüksek severity önce) sıralanır.
        let interop_low  = notice("i1", "INTEROP_LOW",  Severity::Orta,   RuleClass::Interop);
        let interop_high = notice("ih", "INTEROP_HIGH", Severity::Yuksek, RuleClass::Interop);
        let blocker      = notice("bl", "BLOCKER",      Severity::Kritik, RuleClass::Spec);

        let resolution = resolve_symptoms(&[interop_low.clone(), interop_high.clone(), blocker.clone()]);
        let r9 = build_r9(&[interop_low, interop_high, blocker], &resolution);

        assert_eq!(r9.items.len(), 3);
        assert!(r9.items[0].labels.contains(&R9Label::Blocker), "ilk sıra: blocker");
        assert!(r9.items[1].labels.contains(&R9Label::Interop), "ikinci sıra: interop (yüksek)");
        assert!(r9.items[2].labels.contains(&R9Label::Interop), "üçüncü sıra: interop (orta)");
        assert_eq!(r9.items[1].rule_id, "INTEROP_HIGH", "yüksek interop, orta'dan önce");
    }

    #[test]
    fn r9_propagation_label_when_realized_dep_gt_2() {
        // 3 farklı blocked rule'un hepsinin notice'ı var → realized_dep = 3 → Propagation
        let mut root = notice("root", "ARC_001", Severity::Kritik, RuleClass::Spec);
        root.blocks = vec!["B1".to_string(), "B2".to_string(), "B3".to_string()];

        let b1 = notice("b1", "B1", Severity::Orta, RuleClass::Quality);
        let b2 = notice("b2", "B2", Severity::Orta, RuleClass::Quality);
        let b3 = notice("b3", "B3", Severity::Orta, RuleClass::Quality);

        let all = [root, b1, b2, b3];
        let resolution = resolve_symptoms(&all);
        let r9 = build_r9(&all, &resolution);

        let root_item = r9.items.iter().find(|i| i.rule_id == "ARC_001").unwrap();
        assert!(root_item.labels.contains(&R9Label::Propagation),
            "realized_dep=3 → Propagation etiketi beklenir");
    }

    #[test]
    fn r9_quick_win_label() {
        // base_effort=1, 1 instance → fix_effort=1.0, severity=YÜKSEK → QuickWin
        let n = notice("n1", "ARC_001", Severity::Yuksek, RuleClass::Spec);
        let resolution = resolve_symptoms(std::slice::from_ref(&n));
        let r9 = build_r9(&[n], &resolution);
        assert!(r9.items[0].labels.contains(&R9Label::QuickWin));
    }

    // ── R4 display label ──────────────────────────────────────────────────────

    #[test]
    fn r4_display_label_maps_shp_aliases() {
        assert_eq!(r4_display_label("SHP_014"), "GEO_010");
        assert_eq!(r4_display_label("SHP_016"), "GEO_011");
        assert_eq!(r4_display_label("GEO_002"), "GEO_002");
        assert_eq!(r4_display_label("SHP_013"), "SHP_013"); // artık alias yok
    }

    // ── R5 skor ───────────────────────────────────────────────────────────────

    #[test]
    fn r5_empty_feed_scores_100() {
        let r5 = build_r5(&[]);
        assert_eq!(r5.score, 100.0);
        assert_eq!(r5.spec_score, 100.0);
    }

    #[test]
    fn r5_errors_reduce_score() {
        let n = notice("n1", "X", Severity::Kritik, RuleClass::Spec);
        let r5 = build_r5(&[n]);
        assert!(r5.score < 100.0, "hata varken skor 100 olamaz");
        assert!(r5.spec_score < 100.0);
        assert_eq!(r5.interop_score, 100.0, "INTEROP hatası yok → 100");
    }

    // ── R9 skor formülü ───────────────────────────────────────────────────────

    #[test]
    fn priority_score_formula() {
        // severity_weight=4 (KRİTİK), realized_dep=0, affected=1, fix_effort=1.0
        // expected = 4 × 1 × log2(2) / 1 = 4.0
        let score = compute_priority_score(Severity::Kritik, 0, 1, 1.0);
        assert!((score - 4.0).abs() < 0.01, "beklenen ~4.0, gelen {score}");
    }

    #[test]
    fn instance_multiplier_boundaries() {
        assert_eq!(instance_multiplier(1), 1.0);
        assert_eq!(instance_multiplier(5), 1.0);
        assert_eq!(instance_multiplier(6), 1.5);
        assert_eq!(instance_multiplier(50), 1.5);
        assert_eq!(instance_multiplier(51), 2.0);
    }

    // ── Closure: yardımcı fonksiyon testleri ─────────────────────────────────

    /// Senaryo 1: Root notice bir semptom kapatıyor.
    /// score_delta, yalnızca root'un kendi cezasını değil closure setini (root + symptom)
    /// kapsadığından daha büyük olmalı.
    #[test]
    fn closure_score_delta_larger_than_root_only_penalty() {
        // Root: SPEC/KRİTİK — blocks SYMPTOM_RULE
        let mut root = notice("root", "ROOT_001", Severity::Kritik, RuleClass::Spec);
        root.blocks = vec!["SYMPTOM_RULE".to_string()];

        // Symptom: SPEC/YÜKSEK — root ile aynı kök sebebin semptomu
        let sym = notice("sym", "SYMPTOM_RULE", Severity::Yuksek, RuleClass::Spec);

        let all = vec![root, sym];
        let resolution = resolve_symptoms(&all);
        let r9 = build_r9(&all, &resolution);

        // R9'da yalnızca ROOT_001 görünmeli (SYMPTOM_RULE semptom olarak gizlendi)
        let root_item = r9.items.iter().find(|i| i.rule_id == "ROOT_001")
            .expect("ROOT_001 R9'da olmalı");

        // Yalnızca root'un cezasıyla hesaplanan delta: Spec sınıfı, k=50, ceza=4.0
        // cur_cs = 50*100/(50+7) ≈ 87.72; new_cs(sadece root) = 50*100/(50+3) ≈ 94.34
        // Ancak closure delta root+symptom cezasını kaldırır (4+3=7)
        // new_cs(closure) = 50*100/(50+0) = 100
        // Bu yüzden closure delta > sadece-root delta
        let root_only_delta = {
            let k = 50.0_f64;
            let cur_cp = 7.0_f64; // root(4) + sym(3)
            let cur_cs = k * 100.0 / (k + cur_cp);
            let new_cs = k * 100.0 / (k + (cur_cp - 4.0).max(0.0)); // yalnızca root cezası çıkar
            round1(0.4 * (new_cs - cur_cs))
        };

        assert!(
            root_item.score_delta > root_only_delta,
            "closure score_delta ({}) sadece-root delta'dan ({}) büyük olmalı",
            root_item.score_delta, root_only_delta
        );
    }

    /// Senaryo 2: Scope mismatch — farklı scope_key'deki blocked notice closure'a girmemeli.
    #[test]
    fn closure_scope_mismatch_excluded() {
        let mut root = notice("root", "ROOT_001", Severity::Kritik, RuleClass::Spec);
        root.scope_key = Some("scope_A".into());
        root.blocks = vec!["DOWNSTREAM".to_string()];

        // Farklı scope: closure'a girmemeli
        let mut ds_mismatch = notice("ds_mm", "DOWNSTREAM", Severity::Yuksek, RuleClass::Spec);
        ds_mismatch.scope_key = Some("scope_B".into());

        // Aynı scope: closure'a girmeli
        let mut ds_match = notice("ds_ok", "DOWNSTREAM", Severity::Yuksek, RuleClass::Spec);
        ds_match.scope_key = Some("scope_A".into());

        let all = vec![root, ds_mismatch, ds_match];
        let resolution = resolve_symptoms(&all);

        // Closure: group_indices = [0] (root)
        let closure = closure_notice_indices_for_group(&[0], &all, &resolution);

        assert!(closure.contains(&0), "root closure'da olmalı");
        assert!(closure.contains(&2), "aynı scope → closure'da olmalı");
        assert!(!closure.contains(&1), "farklı scope → closure'da olmamalı");
    }

    /// Senaryo 3: scope_key=None root, farklı scope'lardaki blocked notice'ları closure'a almalı.
    #[test]
    fn closure_none_scope_root_captures_all_scopes() {
        let mut root = notice("root", "FEED_ERR", Severity::Kritik, RuleClass::Spec);
        root.scope_key = None; // feed-level
        root.blocks = vec!["ENTITY_ERR".to_string()];

        let mut ds1 = notice("ds1", "ENTITY_ERR", Severity::Yuksek, RuleClass::Interop);
        ds1.scope_key = Some("trip_1".into());

        let mut ds2 = notice("ds2", "ENTITY_ERR", Severity::Yuksek, RuleClass::Interop);
        ds2.scope_key = Some("trip_2".into());

        let mut ds3 = notice("ds3", "ENTITY_ERR", Severity::Orta, RuleClass::Quality);
        ds3.scope_key = None;

        let all = vec![root, ds1, ds2, ds3];
        let resolution = resolve_symptoms(&all);

        let closure = closure_notice_indices_for_group(&[0], &all, &resolution);

        assert!(closure.contains(&0), "root closure'da");
        assert!(closure.contains(&1), "trip_1 scope → None root ile eşleşir");
        assert!(closure.contains(&2), "trip_2 scope → None root ile eşleşir");
        assert!(closure.contains(&3), "None scope downstream → None root ile eşleşir");
        assert_eq!(closure.len(), 4);
    }

    /// Senaryo 4: Transitif zincir A blocks B, B blocks C → A'nın closure'ı hem B hem C içermeli.
    #[test]
    fn closure_transitive_chain_a_blocks_b_blocks_c() {
        let mut a = notice("a", "RULE_A", Severity::Kritik, RuleClass::Spec);
        a.scope_key = Some("S1".into());
        a.blocks = vec!["RULE_B".to_string()];

        let mut b = notice("b", "RULE_B", Severity::Yuksek, RuleClass::Interop);
        b.scope_key = Some("S1".into());
        b.blocks = vec!["RULE_C".to_string()];

        let mut c = notice("c", "RULE_C", Severity::Orta, RuleClass::Quality);
        c.scope_key = Some("S1".into());

        let all = vec![a, b, c];
        let resolution = resolve_symptoms(&all);

        let closure = closure_notice_indices_for_group(&[0], &all, &resolution);

        assert!(closure.contains(&0), "A closure'da");
        assert!(closure.contains(&1), "B (doğrudan blocked) closure'da");
        assert!(closure.contains(&2), "C (transitif: A→B→C) closure'da");
        assert_eq!(closure.len(), 3);
    }

    /// Senaryo 4b: Transitif closure sonsuz döngüye girmemeli (döngüsel blocks).
    #[test]
    fn closure_no_infinite_loop_on_cycle() {
        // A→B→A döngüsü
        let mut a = notice("a", "RULE_A", Severity::Kritik, RuleClass::Spec);
        a.blocks = vec!["RULE_B".to_string()];

        let mut b = notice("b", "RULE_B", Severity::Yuksek, RuleClass::Interop);
        b.blocks = vec!["RULE_A".to_string()]; // döngü

        let all = vec![a, b];
        let resolution = resolve_symptoms(&all);

        // Sonsuz döngüye girmemeli
        let closure = closure_notice_indices_for_group(&[0], &all, &resolution);
        assert!(closure.contains(&0));
        assert!(closure.contains(&1));
        assert_eq!(closure.len(), 2, "döngüde bile her notice bir kez sayılmalı");
    }

    /// Senaryo 5: pub_score_delta yalnızca pub-relevant notice'lar üzerinden,
    /// gruplu hiperbolik hesapla artmalı.
    ///
    /// Kurulum:
    /// - Root: Kritik/Spec (pub-relevant) — blocks DOWNSTREAM
    /// - Downstream 1: Kritik/Spec (pub-relevant) — CLOSURE'A GİRER
    /// - Downstream 2: Orta/Quality (pub-relevant DEĞİL) — closure'a girse de pub'a etki etmez
    ///
    /// Beklenti:
    /// - pub_score_delta > 0 (en az root+downstream1 pub cezası kalktı)
    /// - Ancak quality notice pub_score_delta'ya katkı vermez
    #[test]
    fn closure_pub_score_delta_only_pub_relevant_notices() {
        let mut root = notice("root", "PUB_ROOT", Severity::Kritik, RuleClass::Spec);
        root.blocks = vec!["PUB_DS".to_string(), "QUAL_DS".to_string()];

        // pub-relevant downstream
        let ds_pub = notice("ds_pub", "PUB_DS", Severity::Kritik, RuleClass::Spec);
        // pub-relevant olmayan downstream
        let ds_qual = notice("ds_qual", "QUAL_DS", Severity::Orta, RuleClass::Quality);

        let all = vec![root, ds_pub, ds_qual];
        let resolution = resolve_symptoms(&all);
        let r9 = build_r9(&all, &resolution);

        let root_item = r9.items.iter().find(|i| i.rule_id == "PUB_ROOT")
            .expect("PUB_ROOT R9'da olmalı");

        // pub_score_delta > 0: en az pub-relevant root cezası kalktı
        assert!(root_item.pub_score_delta > 0.0,
            "pub-relevant root → pub_score_delta sıfırdan büyük olmalı; değer: {}",
            root_item.pub_score_delta);

        // pub_score_delta hesabını manuel doğrula:
        // total_pub_penalty: PUB_ROOT(4, cnt=1)×1.0 + PUB_DS(4, cnt=1)×1.0 = 8.0
        // closure pub penalty: aynı = 8.0
        // new_pub = 50*100/(50+0) = 100; cur_pub = 50*100/(50+8) ≈ 86.2
        // pub_score_delta ≈ 13.8
        let k = 50.0_f64;
        let total_pp = 8.0_f64; // PUB_ROOT(4×1) + PUB_DS(4×1)
        let cur_pub = k * 100.0 / (k + total_pp);
        let new_pub = k * 100.0 / (k + (total_pp - total_pp).max(0.0));
        let expected_delta = round1(new_pub - cur_pub);
        assert!(
            (root_item.pub_score_delta - expected_delta).abs() < 0.11,
            "pub_score_delta beklenen ≈{expected_delta}, gelen {}",
            root_item.pub_score_delta
        );
    }

    /// Senaryo 5b: Aynı rule_id altında birden fazla closure notice'ı varsa
    /// gruplu hiperbolik hesap (instance_multiplier) doğru uygulanmalı.
    #[test]
    fn closure_pub_score_delta_grouped_hyperbolic() {
        // Root: pub-relevant, blocks DS_RULE (birden fazla instance)
        let mut root = notice("root", "ROOT_PUB", Severity::Kritik, RuleClass::Spec);
        root.blocks = vec!["DS_RULE".to_string()];

        // Aynı rule_id'den 10 closure downstream (cnt=10 → multiplier=1.5)
        let mut downstream: Vec<Notice> = (0..10)
            .map(|i| notice(&format!("ds{i}"), "DS_RULE", Severity::Kritik, RuleClass::Spec))
            .collect();
        // Hepsi None scope → root ile eşleşir

        let mut all = vec![root];
        all.append(&mut downstream);

        let resolution = resolve_symptoms(&all);
        let r9 = build_r9(&all, &resolution);

        let root_item = r9.items.iter().find(|i| i.rule_id == "ROOT_PUB")
            .expect("ROOT_PUB R9'da olmalı");

        // Closure pub penalty:
        // ROOT_PUB: cnt=1 → 4×1.0 = 4
        // DS_RULE:  cnt=10 → 4×1.5 = 6  (10 > 5, cnt in 6..=50 → 1.5)
        // total = 10; closure_pub_penalty = 10; tüm pub penalty = 10
        // pub_score_delta = 50*100/(50+0) - 50*100/(50+10) = 100 - 83.3 = 16.7
        let k = 50.0_f64;
        let total_pp = 4.0 * 1.0 + 4.0 * 1.5; // root(1 instance) + ds(10 instances → ×1.5)
        let cur_pub = k * 100.0 / (k + total_pp);
        let new_pub = k * 100.0 / (k + 0.0_f64);
        let expected_delta = round1(new_pub - cur_pub);

        assert!(
            (root_item.pub_score_delta - expected_delta).abs() < 0.11,
            "gruplu hiperbolik pub_score_delta beklenen ≈{expected_delta}, gelen {}",
            root_item.pub_score_delta
        );
    }

    // R9 sıralaması eşit skorlarda deterministik mi? (rule_id tie-break)
    #[test]
    fn r9_order_deterministic_on_score_ties() {
        // Aynı severity/class, farklı rule_id → eşit bucket_rank ve priority_score.
        // Registry'de olmayan ID'ler → metadata fallback ile skorlar birebir eşit.
        // scope_key None, blocks yok → hepsi kök neden, hepsi R9 item olur.
        let scrambled = ["ZZ_003", "ZZ_001", "ZZ_002"];
        let notices: Vec<Notice> = scrambled
            .iter()
            .enumerate()
            .map(|(i, rid)| notice(&format!("n{i}"), rid, Severity::Orta, RuleClass::Quality))
            .collect();

        // build_r9 her çağrıda yeni HashMap (yeni random seed) kurar; çıktı sırası
        // tie-break sayesinde her çağrıda aynı VE rule_id'ye göre artan olmalı.
        let mut seen: Option<Vec<String>> = None;
        for _ in 0..16 {
            let resolution = resolve_symptoms(&notices);
            let r9 = build_r9(&notices, &resolution);
            let order: Vec<String> = r9.items.iter().map(|it| it.rule_id.clone()).collect();
            assert_eq!(
                order,
                vec!["ZZ_001", "ZZ_002", "ZZ_003"],
                "eşit skorlarda rule_id'ye göre artan sıralı olmalı"
            );
            match &seen {
                Some(prev) => assert_eq!(&order, prev, "tekrar çağrıda sıra değişmemeli (deterministik)"),
                None => seen = Some(order),
            }
        }
    }
}
