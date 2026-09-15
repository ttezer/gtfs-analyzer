# GTFS-JP v3/v4 uyumluluk matrisi

Bu belge, GTFS Analyzer’ın GTFS-JP v3 kapsamını ve GTFS-JP v4 ile arasındaki farkları kayıt altına alır. Analyzer feed’in v3 veya v4 olduğunu otomatik olarak iddia etmez. `is_gtfs_jp` yalnız içerik sinyalidir; açık `v3`/`v4` seçimi sinyal yoksa doğrulamayı açmaz, yalnız uygulanacak kural kapsamını seçer. UI, CLI, SDK ve WASM'deki profil değeri seçilen doğrulama kapsamını gösterir; tam uyumluluk sertifikası değildir.

V3 kuralları geriye dönük uyumluluk için korunur. MLIT’nin 19 Mart 2026 tarihli v4 spesifikasyonu, v3’teki `agency_jp.txt`, `office_jp.txt` ve `pattern_jp.txt` dosyalarını ana standardın dışına çıkarıp v3 uzantıları için referans bölümüne taşır. Bu fark runtime’a işlendi: v4 profilinde bu dosyalara bağlı JPN kuralları çalışmaz; çeviri/kana ve temel GTFS-JP kontrolleri çalışmaya devam eder. V4’ün ana GTFS alanlarında değiştirdiği tüm zorunluluk sınıfları henüz “tam v4 uyumluluk rozeti” olarak ilan edilmiyor.

## Runtime profil kapısı

| Profil | Sürüm tespiti | `*_jp` uzantı kuralları | Çeviri/kana kuralları | Varsayılan |
|---|---|---|---|---|
| `auto` | Yapılmaz; yalnız GTFS-JP sinyali doğrulamayı açar | Mevcut v3/legacy davranışı | Genel JP/kana kuralları çalışır; sürüme özel yeni sabitler çalışmaz | Evet |
| `v3` | Kullanıcı statik otobüs profilini seçer; JP sinyali varsa doğrular | `JPN_002/003/005/012–018/020` çalışır | JPN_001/004/006–011/019/021/023–028/030/031 | Hayır |
| `v4` | Kullanıcı seçer; JP sinyali varsa doğrular | V3 uzantı kuralları çalışmaz | JPN_001/004/006–011/019/021/022–026/029/031 | Hayır |

CLI: `gtfs-analyzer validate feed.zip --gtfs-jp-profile v4`

JSON config delta: `{"gtfs_jp_profile":"v4"}`. WASM tarafında aynı alan mevcut config delta sözleşmesiyle verilir. Profil feed içeriğinden otomatik çıkarılmaz. Web ayarlarında `Auto / V3 / V4` seçilebilir. UI raporunda `GTFS-JP` tespit rozeti yanında seçilen kapsam rozeti gösterilir; bu rozet feed sürümünü değil analiz profilini ifade eder.

## 2026-08-25 açık V4 ölçümü

592 JP feed aynı manifestten, yalnızca `--gtfs-jp-profile v4` açık seçimiyle yeniden
koşturuldu. 588 sonuç `ok`, 2 sonuç `fatal`, 2 sonuç ise kısmi çıktı verdi; V4
 profilinde 585 feed GTFS-JP sinyali taşıdı. İki kritik ölçüm:

| Kural | Feed | Bulgu | Yorum |
|---|---:|---:|---|
| JPN_019 | 1 | 1 | Boş `record_sub_id` kullanan geçerli V4 çevirileri artık yanlış alarm üretmiyor. |
| JPN_022 | 16 | 16 | Alan bazlı dedup korunuyor; aynı dosya/alandaki çoklu eksiklikler tek feed özetiyle, tekil eksiklikler satır bilgisiyle raporlanıyor. |

Ham sonuç: `/Users/tacettintezer/GTFS/run14-artifacts/jp-v4-aggregated-20260825.summary.json`.
Bu ölçüm varsayılan davranışı değiştirmez; `auto` profili hâlâ v3/legacy davranışını
korur ve feed sürümünü otomatik iddia etmez.

2026-09-13 düzeltme paketinin aynı-arşiv Katori, 590 feed'lik Japonya korpusu ve
`mdb-865` performans sonuçları ayrıca [doğrulama kaydında](gtfs-jp-v3-v4-validation-2026-09-13.md)
yer alır.

