# GTFS-JP takip doğrulaması — 2026-09-14

Bu kayıt, `JPN_027`, `JPN_029` toplulaması, `JPN_031` ve `JPN_006` ücret istisnası düzeltmelerinin kanıtıdır. Tarihsel 13 Eylül ölçümü [önceki kayıtta](gtfs-jp-v3-v4-validation-2026-09-13.md) korunmuştur.

## Korpus ölçümü

592 Japon feed manifestinden 590 arşiv başarıyla koştu; iki URL ZIP olmayan cevap verdi (`mdb-1057`, `mdb-874`) ve sonuçtan çıkarıldı. Aynı arşiv her iki binary ile, `--gtfs-jp-profile v4 --today 20260914` kullanılarak macOS arm64 release binary üzerinde çalıştırıldı. Önceki binary `cd5fda99` kaynaklarından, sonraki binary bu düzeltmelerin son çalışma ağacından derlendi. Arşiv SHA-256 değerleri ve kural/sayaç/skor sonuçları yerel `tmp/jp-followup-20260914/corpus-v4-final.jsonl` kaydındadır; manifest `run13-artifacts/jp-analyzer-only-20260825.jsonl` dosyasıdır. Bu yollar repository'nin üst dizinine göredir.

Ölçülen native binary SHA-256 değerleri:

- Önce: `143d9f0430517663734f3323c2e0b0efe156a9edb98c562667f363d502cde187`.
- Sonra: `c52a363610897af0532e2bb80170fda8dc2e671ed618d7c6213d256699b65e7f`.

| Ölçüm | Önce | Sonra |
|---|---:|---:|
| Koşan feed | 590 | 590 |
| Toplam notice | 2.102.555 | 211.854 |
| JSON çıktısı | 1.554.523.518 byte | 171.433.422 byte |
| `JPN_029` notice | 1.901.271 | 10.570 |
| `JPN_029` `affected_records` toplamı | - | **1.901.271** |
| `JPN_029` etkilenmiş feed | 435 | 435 |

`JPN_029` artık K4'te `(table_name, field_name, source_value)` başına tek notice üretir. Böylece notice sayısı %99,44 azaldı; etkilenen satırların tamamı grup sayaçlarında kaldı. Her feed'de önceki JPN_029 sayısı ile sonraki `affected_records` toplamı ayrıca eşitlik kontrolünden geçti; eksik sayım yok.

`JPN_006` için 11 feed `MEDIUM → INFO` geçti. V4 dosyası fiziksel olarak yok olan bu feed'lerde `review_required=true`; R5 skoru cezalandırılmadı. Boş, header-only, kolonsuz veya kullanılabilir kaydı olmayan dosya sentetik testlerde ORTA kaldı.

`JPN_027` V4 koşumunda tasarım gereği hiç üretilmedi. `JPN_031` de bu korpusta bulgu üretmedi; pozitif kapsamı aşağıdaki sentetik ve gerçek pipeline testleriyle doğrulandı. Korpus karşılaştırmasında yayınlanabilirlik ve yayın skoru değişmedi. Genel skor değişimi notice toplulaması ve INFO ücret dalıyla sınırlıydı: feed başına aralık `0`–`+0,5`, ortalama `+0,0973` puan. JPN_029 dışındaki bütün kuralların bulgu sayıları eşit kaldı.

## Büyük feed ve runtime kontrolü

En büyük Japon örneği `jbda-sankobus-sankobus`, aynı ZIP SHA-256 `c2ff89bf5f636ff07f2c3d20db454d872b967bc1731aba4b84594f7beb2e480b` ile ayrıca `/usr/bin/time -l` üzerinden ölçüldü:

| Ölçüm | Önce | Sonra |
|---|---:|---:|
| JPN_029 | 531.569 | 165 |
| JSON byte | 401.954.471 | 21.101.163 |
| Tepe RSS byte | 2.102.116.352 | 767.344.640 |
| Duvar süresi (tek ölçüm) | 2,93 s | 1,18 s |

Zaman ve RSS değerleri ölçülen makine/koşuma aittir; kalıcı bir performans garantisi değildir.

