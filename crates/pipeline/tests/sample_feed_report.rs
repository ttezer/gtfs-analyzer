/// Gerçek sample-feed-1.zip üzerinde pipeline çalıştırır ve notice'ları dosyaya yazar.
/// Karşılaştırma amaçlı — normal CI'da ignore edilir.
///   cargo test -p gtfs-pipeline --test sample_feed_report -- --nocapture --ignored
#[test]
#[ignore]
fn run_and_print_sample_feed_notices() {
    let zip_path = r"D:\gtfs\test\data\MBTA_GTFS.zip";
    let zip = std::fs::read(zip_path).expect("sample-feed-1.zip okunamadı");

    use gtfs_pipeline::{validate_bytes, ValidateResult, ValidatorConfig};
    let today: u32 = 20_260_520; // 2026-05-20
    let result = validate_bytes(&zip, &ValidatorConfig::default(), today);

    let notices = match result {
        ValidateResult::Ok(ref vr) => &vr.notices,
        ValidateResult::Fatal(ref e) => {
            println!("FATAL: {:?} — {}", e.code, e.message);
            return;
        }
    };

    println!("\n=== BİZİM SONUÇLARIMIZ ({} notice) ===\n", notices.len());

    // Severity + rule bazında grupla
    let mut by_rule: std::collections::BTreeMap<String, Vec<&gtfs_core::Notice>> =
        std::collections::BTreeMap::new();
    for n in notices {
        by_rule.entry(format!("{:?}|{}", n.severity, n.rule_id)).or_default().push(n);
    }

    for (key, group) in &by_rule {
        let parts: Vec<&str> = key.splitn(2, '|').collect();
        let (sev, rule) = (parts[0], parts[1]);
        println!("▸ [{sev}] {rule}  ({} adet)", group.len());
        for n in group.iter().take(5) {
            let satir = n.line.map_or(String::new(), |l| format!(" satır {l}"));
            let dosya = n.file.as_deref().unwrap_or("");
            println!("  {dosya}{satir} — {}", n.message);
        }
        if group.len() > 5 {
            println!("  ... +{} daha", group.len() - 5);
        }
        println!();
    }

    // Özet say
    let mut crit = 0usize; let mut high = 0; let mut med = 0; let mut low = 0; let mut info = 0;
    for n in notices {
        match n.severity {
            gtfs_core::Severity::Kritik   => crit += 1,
            gtfs_core::Severity::Yuksek   => high += 1,
            gtfs_core::Severity::Orta     => med  += 1,
            gtfs_core::Severity::Dusuk    => low  += 1,
            gtfs_core::Severity::Bilgi    => info += 1,
        }
    }
    println!("=== ÖZET ===");
    println!("Kritik: {crit}  |  Yüksek: {high}  |  Orta: {med}  |  Düşük: {low}  |  Bilgi: {info}");
}
