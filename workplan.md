# GTFS Analyzer — Triage Düzeltme İş Planı

Bu plan 2026-09-09 tarihli triage incelemesine göre hazırlanmıştır. İşler aşağıdaki commit sırasıyla yürütülecek. Push yapılmayacak.

## Karar ve kapsam sınırları

- `booking_rules.rs` ve `pathways.rs` mevcut doğrulamaları nedeniyle iş kapsamı dışındadır.
- Parse kampanyasında kalan runtime açıkları: `TRP_032`, `RTS_013`, `RTS_018`.
- Ayrı runtime işleri: `FAR_002`, `FMD_002`, `FMD_003`, `TRN_010`, `TRN_017`.
- `TRN_010` silinmeyecek; `TRN_017` eksik `record_sub_id`, `TRN_010` ise dolu fakat geçersiz/eşleşmeyen `record_sub_id` için kullanılacak.
- `JPN_019` içinden `valid_sub_id` çıkarılmayacak ve kural kapsamı daraltılmayacak.
- `TRN_010` doğrulaması mevcut `stop_times_index.sorted_stops(trip_id)` ile trip-bazlı yapılacak; global `stop_sequence` araması, yeni veri yapısı ve ham satır kopyası yapılmayacak.
- `FMD_003` `Quality` olarak kalacak ve yalnız tip `2` ile `4` için çalışacak.
- `FAR_013` yeni iş değildir. `FPD_002` yalnız başlık ve karar cümlesi düzeltmesidir.
- `GGL_002`, kesin kaynak ve feed kanıtı bulunana kadar karar bekleyen araştırmadır.
- `agency_lang` ve linked-trip çalışmaları ölçüm/tanım işidir; ölçüm veya kapsam kararı olmadan yeni Quality notice eklenmeyecek.

## Commit 1 — Timeout ve benchmark kanıtı

- [x] `mdb-784` yeniden çalıştırılmayacak.
- [x] Mevcut artifact’lerden `ANALYZER_TIMEOUT` / `MD_TIMEOUT` farkı belgelenecek.
- [x] 300 saniyede kesilme ve önceki koşum süreleri kaydedilecek.
- [x] Issue `#2142` gerçek GitHub durumundan kontrol edildi; bu checkout’un `ttezer/gtfs-analyzer` deposunda issue bulunamadı.
- [ ] Issue bulunamadığı için performans rakamlarına düzeltme notu/dipnot eklenmedi; dış issue durumu ayrıca doğrulanmayı bekliyor.
- [x] Timeout değişikliği uygulanacak.
- [x] Yeni tam korpus koşumuna kadar durum “düzeltildi, doğrulama bekliyor” olarak izlenecek.

Bu commit Python/benchmark kapsamındadır; WASM ve SDK kapısı çalıştırılmayacak.

## Commit 2 — Sessiz parse düzeltmeleri

- [x] `TRP_032`: parse edilemeyen `cars_allowed` değerleri raporlanacak.
- [x] `RTS_013`: `continuous_pickup` parse hataları raporlanacak.
- [x] `RTS_018`: `continuous_drop_off` parse hataları raporlanacak.
- [x] Her rule için geçerli, geçersiz sayısal, parse edilemeyen metin ve boş değer testleri eklenecek.
- [x] `booking_rules.rs` ve `pathways.rs` mevcut kontrolleri nedeniyle değiştirilmedi.
- [x] Daha önce düzeltilmiş `AGN_012`, `STP_008`, `STP_013`, `STM_022`, `STM_030`, `TRP_006`, `TRP_007`, `RTS_024` için runtime değişikliği yapılmadı; yalnız kartlar güncellendi.

Bu commit pipeline değişikliği içerdiği için Cargo testleri, Clippy, WASM ve SDK doğrulamaları çalıştırılacak.

## Commit 3 — FAR_002

- [x] Boş `fare_attributes.price` değerleri `FAR_002` ile raporlanacak.
- [x] Boş, geçersiz metin ve negatif değer senaryoları ayrı test edilecek.
- [x] `FAR_011` ve `FAR_012` ile notice davranışı karşılaştırılacak.

Bu commit pipeline değişikliği içerdiği için ilgili Rust/WASM/SDK kapıları çalıştırılacak.

## Commit 4 — Fare media ve locale

