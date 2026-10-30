use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct FareRuleRecord {
    pub fare_id: String,
    pub route_id: Option<String>,
    pub origin_id: Option<String>,
    pub destination_id: Option<String>,
    pub contains_id: Option<String>,
    pub line: u64,
}

/// Satır başına `RowMap` kurulmaz: hiçbir tüketici okumuyordu ve büyük fare_rules
/// dosyalarında (mdb-992: 37 MB) her satır için HashMap + başlık/değer String'leri
/// ayırmak K2'yi native'de 0,3 s iken WASM'da 5,6 s (wasm32) / 25 s (wasm64) yapıyordu.
pub fn parse_fare_rules(file: &RawFile) -> Vec<FareRuleRecord> {
    // Yinelenen başlıkta eski RowMap son sütunu tutuyordu; rposition aynı sonucu verir.
    let col = |name: &str| file.headers.iter().rposition(|h| h == name);
    let fare_id_col = col("fare_id");
    let route_id_col = col("route_id");
    let origin_id_col = col("origin_id");
    let destination_id_col = col("destination_id");
    let contains_id_col = col("contains_id");
    file.rows
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let raw = |c: Option<usize>| c.and_then(|i| row.get(i)).map(|v| v.as_str());
            let present = |c: Option<usize>| {
                raw(c)
                    .filter(|v| !v.trim().is_empty())
                    .map(str::to_string)
            };
            FareRuleRecord {
                fare_id: raw(fare_id_col).unwrap_or("").to_string(),
                route_id: present(route_id_col),
                origin_id: present(origin_id_col),
                destination_id: present(destination_id_col),
                contains_id: present(contains_id_col),
                line: (row_idx + 2) as u64,
            }
        })
        .collect()
}
