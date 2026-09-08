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
- [ ] Issue `#2142` gerçek GitHub durumundan kontrol edilecek; bu checkout’un `ttezer/gtfs-analyzer` deposunda issue bulunamadı.
- [ ] Issue gönderilmişse performans rakamları için düzeltme notu hazırlanacak; taslaksa dipnot eklenecek.
- [x] Timeout değişikliği uygulanacak.
- [x] Yeni tam korpus koşumuna kadar durum “düzeltildi, doğrulama bekliyor” olarak izlenecek.

Bu commit Python/benchmark kapsamındadır; WASM ve SDK kapısı çalıştırılmayacak.

## Commit 2 — Sessiz parse düzeltmeleri

- [x] `TRP_032`: parse edilemeyen `cars_allowed` değerleri raporlanacak.
- [x] `RTS_013`: `continuous_pickup` parse hataları raporlanacak.
- [x] `RTS_018`: `continuous_drop_off` parse hataları raporlanacak.
- [x] Her rule için geçerli, geçersiz sayısal, parse edilemeyen metin ve boş değer testleri eklenecek.
- [ ] `booking_rules.rs` ve `pathways.rs` değiştirilmeyecek.
- [ ] Daha önce düzeltilmiş `AGN_012`, `STP_008`, `STP_013`, `STM_022`, `STM_030`, `TRP_006`, `TRP_007`, `RTS_024` için runtime değişikliği yapılmayacak.

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
- [ ] `FAR_013` için eski/uygulanmayan iş veya iddia eklenmeyecek.
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

- [ ] Japonya’ya özel `ic_price` sözleşmesinin kesin kaynağı bulunacak.
- [ ] Kaynak, schema ve gerçek feed örnekleri karşılaştırılacak.
- [ ] Alanın `fare_attributes.txt`, `fare_products.txt` veya her ikisindeki kapsamı kesinleştirilecek.
- [ ] Kanıt gelmeden rule taşınmayacak veya mevcut kontrol silinmeyecek.
- [ ] Sonuç; yalnız `fare_attributes`, iki dosyada ayrı kontrol veya mevcut davranış seçeneklerinden biri olarak kaydedilecek.

### `agency_lang`

- [ ] Gerçek korpusta eksiklik oranı ölçülecek.
- [ ] Feed ve profil bazında dağılım çıkarılacak.
- [ ] Ölçüm olmadan yeni notice davranışı eklenmeyecek.

### Linked trip

- [ ] “Linked trip” kapsamı tanımlanacak.
- [ ] Hangi alan veya sefer ilişkisinin ölçüleceği belirlenecek.
- [ ] Mevcut kapsam dışı kararla karşılaştırılacak.
- [ ] Tanım netleşmeden mesafe ölçümü veya eşik belirlenmeyecek.

## Commit 7 — TRN/K4 geçişi

Bu commit en sona bırakılacak.

### `TRN_017`

- [ ] K2’de eksik `record_sub_id` koşulu korunacak.
- [ ] `record_id` dolu ve `record_sub_id` boş durumunda yalnız `TRN_017` üretilecek.
- [ ] Sentetik fixture kullanılacak; korpusta örnek bulunmaması beklenen durum olarak kaydedilecek.

### `TRN_010`

- [ ] K2’deki duplicate eksik-alan emisyonu kaldırılacak.
- [ ] `TRN_010` K2’den K4’e taşınacak.
- [ ] Dolu fakat eşleşmeyen `record_sub_id` için `TRN_010` üretilecek.
- [ ] Mevcut `stop_times_index.sorted_stops(trip_id)` kullanılacak.
- [ ] Arama `record_id` ile belirlenen trip’e göre yapılacak.
- [ ] Global `stop_sequence` kümesi kullanılmayacak.
- [ ] `u32::MAX` sentinel değeri geçersiz kabul edilecek.
- [ ] `record_id` boş ve `record_sub_id` dolu durum TRN_010’a alınmayacak.
- [ ] Rule kartındaki aşama `K2` → `K4` olarak güncellenecek.
- [ ] Emisyon `make_k2_notice` yerine K4 `notice()` ile yapılacak.
- [ ] `EntityType`, satır ve kimlik bağlamı K4 davranışına göre güncellenecek.