- [x] `ui/src/locales/en.ts`, `ja.ts` ve `fr.ts` içindeki yalnız `FMD_002` `recommendation` metinleri düzeltilecek.
- [x] Türkçe `FMD_002` öneri metni `fare_media.rs` içindeki hard-coded metinde düzeltilecek.
- [x] `FMD_002` eşlemeleri şu hale getirilecek:
  - `0`: None
  - `1`: Physical paper ticket
  - `2`: Physical transit card
  - `3`: cEMV
  - `4`: Mobile app
- [x] `FMD_003` kümesinden tip `1` çıkarılacak; yalnız `2` ve `4` kalacak.
- [x] Tip `1` bekleyen mevcut test düzeltilecek.
- [x] Tip `2` ve `4` testleri korunacak.
- [x] `FMD_003` locale mesajlarına dokunulmayacak.
- [x] `npm run locales:export` çalıştırılacak.
- [x] `FMD_003` için locale/export çıktısının değişmemesi beklenen kontrol olarak doğrulanacak.
- [x] `npm run locales:export -- --check` çalıştırılacak.
- [x] `FMD_003` sınıfı `Quality` olarak korunacak.

Locale zinciri: `ui/src/locales/{en,ja,fr}.ts` → export → `crates/cli/locales/*.json`. Türkçe öneri metni Rust kaynağında ayrı ele alınacak.

## Commit 5 — Dokümantasyon ve rule kartları

Aşağıdaki kartlarda runtime davranışı değil, eski açıklamalar düzeltilecek:

- [x] `AGN_012`
- [x] `STP_008`
- [x] `STP_013`
- [x] `STM_022`
- [x] `STM_030`
- [x] `TRP_006`
- [x] `TRP_007`
- [x] `RTS_024`

Her kart için:

- [x] Parse hatalarının artık notice ürettiği belirtilecek.
- [x] Artık geçerli olmayan yanlış-negatif iddiaları düzeltilecek.
- [x] `## Yanlış pozitif / negatif` bölümü korunacak.
- [x] Yalnız geçersiz maddeler güncellenecek.
- [x] `STP_008` ve `STP_013` kapsam cümleleri yeni davranışa göre düzeltilecek.

Ayrıca:

- [x] `FAR_004` kartında boş `payment_method` kontrolünün `FAR_011` tarafından yapıldığı belirtilecek.
- [x] `FPD_002` başlığı ve karar cümlesi negatif tutar Spec’e uygun olacak şekilde düzeltilecek.
- [x] `FAR_013` için eski/uygulanmayan iş veya iddia eklenmedi.
- [x] Dil README’leri ve ilgili `RULES.md` dosyaları güncel rule başlıkları, sınıfları ve aşamalarıyla eşleştirilecek.
- [x] README’lerdeki kural sayısı, badge, kapsam ve indeks referansları kontrol edilecek.

Bu commit yalnız dokümantasyon içeriyorsa WASM/SDK kapısı çalıştırılmayacak.

## Commit 6 — Triage güncellemesi

- [x] `spec-audit/PROVISION_TRIAGE.md` başına güncel durum tablosu eklenecek.
- [x] Tarihsel kayıtlar silinmeyecek.
- [x] Tarihsel bölümler “değiştirilmez kanıt kaydı” olarak etiketlenecek.
- [x] Runtime, Quality, dokümantasyon ve kapanmış maddeler ayrılacak.
- [x] Timeout maddesi “düzeltildi, tam korpus doğrulaması bekliyor” olarak gösterilecek.
- [x] `agency_lang` ve linked-trip maddeleri tanım/ölçüm bekleyen Quality işleri olarak gösterilecek.
- [x] Güncel durum ile tarihsel kanıt bölümlerinin birbirini çelişkili göstermediği doğrulanacak.

Bu commit yalnız triage/dokümantasyon içeriyorsa WASM/SDK kapısı çalıştırılmayacak.

## Araştırma hattı — GGL_002 ve Quality

Bu işler ana runtime commitlerini bloke etmeyecek.

### GGL_002

- [x] Japonya’ya özel `ic_price` sözleşmesinin kesin kaynağı bulundu: Google Transit resmi uzantı referansı.
- [x] Kaynak ve mevcut kod/fixture kapsamı karşılaştırıldı.
- [x] Alanın yalnız `fare_attributes.txt` kapsamındaki Google uzantısı olduğu kesinleştirildi.
- [x] Kaynak kanıtı gelmeden taşınmadı; resmi kaynak doğrulandıktan sonra taşındı.
- [x] Sonuç: `GGL_002` yalnız `fare_attributes.txt` içindeki `ic_price` değerini denetliyor.

