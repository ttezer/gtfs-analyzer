#!/usr/bin/env python3
from pathlib import Path

# FAR_006: GTFS transfer_duration is non-negative; zero is valid.
p = Path('crates/pipeline/src/k2/fare_attributes.rs')
s = p.read_text()
start = s.index('        let transfer_duration = match parse_u32(&row_map, "transfer_duration") {')
end = s.index('\n\n        records.push(FareAttributeRecord {', start)
replacement = '''        let transfer_duration = match parse_u32(&row_map, "transfer_duration") {
            Ok(value) => value,
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "FAR_006", EntityType::Fare, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("transfer_duration"),
                    get_trimmed_field(&row_map, "transfer_duration").map(str::to_string),
                    Some(">= 0".to_string()), err,
                    "transfer_duration değerini negatif olmayan bir tam sayıya ayarlayın veya boş bırakın.",
                ));
                None
            }
        };'''
s = s[:start] + replacement + s[end:]
p.write_text(s)

# TRP_017: if the frequency trip does not exist, FRQ_001 is the root FK error.
p = Path('crates/pipeline/src/k4_cross_ref.rs')
s = p.read_text()
old = '''        // TRP_017: frekans tabanlı sefer stop_times'ta eksik
        if !trips_in_stm.contains(rec.trip_id.as_str()) {'''
new = '''        // TRP_017: frekans tabanlı, trips.txt'te GEÇERLİ sefer stop_times'ta eksik.
        // trips.txt'te olmayan bir trip için FRQ_001 kök FK hatasıdır; aynı kökten
        // ayrıca "stop_times eksik" üretmek türev bulgudur ve düzeltme yönünü bulandırır.
        if map.trips.contains_key(rec.trip_id.as_str())
            && !trips_in_stm.contains(rec.trip_id.as_str()) {'''
assert s.count(old) == 1, f'TRP_017 source anchor count={s.count(old)}'
s = s.replace(old, new)
p.write_text(s)

# Runtime emit proof: FAR_006 fixture must now use an actually invalid negative value.
p = Path('crates/pipeline/tests/emit_proof.rs')
s = p.read_text()
old = 'fx("FAR_006", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method,transfer_duration\\nF1,2.5,USD,0,0\\n")]),'
new = 'fx("FAR_006", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method,transfer_duration\\nF1,2.5,USD,0,-1\\n")]),'
assert s.count(old) == 1, f'FAR_006 fixture anchor count={s.count(old)}'
s = s.replace(old, new)
assert 'fn full_catalog_far006_zero_is_valid_non_negative()' not in s
s += r'''

// Full-catalog regression: tld-4477 exposed 287k FAR_006 notices caused by the
// valid boundary value transfer_duration=0. GTFS defines this field as non-negative.
#[test]
fn full_catalog_far006_zero_is_valid_non_negative() {
    let files = with_opts(
        &[("fare_attributes.txt", "fare_id,price,currency_type,payment_method,transfer_duration\nF1,2.5,USD,0,0\n")],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    assert!(!notices.iter().any(|n| n.rule_id == "FAR_006"), "transfer_duration=0 is valid");
}

// Full-catalog regression: mdb-1106 produced FRQ_001 and TRP_017 for the same
// missing trip_id. FRQ_001 owns the broken FK; TRP_017 remains useful when the
// trip exists but has no stop_times rows.
#[test]
fn full_catalog_trp017_requires_an_existing_trip() {
    let missing_trip = with_opts(
        &[("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nMISSING,08:00:00,10:00:00,600\n")],
        &[],
        &[],
    );
    let notices = notices_for(&missing_trip, &ValidatorConfig::default());
    assert!(notices.iter().any(|n| n.rule_id == "FRQ_001"));
    assert!(!notices.iter().any(|n| n.rule_id == "TRP_017"));

    let known_without_stop_times = with_opts(
        &[
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n"),
            ("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT2,08:00:00,10:00:00,600\n"),
        ],
        &[],
        &[],
    );
    let notices = notices_for(&known_without_stop_times, &ValidatorConfig::default());
    assert!(!notices.iter().any(|n| n.rule_id == "FRQ_001"));
    assert!(notices.iter().any(|n| n.rule_id == "TRP_017"));
}
'''
p.write_text(s)

