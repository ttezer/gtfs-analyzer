/// Regression test runner — tek bir feed ZIP'ini çalıştırıp JSON özet üretir.
///
/// Kullanım:
///   cargo run --example regress --release -- <feed.zip> [today_yyyymmdd]
///
/// Çıktı (stdout, JSON):
///   {
///     "source": "feed.zip",
///     "notice_total": 42,
///     "by_rule": { "ARC_007": 3, "STP_003": 12, ... },
///     "by_severity": { "CRITICAL": 1, "HIGH": 5, "MEDIUM": 8, "LOW": 20, "INFO": 8 },
///     "fatal": null          // veya { "code": "ZipUnreadable", "message": "..." }
///   }
use std::collections::BTreeMap;

use gtfs_core::Severity;
use gtfs_pipeline::{validate_bytes, ValidateResult, ValidatorConfig};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Kullanım: regress <feed.zip> [today_yyyymmdd]");
        std::process::exit(1);
    }

    let source = &args[1];
    let today: u32 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_260_101);

    let zip_bytes = match std::fs::read(source) {
        Ok(b) => b,
        Err(e) => {
            let out = serde_json::json!({
                "source": source,
                "notice_total": 0,
                "by_rule": {},
                "by_severity": {},
                "fatal": { "code": "IoError", "message": e.to_string() }
            });
            println!("{}", serde_json::to_string(&out).unwrap());
            return;
        }
    };

    let result = validate_bytes(&zip_bytes, &ValidatorConfig::default(), today);

    match result {
        ValidateResult::Fatal(e) => {
            let out = serde_json::json!({
                "source": source,
                "notice_total": 0,
                "by_rule": {},
                "by_severity": {},
                "fatal": { "code": format!("{:?}", e.code), "message": e.message }
            });
            println!("{}", serde_json::to_string(&out).unwrap());
        }
        ValidateResult::Ok(vr) => {
            let mut by_rule: BTreeMap<&str, u32> = BTreeMap::new();
            let mut by_severity: BTreeMap<&str, u32> = BTreeMap::new();

            for n in &vr.notices {
                *by_rule.entry(n.rule_id.as_str()).or_default() += 1;
                let sev = match n.severity {
                    Severity::Kritik => "CRITICAL",
                    Severity::Yuksek => "HIGH",
                    Severity::Orta   => "MEDIUM",
                    Severity::Dusuk  => "LOW",
                    Severity::Bilgi  => "INFO",
                };
                *by_severity.entry(sev).or_default() += 1;
            }

            let out = serde_json::json!({
                "source": source,
                "notice_total": vr.notices.len(),
                "by_rule": by_rule,
                "by_severity": by_severity,
                "fatal": null
            });
            println!("{}", serde_json::to_string(&out).unwrap());
        }
    }
}