### `agency_lang`

- [x] Gerçek korpusta eksiklik oranı ölçüldü: 830 feed, 826 `agency.txt`, 2.373 agency satırı; 463 eksik (%19,51), 54 feed etkileniyor.
- [x] Dil dağılımı çıkarıldı; en yaygın değerler `ja` 606, `de` 487, `cs` 329, `en` 135, `et` 64.
- [x] Ölçüm sonucunda yeni notice davranışı eklenmedi; Quality değerlendirmesi ayrı karar olarak kaldı.

### Linked trip

- [ ] “Linked trip” kapsamı için `block_id`, Fares v2 veya GTFS-JP anlamlarından biri seçilecek.
- [ ] Tanım seçilmeden hangi alan veya sefer ilişkisinin ölçüleceği belirlenmeyecek.
- [x] Mevcut kapsam dışı kararla karşılaştırıldı; mevcut hüküm KAPSAM DIŞI olarak korundu.
- [x] Tanım netleşmediği için mesafe ölçümü veya eşik belirlenmedi.

## Commit 7 — TRN/K4 geçişi

Bu commit çekirdek runtime düzeltmeleri içinde en sona bırakıldı. Bağımsız GGL araştırması
ve son doğrulama/doküman commitleri bunun ardından gelebilir; TRN kod kararı değişmez.

### `TRN_017`

- [x] K2’de eksik `record_sub_id` koşulu korunacak.
- [x] `record_id` dolu ve `record_sub_id` boş durumunda yalnız `TRN_017` üretilecek.
- [x] Sentetik fixture kullanılacak; korpusta örnek bulunmaması beklenen durum olarak kaydedilecek.

### `TRN_010`

- [x] K2’deki duplicate eksik-alan emisyonu kaldırılacak.
- [x] `TRN_010` K2’den K4’e taşınacak.
- [x] Dolu fakat eşleşmeyen `record_sub_id` için `TRN_010` üretilecek.
- [x] Mevcut `stop_times_index.sorted_stops(trip_id)` kullanılacak.
- [x] Arama `record_id` ile belirlenen trip’e göre yapılacak.
- [x] Global `stop_sequence` kümesi kullanılmayacak.
- [x] `u32::MAX` sentinel değeri geçersiz kabul edilecek.
- [x] `record_id` boş ve `record_sub_id` dolu durum TRN_010’a alınmayacak.
- [x] Rule kartındaki aşama `K2` → `K4` olarak güncellenecek.
- [x] Emisyon `make_k2_notice` yerine K4 `notice()` ile yapılacak.
- [x] `EntityType`, satır ve kimlik bağlamı K4 davranışına göre güncellenecek.

### `JPN_019`

- [x] `valid_sub_id` JPN_019’dan çıkarılmayacak.
- [x] JPN_019’un üçlü OR kontrolü korunacak.
- [x] GTFS-JP’ye özgü diğer tablo alt-kolları değiştirilmeyecek.
- [x] JPN_019 ile TRN_010’un aynı fixture’da birlikte üretim yapıp yapmadığı ölçülecek.
- [x] Çakışma varsa önce farklı eksenler doğrulanacak.
- [x] Gerekirse `blocks` ilişkisi değerlendirilecek; JPN_019 kapsamı daraltılmayacak.

### TRN kart ve senkron kapıları

- [x] Rule registry ve rule kartları güncellenecek.
- [x] Locale anahtarları kontrol edilecek.
- [x] `RULES.md` dosyaları güncellenecek.
- [x] `emit_proof` fixture’ları güncellenecek.
- [x] `md_parity_mapping.py` değiştirilmeyecek; bu rule’lar orada yok.
- [x] `cargo run -p gtfs-rules --example sync_cards` aynı commit içinde çalıştırılacak.
- [x] `card_consistency` testleri çalıştırılacak.
- [x] Aynı fixture’da duplicate notice olmadığı doğrulanacak.

Bu commit K2/K4 pipeline değişikliği içerdiği için Cargo testleri, Clippy, WASM ve SDK aşamaları zorunludur.

## Commit 8 — GGL_002 Google uzantısı dosya hizalaması