Kaynaklar: [GTFS-JP v3 nihai resmî belgesi (Temmuz 2021)](https://www.mlit.go.jp/sogoseisaku/transport/content/001981046.docx), [MLIT eski sürümler arşivi](https://www.mlit.go.jp/sogoseisaku/transport/sosei_transport_tk_000067.html), [GTFS-JP v4 spesifikasyonu (19 Mart 2026)](https://www.mlit.go.jp/commmmons/document/007/), [V4 ana PDF](https://www.mlit.go.jp/commmmons/document/007/commmmons_doc_007-01_ver01.pdf), [v3-v4 fark belgesi](https://www.mlit.go.jp/commmmons/document/007/commmons_doc_007-03_ver01.pdf), [pattern_jp.txt rehberi](https://www.busdata.or.jp/gtfs_guide/08%E3%80%80pattern_jp-txt%EF%BC%88%E5%81%9C%E8%BB%8A%E3%83%91%E3%82%BF%E3%83%BC%E3%83%B3%E6%83%85%E5%A0%B1%EF%BC%89%E3%80%80%E3%80%90%E4%BB%BB%E6%84%8F%E3%80%91/).

## 2026-09-13 kaynak ve davranış kaydı (14 Eylül düzeltmeleriyle)

| Konu | Kaynak / sayfa | Önceki davranış | Bu pakette beklenen davranış |
|---|---|---|---|
| Açık profil kapısı | Ürün kararı; V3/V4 kullanıcı seçimi | Açık profil JP sinyali yokken de kuralları çalıştırıyordu | JP sinyali yoksa hiçbir `JPN_*` çalışmaz; profil yalnız sürüm kapsamını seçer |
| `feed_lang=ja` | V4 feed_info, s.29 | JP'ye özgü sabit kontrol yoktu | Tespit için geçerli `ja-*` etiketi kabul edilir; açık profilde JPN_023 yalnız tam `ja` değerini kabul eder (`ja-JP` bulgu üretir) |
| `agency_lang=ja`, `agency_timezone=Asia/Tokyo` | V4 agency, s.35 | Yalnız genel biçim kontrolleri | JPN_024/JPN_025, yalnız açık profil |
| `currency_type=JPY` | V4 fare_attributes, s.67 | Yalnız genel ISO kodu kontrolü | JPN_026; dosya yokluğu bu kuralın konusu değil |
| `location_type` | V4 stops, s.38; fark tablosu s.120 | Boş hücre JPN_022 sayılıyordu | Eksik kolon JPN_022; boş hücre geçerli `0 veya boş`; geçersiz enum STP_008 |
| V3 çeviri alanları | V3 nihai belge 2-14, s.33 | JPN_001/008/009/010 dışındaki alanlar eksikti | JPN_028 kana + JPN_030 `ja`; bir geçişte kurulan ödünç indeksler |
| V4 önerilen kalan okumalar | V4 translations, s.77 | İlk paket satır başına notice üretiyordu | JPN_029, Düşük/Quality; tablo+alan+kaynak değeri başına K4 toplulaması, eksik kayıt sayısı ve en fazla beş örnek |
| Kana içeriği | V4 translations, s.75–77 | Kanji tek başına okuma sayılıyordu; yarım genişlik Katakana yoktu | Hiragana/Katakana gerekir; yarım genişlik Katakana kabul edilir |
| Japonca metin tespiti | Katalog ölçümü: 590 JP feed + 3.103 negatif örnek | Kanji tek başına kullanılırsa Çin/Tayvan feed'leri de açılıyordu | `agency_name` veya `stop_name` içinde en az bir kana; beş kaçan JP feed'inin tamamı yakalandı, ölçülen negatiflerde yanlış pozitif çıkmadı |
| Ücret istisnası | V4 s.20/22/68 | İstisna mesajı doğru olsa da dosya yokluğu Orta puan kaybıydı | JPN_006: V4 fiziksel yokluk BİLGİ/elle inceleme, sıfır ceza; mevcut boş/bozuk dosya Orta; V3/Auto Orta |
| V3 otobüs `route_type=3` | V3 nihai belge routes.txt; MLIT V3→V4 revizyon açıklaması | JPN_027 önce `route_type=3` ankrajı ve otobüs çoğunluğu sezgisi kullanıyordu | **Düzeltildi:** JPN_027 ortak JP tespit kapısından sonra açık V3'te her sayısal `route_type != 3` değeri için tip başına tek bulgu üretir; `700..716` ve `800` de V3 ihlalidir. Auto/V4 sessiz |
| Koşullu `zone_id` | V3 stops/ücret örnekleri; V4 stops.zone_id ve補足3 | JP koşullu zorunluluk yoktu | JPN_031, V3/V4 Yüksek/Interop; fare→route→trip→stop kapsamı. Tek ücretli/ilgisiz duraklarda bulgu yok |

14 Eylül kapsamı ve ölçümleri [takip doğrulama kaydında](gtfs-jp-followup-2026-09-14.md) tutulur.
V3 otobüs sınırının kaldırılması [MLIT revizyon açıklamasında](https://www.mlit.go.jp/sogoseisaku/transport/content/001993769.pdf) ayrıca açıklanır; nihai hükümler için V3 ve V4 belgeleri esastır.

JPN_031 tarifeyi dışarıdan tahmin etmez. Açık hat referansı olmayan bölge kuralı için tekil işletici kapsamı **ve hizmet verilen bir durakta bölge eşleşmesi** gerekir; açık hat-içi tek ücret kuralı bu çıkarımı sınırlar. Belirsiz işletici, bozuk referans veya bütün bölge bağlantılarının eksikliği halinde kanıtsız hat kapsamı genişletilmez. Ayrıntılar [JPN_031 kartında](rules/JPN/JPN_031.md).

| Dosya / alan | v3 durumu | v4 durumu | Zorunluluk seviyesi | Kural | Sınıf | Kaynak | Test senaryosu |
|---|---|---|---|---|---|---|---|
| `agency_jp.txt` | Profil dosyası; mevcutsa işleticinin Japonya-özel bilgileri | Ana v4 standardından çıkarıldı; v3 uzantısı olarak referans bölümünde | Opsiyonel dosya | v3/auto: JPN_003, JPN_012, JPN_013; v4: — | Interop / Quality | format reference / v4 farkı | Profil başına dosya mevcut ve hatalı |
| `agency_jp.agency_id` | `agency.txt` kimliğine bağlanan zorunlu alan | V4 ana standardında yok; v3 alanı | Dosya mevcutsa zorunlu | v3/auto: JPN_012; v4: — | Interop | format reference / v4 farkı | Profil başına boş değer sonucu |
| `agency_jp.agency_zip_number` | Varsa 7 ASCII rakam | V4 ana standardında yok; v3 alanı | Opsiyonel alan; mevcutsa biçim | v3/auto: JPN_013; v4: — | Quality | format reference / v4 farkı | Profil başına biçim sonucu |
| `office_jp.txt` | Ofis bilgileri; dosya opsiyonel | Ana v4 standardından çıkarıldı; v3 uzantısı olarak referans bölümünde | Opsiyonel dosya | v3/auto: JPN_002, JPN_005, JPN_014, JPN_020; v4: — | Interop / Quality | format reference / v4 farkı | Profil başına dosya mevcut ve hatalı |
| `office_jp.office_id` | Birincil anahtar; mevcut satırda dolu ve tekil | V4 ana standardında yok; v3 alanı | Dosya mevcutsa zorunlu ve tekil | v3/auto: JPN_014; v4: — | Interop | format reference / v4 farkı | Profil başına boş ve tekrar eden kimlik |
| `office_jp.office_name` | Mevcut `office_id` için zorunlu | V4 ana standardında yok; v3 alanı | Dosya mevcutsa zorunlu | v3/auto: JPN_005; v4: — | Interop | format reference / v4 farkı | Profil başına boş isim |
| `office_jp.office_url` | Varsa HTTP(S) biçim kalite kontrolü | V4 ana standardında yok; v3 alanı | Opsiyonel; mevcutsa biçim | v3/auto: JPN_020; v4: — | Quality | format reference / v4 farkı | Profil başına URL sonucu |
| `office_jp.office_phone` | Varsa temel telefon biçim kalite kontrolü | V4 ana standardında yok; v3 alanı | Opsiyonel; mevcutsa biçim | v3/auto: JPN_020; v4: — | Quality | format reference / v4 farkı | Profil başına telefon sonucu |
| `routes_jp.txt` | v3'te yok; eski v2 feed'leri için parser/sinyal ve legacy JPN_015/JPN_016 korunur | V4 ana standardında yok | Legacy uyumluluk | v3/auto: JPN_015, JPN_016; v4: — | Interop / Quality | [v3 nihai belge](https://www.mlit.go.jp/sogoseisaku/transport/content/001981046.docx) / v4 farkı | Profil başına eski dosyanın sonucu |
| `pattern_jp.txt` | Opsiyonel duruş paterni dosyası | Ana v4 standardından çıkarıldı; v3 uzantısı olarak referans bölümünde | Opsiyonel dosya | v3/auto: JPN_017, JPN_018; v4: — | Interop | pattern rehberi / v4 farkı | V4'te masterless `jp_pattern_id` kabul edilir |
| `pattern_jp.jp_pattern_id` | Dosya mevcutsa zorunlu ve tekil | V4 ana standardında `pattern_jp` master'ı yok; v4'teki `jp_pattern_id` alanıyla aynı ilişki varsayılmaz | Dosya mevcutsa zorunlu | v3/auto: JPN_017; v4: — | Interop | pattern rehberi / v4 farkı | Profil başına eksik ve tekrar eden kimlik |
| `pattern_jp.route_update_date` | Varsa geçerli `YYYYMMDD` | V4 ana standardında yok; v3/legacy alanı | Opsiyonel; mevcutsa biçim | v3/auto: JPN_016; v4: — | Quality | v3 PDF / pattern rehberi / v4 farkı | Profil başına tarih sonucu |
| `trips.jp_pattern_id` | `pattern_jp.txt` mevcutsa `pattern_jp.jp_pattern_id` referansı; dosya yokken alan opsiyonel/iç kod olabilir | V4'te opsiyonel JP alanı korunur; `pattern_jp` master'ı v4 standardında olmadığı için foreign key uygulanmaz | Opsiyonel alan | v3/auto: JPN_018; v4: — | Interop | trips rehberi / v4 farkı | V4'te master dosyası olmadan değer kabul edilir |
| `shapes.txt` / `trips.shape_id` | `shapes.txt` opsiyonel; `shape_id` normal GTFS ilişkisi içinde kullanılır | Continuous pickup/drop-off aktifse `shape_id` koşullu zorunlu; sabit rotalarda önerilir. `shapes.txt` dosyasının yokluğu tek başına hata değildir | Koşullu zorunlu / önerilen | TRP_019: continuous aktif + boş `shape_id`; TRP_004: mevcut ID için FK | Spec | [GTFS-JP v4 farkı](https://www.mlit.go.jp/commmmons/document/007/commmons_doc_007-03_ver01.pdf) / [GTFS-JP format referansı](https://www.gtfs.jp/developpers-guide/format-reference.html) | `continuous_*` 0/2/3 + boş `shape_id` bulgu üretir; 1/boş değer sessiz kalır |
| `transfers.txt` | Opsiyonel | Önerilir; dosyanın yokluğu zorunlu hata değildir | Önerilen | Dosya mevcutsa TRF ailesi bütünlük kontrolleri | Quality / Interop | [GTFS-JP v4 farkı](https://www.mlit.go.jp/commmmons/document/007/commmons_doc_007-03_ver01.pdf) / [GTFS-JP format referansı](https://www.gtfs.jp/developpers-guide/format-reference.html) | Dosya yokken ceza yok; mevcut dosyada geçersiz durak/sefer referansı kontrol edilir |
| `translations.txt` kana satırları | `ja-Hrkt` okumaları ve GTFS-JP v3 referans bütünlüğü | V4'te standart translations dosyasıdır; stop_name okuması zorunlu, yıldızlı alanlar önerilir | Profil kurallarına göre | JPN_001, JPN_008–010, JPN_019, JPN_021, JPN_028–030 | Quality / Interop | V3 s.33 / V4 s.75–77 | `record_id`, `field_value`, anahtarsız feed_info ve stop_times bileşik anahtarı; V3'te tablo+alan+kaynak değerine göre toplu notice |
| `stops.location_type` | V3'te opsiyonel; boş değer normal durak gibi yorumlanır | V4'te kolon zorunlu; geçerli değerlerden biri `0 veya boş` | V4'te zorunlu | JPN_022 eksik kolon; STP_008 enum/biçim | Interop / Spec | V4 s.38/120 | Eksik kolon JPN_022; boş hücre sessiz; non-numeric/enum dışı STP_008 |
| `stops.parent_station` | V3'te opsiyonel; hiyerarşi kısıtları koşullu | V4'te location type 2/3/4 için koşullu zorunlu; parent türü ve istasyon hiyerarşisi korunur | Koşullu zorunlu | STP_009/010/011/021/032/036 | Spec | v4 farkı / GTFS Reference | `location_type=2/3/4` + boş parent; yanlış parent türü; istasyonun parent taşıması |
| `feed_info` / `agency` ana alanları | V3'te `feed_start_date`, `feed_end_date`, `feed_version` opsiyonel; Japonya sabitleri uygulanır | V4'te tarihler/sürüm ve `agency_lang` zorunlu; `feed_lang=ja`, `agency_timezone=Asia/Tokyo` | Açık profile göre | JPN_022–025; FIN_005/006/007; AGN_006 | Interop / Quality / Spec | V4 s.29/35/120 | Eksik, boş, geçersiz ve yanlış sabit değer ayrı test edilir |
| `fare_attributes.agency_id` | V3'te alan standardın bu sürümünde yok | Birden fazla agency tanımlıysa koşullu zorunlu | Koşullu zorunlu | AGN_011 eksiklik; FAR_008 foreign key | Spec | v4 farkı / GTFS Reference | Tek agency'de boşluk uyarı/öneri; çoklu agency'de eksiklik bulgusu; hatalı ID FK bulgusu |
| `stop_times` Flex alanları | V3'te yok veya sınırlı kullanım | `start/end_pickup_drop_off_window` Flex lokasyonuyla koşullu zorunlu; arrival/departure ve pickup/drop-off alanlarında koşullu yasaklar uygulanır | Koşullu zorunlu / koşullu yasak | STM_037–041, STM_051–055, STM_058; RTS_028 | Spec / Interop | v4 farkı / GTFS Reference | Lokasyon + eksik pencere; pencere + arrival/departure; pencere + yasak pickup/drop-off; rota düzeyi continuous çelişkisi |
| `jp_parent_route_id` | Tanınır; otomatik `route_id` foreign key sayılmaz | V4'te isteğe bağlı JP alanı korunur; rota gruplama anlamı açıkça tarif edilir | Opsiyonel alan | - | - | v3/v4 farkı | Değerin varlığı tek başına bulgu üretmez |
| `jp_trip_desc` | Tanınır; Japonca değer V3 çeviri çiftine girer | V4'te isteğe bağlı JP alanı korunur | Opsiyonel alan | V3: JPN_028/JPN_030 | Quality | V3 s.18/33 | Seyrek trip side-map'i; özel biçim icat edilmez |
| `jp_trip_desc_symbol` | Tanınır; spesifikasyonda olmayan regex uygulanmaz | V4'te isteğe bağlı JP alanı korunur | Opsiyonel alan | - | - | v3/v4 farkı | Özel biçim icat edilmez |

## Zorunluluk ve skor politikası

- Opsiyonel dosyanın yokluğu tek başına analiz skorunu veya yayın engelini değiştirmez.
- Opsiyonel dosya mevcutsa hatalı kimlik, tarih veya biçim Interop/Quality seviyesinde raporlanabilir.
- Rapor GTFS-JP tespiti yapar ve seçilen kural profilini taşır; `v3`/`v4` feed sürümü iddiası üretmez.
- Varsayılan `auto` profilinde mevcut legacy/v3 davranışı korunur. `v4` profilinde `agency_jp.txt`, `office_jp.txt` ve `pattern_jp.txt` referans verisi olarak okunabilir ama ilgili v3 bulguları üretilmez. Bu seçim feed'in sürümünü otomatik kanıtlamaz.
- `pattern_jp.txt` içindeki `origin_stop`, `via_stop` ve `destination_stop` açıklayıcı metindir; `stop_id` foreign key'i değildir.
- `translations.txt` içinde `record_sub_id` yalnızca `stop_times` için kullanılır. `agency`, `stops`, `routes` ve `trips` satırlarında alan boş bırakılmalıdır; `NONE` gerçek bir alt kimlik olmadığı için V4'te geçersizdir. `stop_times` için gerçek `stop_sequence` gerekir.

## V4'ün kalan kapsamı

MLIT v4 belgesinin uzantı dosyası, `jp_pattern_id` farkı, translations alt kimlik semantiği, ana alan zorunlulukları ve `shapes`/`transfers` koşulları runtime/dokümantasyona alındı. Bu paketle Japonya sabitleri ve hedeflenen çeviri boşlukları da eklendi. `shapes` için mevcut TRP_019 koşullu zorunluluğu uygular; `transfers` yalnızca öneri olarak belgelenir ve yokluğu cezalandırılmaz. Tam v4 uyumluluk iddiası için sonraki sprintte:

1. V4 teknik rehberindeki uygulama rehberleri ve öneri alanlarını ayrı kalite kapsamı olarak değerlendirmek,
2. Bu kapsamın tamamı için üretici çeşitliliğini temsil eden ek fixture/korpus doğrulaması yapmak

gerekecek. UI’daki `GTFS-JP V4` rozeti “v4 uyumlu” anlamına gelmez; `--gtfs-jp-profile v4` yalnız kodlanmış v4 kapsamını açıkça seçer.
