//! Rule IDENTITY check — "for how many distinct FACTS is a rule_id emitted?"
//!
//! `emit_coverage.rs` asks "does the rule occur in code" (dead rule).
//! `emit_proof.rs` asks "can the rule fire?".
//! `spec_conformance.rs` asks "is valid data silent?".
//! None asked: **is a rule_id also used for a fact OTHER than the one described by the
//! registry?**
//!
//! ATR_006/ATR_007 exposed this blind spot (issue #62): ATR_006 emits both the
//! `is_authority` enum (k2) and the `attributions.route_id` cross-reference (k4), while the
//! registry title describes only the first. A feed with a bad `route_id` gets a finding whose
//! title says "is_authority invalid" but whose fix says "Use a valid route_id". Adding a
//! second emit point to an existing rule_id was visible to NONE of the existing gates.
//!
//! ## Criterion: DISJOINT field sets
//! Multiple emit points for a rule are normal (missing/invalid branches, or measuring the same
//! fact in multiple files). The distinguishing case is when points have **no fields in common**:
//!   `CAL_003` → `[start_date]` and `[end_date, start_date]`  → overlap, same fact, SILENT
//!   `ATR_006` → `[is_authority]` and `[route_id]`            → DISJOINT, two facts, REPORT
//! Measured: 32 rows without the disjointness filter (mostly window spillover), 8 with it.
//!
//! ## Second check, tried and REJECTED
//! We also tried "if a registry title contains a field name, that name must occur at the emit
//! point": it produced 47 warnings, almost all noise (`feed_info` from the `feed_info.txt`
//! filename in a title, `cemv_support` for the `agency_cemv_support` field, and window spillover).
//! This check already catches ATR_006/ATR_007 → adding a second noise source has NO payoff and
//! was not kept.
//!
//! NOT A GATE, A LEDGER: legitimate examples exist (DQ_018 measures the same fact in seven text fields).
//! The ledger rejects new rows; every new row must be adjudicated.
//!
//! ## Ledger ADJUDICATION (2026-08-01) — reasons live here because the ledger is regenerated
//! | Rule | Decision |
//! |---|---|
//! | Rule | Decision |
//! | ~~ATR_006~~ | ✅ **RESOLVED (issue #62)** — The `route_id` FK was split into `ATR_011`; ATR_006 now measures only the `is_authority` enum. Removed from the ledger. |
//! | ~~ATR_007~~ | ✅ **RESOLVED (issue #62)** — The `trip_id` FK was split into `ATR_012`; ATR_007 now measures only `attribution_url`. Removed from the ledger. |
//! | **STP_003** | ⚠️ Same shape, but **EXPLICITLY stated in the title**: *"stop_name missing or stop_lat/stop_lon out of range (two separate conditions under the same ID)"*. Users do not receive a mislabeled finding. Splitting it is a separate decision; noted in #62. |
//! | ~~AGN_013~~ | ✅ **RESOLVED (2026-09-30)** — The first-agency duplicate in K2 was removed; the sole emitter is K4 `check_agencies` (per agency). Removed from the ledger. |
//! | BKR_001 | ✅ One fact: `prior_notice_start_day` / `prior_notice_last_day` share the same forbidden context |
//! | DQ_018 · DQ_019 | ✅ One fact (all-caps / control character), measured across seven text fields |
//! | OPR_010 | ✅ The title names both: *"accessibility OR bicycle policy conflicts"* |
//! | SHP_009 · SHP_020 | ✅ **Heuristic noise:** the detected literals are not fields but `details` map keys (`affected_shapes`, `repeated_shapes`) |
//! | STM_021 | ✅ **Heuristic noise (2026-11-04):** the `lib.rs` row is `aggregate_stm021`, which groups the K6 findings by the `details` keys `stop_a` / `stop_b` (one finding per stop pair); it emits no new fact |
//! | (Flex context loop) | ✅ **Heuristic noise, no row since 2026-11-07:** the Flex planner-context loop in `stop_times.rs` matches rule ids (STM_037–041, STM_051/052/054/055, STM_058) to add `details` keys (`location_id`, `window_start` …) to findings already emitted. The window lands on the list's last literal; it produced a STM_037/038 row, then a STM_055 row, and now falls on STM_058, whose own emit shares the window keys, so no disjoint row remains. It emits no new fact |
//! | TRF_017 | ✅ One fact, the two ends of a transfer: `from_route_id` / `to_route_id` |
//! | XFL_021 | ✅ The title already names the pair: *"(from_trip_id/to_trip_id, stop_id) pair"* |