Japonya dışı büyük kontrol `mdb-865`, Auto profilinde aynı SHA-256 `8c52771818a93e084188d04f291ea2494f4a98da023ba59f8110a6a9484567d0` ile koşuldu. Bütün kural sayıları, genel skor `74,9`, yayın skoru `83,3`, yayın kararı ve çıktı boyutu `63.139.710 byte` önce/sonra aynı kaldı.

Bu kontrolün RSS'si koşumlar arasında dalgalandığı için iki binary dönüşümlü sırayla üçer kez tekrar ölçüldü (`rss-repeats.json`). Medyan süre `10,76 → 10,57 s`, medyan RSS `4.921.442.304 → 5.094.260.736 byte` (%3,5 artış) oldu. Tekrar aralıkları önce `4,57–5,52 GB`, sonra `4,92–5,49 GB`; bu örnek için bellek azalması iddia edilmiyor.

İki sentetik ZIP (`rail`, `zoned`) × üç profil (`auto`, `v3`, `v4`) için native/WASM32/WASM64/SDK karşılaştırıldı. WASM32/64 çıktıları tamamen eşit; dört runtime arasında notice semantiği, raporlar ve metrikler eşit. SDK'nın İngilizce metinleri ayrıca INFO/elle inceleme ve toplu satır sayısını koruyor. Son kaynaklardan yeniden derlenen bütün paketlerle altı senaryo geçti (`parity-final.log`).

## Kalıcı regresyon kanıtı

- `crates/pipeline/tests/jp_followup.rs`: 9 test; V3 `route_type`, Auto/V3/V4 kapısı, V4 ücret severity, bozuk ücret dosyası, kısmi çeviri, 50.000 tekrarlı headsign, hat/işletici/ücret kapsamı, belirsiz referanslar ve `location_type` sınırları.
- `crates/pipeline/tests/integration.rs`: mevcut stop-time bileşik kimlik testi toplu notice modeline uyarlandı.
- `crates/pipeline/tests/emit_proof.rs`: JPN_027 ve JPN_031 gerçek pipeline emit fixture'ları.
- `crates/wasm/src/i18n.rs`, CLI ve UI: profile + variant mesaj/remediation çözümlemesi; aggregate sayaç ve manual-review metni locale testleri.

## Son paket kontrolleri

- `cargo fmt --all -- --check` ve `git diff --check`: geçti.
- Workspace Clippy, bütün target/feature'lar ve `-D warnings`: geçti (`prepush-rust-final.log`).
- Locale export eşitliği ve UI testleri: 13 dosya, **118 test**, sıfır hata; satır kapsamı %94,21 (`ui-final.log`).
- WASM seri/threaded/memory64 derlemeleri, WASM32/64 eşitliği ve UI TypeScript/Vite üretim derlemesi: geçti (`wasm-sdk-final.log`).
- SDK yapılandırma eşitliği: **37 anahtar**; smoke ve paket kapısı: geçti. Son paket **936.592 byte packed / 2.603.319 byte unpacked**, 9 dosya. Boyut sınırları değiştirilmedi.
- Rust bağımlılık denetimi ve UI üretim bağımlılık denetimi: geçti (`precommit.log`).

Loglar üst dizindeki `tmp/jp-followup-20260914/` altında yereldir; derlenen çıktılar, ham korpus raporları ve geçici ölçüm araçları bu commit'e eklenmez. Push veya sürüm yayını yapılmadı.

## Kaynak sınırı

JPN_027, MLIT GTFS-JP V3'ün statik otobüs formatı olmasına ve `routes.txt.route_type` için otobüs işletmecisinin `3` ayarlamasına dayanır. V4'te bu otobüsle sınırlı kapsam kaldırıldığı için kural V4'e taşınmaz. `JPN_031`, V4 `stops.zone_id` koşullarını fare→route→trip→stop bağlantısı üzerinden muhafazakâr biçimde uygular; tarife dışından zone veya hat kapsamı tahmin etmez.

Resmî kaynaklar: [MLIT V3 nihai belge](https://www.mlit.go.jp/sogoseisaku/transport/content/001981046.docx), [MLIT V3→V4 revizyon açıklaması](https://www.mlit.go.jp/sogoseisaku/transport/content/001993769.pdf), [MLIT V4 PDF](https://www.mlit.go.jp/commmmons/document/007/commmmons_doc_007-01_ver01.pdf).