# FAR_006 card: document spec-conformant boundary.
p = Path('docs/rules/FAR/FAR_006.md')
s = p.read_text()
s = s.replace(
    '> **Karar:** `transfer_duration` alanı dolu olduğu halde pozitif `u32` olarak okunamıyorsa veya değeri `0` ise FAR_006 üretilir. **Düzeltme:** `transfer_duration` değerini pozitif tam sayıya ayarlayın veya boş bırakın.\n> ⚠️ GTFS spec `transfer_duration` için non-negative integer der; mevcut kod ise `0` değerini de geçersiz sayarak `> 0` bekler. Bu kart repo davranışını esas alır.',
    '> **Karar:** `transfer_duration` alanı dolu olduğu halde negatif olmayan bir `u32` olarak okunamıyorsa FAR_006 üretilir. `0` geçerlidir. **Düzeltme:** `transfer_duration` değerini negatif olmayan bir tam sayıya ayarlayın veya boş bırakın.\n> GTFS spec bu alanı **non-negative integer** olarak tanımlar; full-catalog audit `0` sınır değerinin yanlış reddedildiğini ortaya çıkardı.'
)
s = s.replace('`parse_u32(row, "transfer_duration")` hata döndürürse veya parse edilen değer `0` ise notice eklenir.', '`parse_u32(row, "transfer_duration")` hata döndürürse notice eklenir. Parse edilen `0` dahil tüm `u32` değerleri geçerlidir.')
s = s.replace('- **Yanlış pozitif:** Kod koşulu sağlanıyorsa veri modeli açısından sorun kabul edilir; özel işletme yorumu varsa veri tarafında ayrıca modellenmelidir.', '- **Yanlış pozitif:** `0` değerini reddetmek yanlış pozitiftir; GTFS bu sınır değerine izin verir. Bu regresyon testle kilitlenmiştir.')
s = s.replace('- `transfer_duration pozitif olmalıdır.`\n- **Çözüm önerisi:** `transfer_duration` değerini pozitif tam sayıya ayarlayın veya boş bırakın.', '- Parse edilemeyen/negatif `transfer_duration` değeri geçersizdir.\n- **Çözüm önerisi:** `transfer_duration` değerini negatif olmayan tam sayıya ayarlayın veya boş bırakın.')
s = s.replace('- Sınır değer: boş, geçersiz veya en küçük/geçerli değer ayrı denenmeli.', '- Sınır değer: `0` geçerli ve notice üretmemeli; negatif/geçersiz değer notice üretmeli.')
p.write_text(s)

# TRP_017 card: make the FRQ_001 root-cause guard explicit.
p = Path('docs/rules/TRP/TRP_017.md')
s = p.read_text()
s = s.replace(
    '> ⚠️ `check_frequencies` içinde, frekans satırının `trip_id`\'si `trips_in_stm` setinde yoksa tetiklenir; FRQ_001 (trip_id FK) ile aynı döngüde değerlendirilir.',
    '> `check_frequencies` içinde yalnız `trip_id` trips.txt\'te mevcut olduğu halde `trips_in_stm` setinde yoksa tetiklenir. trips.txt\'te olmayan kimlik FRQ_001 kök FK hatasıdır ve TRP_017 üretilmez.'
)
s = s.replace(
    'frequencies.trip_id trips_in_stm (stop_times\'lı trip seti) içinde DEĞİL\n→ TRP_017 (frequencies bazlı, trip kimlikli notice)',
    'frequencies.trip_id trips.txt içinde VAR\nVE frequencies.trip_id trips_in_stm (stop_times\'lı trip seti) içinde DEĞİL\n→ TRP_017 (frequencies bazlı, trip kimlikli notice)'
)
s = s.replace('- **Yanlış negatif:** Frekans satırının `trip_id`\'si trips.txt\'te de yoksa FRQ_001 (FK) öne çıkar.\n- **Yanlış pozitif:** Yok; frekans tabanlı sefer için stop_times zorunludur.', '- **Yanlış negatif:** Bilinen trip stop_times\'ta yoksa TRP_017 yine üretilir.\n- **Yanlış pozitif:** trips.txt\'te olmayan kimlik için ayrıca TRP_017 üretmek FRQ_001\'in türeviydi; full-catalog audit sonrası bu yol bloke edildi.')
s = s.replace('- `blocks` yok.', '- **FRQ_001 kök nedeni:** `frequencies.trip_id` trips.txt\'te yoksa TRP_017 çalışmaz; önce FK düzeltilir.')
s = s.replace('- frequencies trip_id\'si stop_times\'ta yok → TRP_017 çıkmalı.\n- frequencies trip_id\'si stop_times\'ta var → notice yok.', '- trips.txt\'te mevcut frequency trip_id\'si stop_times\'ta yok → TRP_017 çıkmalı.\n- frequency trip_id trips.txt\'te yok → FRQ_001 çıkar, TRP_017 çıkmaz.\n- frequency trip_id stop_times\'ta var → notice yok.')
p.write_text(s)

print('patch applied')