### `JPN_019`

- [ ] `valid_sub_id` JPN_019’dan çıkarılmayacak.
- [ ] JPN_019’un üçlü OR kontrolü korunacak.
- [ ] GTFS-JP’ye özgü diğer tablo alt-kolları değiştirilmeyecek.
- [ ] JPN_019 ile TRN_010’un aynı fixture’da birlikte üretim yapıp yapmadığı ölçülecek.
- [ ] Çakışma varsa önce farklı eksenler doğrulanacak.
- [ ] Gerekirse `blocks` ilişkisi değerlendirilecek; JPN_019 kapsamı daraltılmayacak.

### TRN kart ve senkron kapıları

- [ ] Rule registry ve rule kartları güncellenecek.
- [ ] Locale anahtarları kontrol edilecek.
- [ ] `RULES.md` dosyaları güncellenecek.
- [ ] `emit_proof` fixture’ları güncellenecek.
- [ ] `md_parity_mapping.py` değiştirilmeyecek; bu rule’lar orada yok.
- [ ] `cargo run -p gtfs-rules --example sync_cards` aynı commit içinde çalıştırılacak.
- [ ] `card_consistency` testleri çalıştırılacak.
- [ ] Aynı fixture’da duplicate notice olmadığı doğrulanacak.

Bu commit K2/K4 pipeline değişikliği içerdiği için Cargo testleri, Clippy, WASM ve SDK aşamaları zorunludur.

## Commit bazlı doğrulama

- [ ] Timeout commit’i: benchmark kontrolleri; WASM/SDK yok.
- [ ] Parse, FAR, FMD ve TRN commitleri: ilgili Cargo testleri ve Clippy.
- [ ] Rust pipeline değişen commitlerde WASM ve SDK.
- [ ] FMD commitinde locale export ve locale parity.
- [ ] TRN commitinde `sync_cards`, `emit_proof` ve `card_consistency`.
- [ ] Dokümantasyon commitinde rule parity ve doküman kontrolleri.
- [ ] Triage commitinde triage/evidence kontrolleri.
- [ ] Sabit test sayısı yerine tüm ilgili kontrollerin yeşil olması esas alınacak.
- [ ] Push yapılmayacak.

## SDK paket kapağı — son aşama

- [ ] CI ile aynı Rust toolchain doğrulanacak.
- [ ] `rustc --version` kaydedilecek.
- [ ] Önce/sonra unpacked boyutları ölçülecek.
- [ ] Önce/sonra packed boyutları ölçülecek.
- [ ] Byte ve yüzde farkı hesaplanacak.
- [ ] `pkg/gtfs_wasm_bg.wasm` boyutu kaydedilecek.
- [ ] Beklenen 9 dosyalık paket listesi karşılaştırılacak.
- [ ] `npm run package:check` çalıştırılacak.
- [ ] `sdk/package-size-baseline.json` otomatik güncellenmeyecek.
- [ ] Boyut artışı beklenmeyen büyüme, kabul edilebilir doğal artış veya optimizasyon gerektiren artış olarak sınıflandırılacak.
- [ ] Gerekçeli karar verilmeden SDK yayınlanmayacak.

## Son rapor

- [ ] Yerel commit listesi.
- [ ] Çalıştırılan test ve kapılar.
- [ ] Başarılı/başarısız doğrulamalar.
- [ ] Korpus ve timeout durumu.
- [ ] SDK boyut karşılaştırması.
- [ ] Açık kalan veya doğrulama bekleyen maddeler.
- [ ] Push yapılmadığı bilgisi.