- [x] Google'ın resmi referansında `ic_price` alanının `fare_attributes.txt` altında olduğu doğrulandı.
- [x] `GGL_002` kontrolü `fare_products.txt`ten `fare_attributes.txt`e taşındı.
- [x] Eski Fares v2 kontrolü kaldırıldı; `fare_products.txt` artık `ic_price` nedeniyle işaretlenmiyor.
- [x] Unit testler ve `emit_proof` fixture'ı `fare_attributes.txt`e taşındı.
- [x] GGL_002 kartı, kaynak ve alan açıklamaları güncellendi.

Bu commit K2 pipeline değişikliği içerdiği için ilgili Cargo testleri, emit-proof ve kart senkronu zorunludur.

## Commit 9 — TRN entegrasyon testi hizalaması

- [x] `record_id` dolu ve `record_sub_id` boş senaryosunun entegrasyon beklentisi TRN_017 olarak düzeltildi.
- [x] `field_value` modunda TRN_010/TRN_017 sessizliği ve `record_id` modunda yalnız TRN_017 üretimi test edildi.

## Commit 10 — FPD_002 registry/kart son hizalaması

- [x] Registry başlığı negatif tutarı hatalı göstermeyecek şekilde `amount eksik veya sayısal değil` yapıldı.
- [x] FPD_002 kartındaki R9 mesajı registry ve gerçek runtime davranışıyla eşitlendi.
- [x] Negatif tutarın geçerli olduğunu açıklayan kart bölümü korundu.

## Commit bazlı doğrulama

- [x] Timeout commit’i: `test_timing` 25/25 benchmark testi; WASM/SDK yok.
- [x] Parse, FAR, FMD ve TRN commitleri: ilgili Cargo testleri ve Clippy geçti.
- [x] GGL commiti: fare_attributes testleri, emit-proof ve kaynak/kart senkronu.
- [x] Rust pipeline değişen commitlerde tam prepush WASM determinism kapısı tamamlandı; SDK kapısı geçti.
- [x] FMD commitinde locale export ve locale parity geçti.
- [x] TRN commitinde `sync_cards`, `emit_proof` ve `card_consistency` geçti.
- [x] Dokümantasyon commitinde rule parity ve doküman kontrolleri geçti.
- [x] Triage commitinde triage/evidence kontrolleri geçti.
- [x] Sabit test sayısı yerine tüm ilgili kontrollerin yeşil olması esas alındı.
- [x] Prepush `rust ui sdk wasm` kapısı tamamen geçti; wasm32/threads/memory64 parity `19 notice birebir eşit` verdi.
- [ ] `cargo fmt --all -- --check` repo-geneli mevcut format drift'i nedeniyle başarısız; formatter çalıştırılmadı ve kapsam dışı satırlar değiştirilmedi.
- [x] Push yapılmayacak.

## SDK paket kapağı — son aşama

- [x] CI ile aynı stable Rust toolchain doğrulandı: `rustc 1.98.1 (48a229cea 2026-09-01)`.
- [x] `rustc --version` kaydedildi.
- [x] Önce/sonra unpacked boyutları ölçüldü: `2.495.431 → 2.497.474` byte (`+2.043`, `%+0,0819`).
- [x] Önce/sonra packed boyutları ölçüldü: `898.394 → 898.757` byte (`+363`, `%+0,0404`).
- [x] `pkg/gtfs_wasm_bg.wasm` boyutu kaydedildi: `2.439.100 → 2.441.143` byte (`+2.043`, `%+0,0838`).
- [x] Beklenen 9 dosyalık paket listesi önce/sonra aynı çıktı.
- [x] `npm run package:check` geçti; güncel paket kapak altında kaldı.
- [x] `sdk/package-size-baseline.json` otomatik güncellenmedi.
- [x] Artış küçük, deterministik ve runtime düzeltmelerinin doğal sonucu olarak sınıflandırıldı; optimizasyon yeniden değerlendirilmedi.
- [x] Gerekçeli karar verilmeden SDK yayınlanmayacak; bu çalışma push/yayın yapmıyor.

## Son rapor

- [ ] Yerel commit listesi.
- [ ] Çalıştırılan test ve kapılar.
- [ ] Başarılı/başarısız doğrulamalar.
- [ ] Korpus ve timeout durumu.
- [ ] SDK boyut karşılaştırması.
- [ ] Açık kalan veya doğrulama bekleyen maddeler.
- [ ] Push yapılmadığı bilgisi.
