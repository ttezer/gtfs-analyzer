/// Adım 0 timing + notice baseline aracı.
///
/// Kullanım:
///   cargo run --example baseline -- <feed.zip> [today_yyyymmdd]
///
/// Timing → stderr (Timer::drop otomatik yazar)
/// Notice baseline → stdout
use std::collections::{BTreeMap, BTreeSet};

use gtfs_pipeline::{validate_bytes, ValidateResult, ValidatorConfig};
use gtfs_core::{Notice, Severity};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Kullanım: baseline <feed.zip> [today_yyyymmdd]");
        std::process::exit(1);
    }

    let path = &args[1];
    let today: u32 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_260_515);

    let zip_bytes = std::fs::read(path).unwrap_or_else(|e| {
        eprintln!("Dosya okunamadı '{path}': {e}");
        std::process::exit(1);
    });

    eprintln!("=== TIMING (stderr) ===");
    let result = validate_bytes(&zip_bytes, &ValidatorConfig::default(), today);

    let notices = match result {
        ValidateResult::Ok(vr) => vr.notices,
        ValidateResult::Fatal(e) => {
            eprintln!("FATAL: {:?} — {}", e.code, e.message);
            std::process::exit(2);
        }
    };

    print_baseline(&notices, path);
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
        let lines: Vec<String> = ns.iter()
            .filter_map(|n| n.line)
            .take(3)
            .map(|l| l.to_string())
            .collect();
        let entity_ids: Vec<&str> = ns.iter()
            .filter_map(|n| n.entity_id.as_deref())
            .take(3)
            .collect();
        let scope_keys: Vec<&str> = ns.iter()
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