use gtfs_rules::RULES;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// Removes `#[cfg(test)]` blocks by matching braces (same method as emit_coverage.rs).
fn strip_test_mods(src: &str) -> String {
    let marker = "#[cfg(test)]";
    let (mut out, mut rest) = (String::new(), src);
    while let Some(pos) = rest.find(marker) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos..];
        let Some(brace) = after.find('{') else {
            rest = "";
            break;
        };
        let bytes = after.as_bytes();
        let (mut depth, mut k) = (0i32, brace);
        while k < bytes.len() {
            match bytes[k] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        k += 1;
                        break;
                    }
                }
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
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
            out.push(p);
        }
    }
}

/// Does a string literal look like a GTFS field name? (snake_case, not a filename/sentence)
fn looks_like_field(s: &str) -> bool {
    s.contains('_')
        && !s.contains('.')
        && !s.contains(' ')
        && s.len() <= 40
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b == b'_' || b.is_ascii_digit())
}

/// Window: number of lines after the rule_id literal. Emit call argument lists are
/// shorter than this; stop early when another rule_id literal appears.
const WINDOW: usize = 14;

/// rule_id → field-like literals seen at each emit point.
fn emit_field_sets() -> BTreeMap<String, Vec<(String, BTreeSet<String>)>> {
    let mut files = Vec::new();
    rs_files(&repo_root().join("crates/pipeline/src"), &mut files);
    let known: BTreeSet<&str> = RULES.iter().map(|r| r.id).collect();
    let mut out: BTreeMap<String, Vec<(String, BTreeSet<String>)>> = BTreeMap::new();

    for f in &files {
        // k7 report-view logic is not an emit source (same exception as emit_coverage.rs).
        if f.file_name().and_then(|n| n.to_str()) == Some("k7_reporting.rs") {
            continue;
        }
        let text = strip_test_mods(&fs::read_to_string(f).unwrap());
        let lines: Vec<&str> = text.lines().collect();
        let fname = f.file_name().unwrap().to_string_lossy().to_string();

        for (i, line) in lines.iter().enumerate() {
            for id in known.iter() {
                if !line.contains(&format!("\"{id}\"")) {
                    continue;
                }
                let mut fields = BTreeSet::new();
                for l in lines.iter().take((i + WINDOW).min(lines.len())).skip(i) {
                    if *l != *line
                        && known
                            .iter()
                            .any(|o| o != id && l.contains(&format!("\"{o}\"")))
                    {
                        break;
                    }
                    // `details` keys (`route_label`, `affected_stops` …) fill the message
                    // template; they do not say WHICH FIELD the emit concerns.
                    // They looked like field names and polluted the identity set:
                    // OPR_010's two emits ceased to be disjoint because of a shared `route_label`
                    // and were removed from the ledger (2026-08-29).
                    if l.contains(".insert(") {
                        continue;
                    }
                    for lit in l.split('"').skip(1).step_by(2) {
                        if lit != *id && looks_like_field(lit) {
                            fields.insert(lit.to_string());
                        }
                    }
                }
                if !fields.is_empty() {
                    out.entry((*id).to_string())
                        .or_default()
                        .push((fname.clone(), fields));
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
        if sites.len() < 2 {
            continue;
        }
        // If any two points share a field, treat them as branches of the same fact.
        let overlap = sites
            .iter()
            .enumerate()
            .any(|(a, (_, sa))| sites.iter().skip(a + 1).any(|(_, sb)| !sa.is_disjoint(sb)));
        if overlap {
            continue;
        }
        let mut all: Vec<String> = sites
            .iter()
            .map(|(f, s)| format!("{f}:{}", s.iter().cloned().collect::<Vec<_>>().join("+")))
            .collect();
        all.sort_unstable();
        all.dedup();
        if all.len() < 2 {
            continue;
        }
        found.push(format!("{id}  {}", all.join("  ")));
    }
    found.sort_unstable();

    let ledger_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("emit_identity_ledger.txt");
    let ledger_raw = fs::read_to_string(&ledger_path).unwrap_or_default();
    let ledger: Vec<String> = ledger_raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect();

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
            found.len(),
            ledger.len(),
            added,
            removed,
        );
    }
}
