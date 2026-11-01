/// Step 0 timing + notice baseline tool.
///
/// Usage:
///   cargo run --example baseline -- <feed.zip> [today_yyyymmdd]
///
/// Timing → stderr (written automatically by Timer::drop)
/// Notice baseline → stdout
use std::collections::{BTreeMap, BTreeSet};

use gtfs_core::{Notice, Severity};
use gtfs_pipeline::{validate_bytes, ValidateResult, ValidatorConfig};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: baseline <feed.zip> [today_yyyymmdd] [--json]");
        std::process::exit(1);
    }

    let path = &args[1];
    // today = first u32-parseable argument after path (skips the --json flag)
    let today: u32 = args[2..]
        .iter()
        .find_map(|s| s.parse().ok())
        .unwrap_or(20_260_515);
    let json_mode = args.iter().any(|a| a == "--json");

    let zip_bytes = std::fs::read(path).unwrap_or_else(|e| {
        eprintln!("Could not read file '{path}': {e}");
        std::process::exit(1);
    });

    eprintln!("=== TIMING (stderr) ===");
    let result = validate_bytes(&zip_bytes, &ValidatorConfig::default(), today);

    let vr = match result {
        ValidateResult::Ok(vr) => vr,
        ValidateResult::Fatal(e) => {
            eprintln!("FATAL: {:?} — {}", e.code, e.message);
            std::process::exit(2);
        }
    };

    if json_mode {
        let mut vr = vr;
        // The R9 queue is order-ambiguous for items tied on (bucket, priority_score)
        // (build_r9 sorting is not stable on ties — this is pre-existing and independent of
        // my changes). Sort CANONICALLY by rule_id for a meaningful diff (rule_id is unique
        // in R9). This canonicalizes only the dev dump → it DOES NOT CHANGE pipeline output.
        // Report equality = content equality.
        vr.reports
            .r9
            .items
            .sort_by(|a, b| a.rule_id.cmp(&b.rule_id));
        // Canonical (key-sorted) FULL ValidationResult dump → rules AND reports
        // (notices + R1–R9 + metrics + name_index + capped_totals). serde_json::Value::Object
        // is a BTreeMap, so all map keys are sorted → HashMap ordering ambiguity is removed.
        let canon: serde_json::Value = serde_json::to_value(&vr).expect("serialize");
        println!("{}", serde_json::to_string(&canon).expect("to_string"));
    } else {
        print_baseline(&vr.notices, path);
    }
}

fn print_baseline(notices: &[Notice], source: &str) {
    println!("\n=== NOTICE BASELINE: {source} ===");
    println!("Total notices: {}", notices.len());

    // 1. rule_id histogram (sorted)
    let mut histogram: BTreeMap<&str, u32> = BTreeMap::new();
    for n in notices {
        *histogram.entry(n.rule_id.as_str()).or_default() += 1;
    }

    println!("\n--- rule_id histogram ---");
    for (rule_id, count) in &histogram {
        println!("{rule_id}: {count}");
    }

    // 2. critical/high set (sorted, with severity)
    println!("\n--- critical/high set ---");
    let mut crit_high: BTreeSet<String> = BTreeSet::new();
    for n in notices {
        if matches!(n.severity, Severity::Kritik | Severity::Yuksek) {
            crit_high.insert(format!("{} ({:?})", n.rule_id, n.severity));
        }
    }
    for entry in &crit_high {
        println!("{entry}");
    }
    if crit_high.is_empty() {
        println!("(none)");
    }

    // 3. per rule_id: line numbers + entity_id + scope_key samples
    println!("\n--- per rule_id detail (first 3 samples each) ---");
    let mut per_rule: BTreeMap<&str, Vec<&Notice>> = BTreeMap::new();
    for n in notices {
        per_rule.entry(n.rule_id.as_str()).or_default().push(n);
    }

    for (rule_id, ns) in &per_rule {
        let lines: Vec<String> = ns
            .iter()
            .filter_map(|n| n.line)
            .take(3)
            .map(|l| l.to_string())
            .collect();
        let entity_ids: Vec<&str> = ns
            .iter()
            .filter_map(|n| n.entity_id.as_deref())
            .take(3)
            .collect();
        let scope_keys: Vec<&str> = ns
            .iter()
            .filter_map(|n| n.scope_key.as_deref())
            .take(3)
            .collect();

        println!(
            "{rule_id} | count={} | lines=[{}] | entity_ids=[{}] | scope_keys=[{}]",
            ns.len(),
            lines.join(", "),
            entity_ids.join(", "),
            scope_keys.join(", "),
        );
    }
}
