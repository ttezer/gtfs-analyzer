# GTFS-JP v3/v4 uyumluluk matrisi

Bu belge, GTFS Analyzer’ın GTFS-JP v3 kapsamını ve GTFS-JP v4 ile arasındaki farkları kayıt altına alır. Analyzer feed’in v3 veya v4 olduğunu otomatik olarak iddia etmez; runtime yalnızca GTFS-JP sinyali üretir. Kural kapsamı açık profil seçimiyle kontrol edilir: varsayılan `auto` profilidir, `v3` eski Japonya-özel uzantıları doğrular, `v4` ise yeni V4 kapsamını açıkça seçer. UI ve SDK’daki profil rozeti yalnızca bu seçimi gösterir; tam V4 uyumluluk sertifikası değildir.

V3 kuralları geriye dönük uyumluluk için korunur. MLIT’nin 19 Mart 2026 tarihli v4 spesifikasyonu, v3’teki `agency_jp.txt`, `office_jp.txt` ve `pattern_jp.txt` dosyalarını ana standardın dışına çıkarıp v3 uzantıları için referans bölümüne taşır. Bu fark runtime’a işlendi: v4 profilinde bu dosyalara bağlı JPN kuralları çalışmaz; çeviri/kana ve temel GTFS-JP kontrolleri çalışmaya devam eder. V4’ün ana GTFS alanlarında değiştirdiği tüm zorunluluk sınıfları henüz “tam v4 uyumluluk rozeti” olarak ilan edilmiyor.

## Runtime profil kapısı

| Profil | Sürüm tespiti | `*_jp` uzantı kuralları | Çeviri/kana kuralları | Varsayılan |
|---|---|---|---|---|
| `auto` | Yapılmaz; yalnız GTFS-JP sinyali | Mevcut v3/legacy davranışı | Çalışır | Evet |
| `v3` | Kullanıcı seçer | `JPN_002/003/005/012–018/020` çalışır | Çalışır | Hayır |
| `v4` | Kullanıcı seçer | Bu uzantılar referans kapsamıdır; yukarıdaki kurallar çalışmaz | `JPN_001/004/006–011/019/021/022` çalışır | Hayır |

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

Kaynaklar: [GTFS-JP v3 resmî arşiv PDF'i](https://www.mlit.go.jp/sogoseisaku/transport/content/001981081.pdf), [GTFS-JP format referansı](https://www.gtfs.jp/developpers-guide/format-reference.html), [pattern_jp.txt rehberi](https://www.busdata.or.jp/gtfs_guide/08%E3%80%80pattern_jp-txt%EF%BC%88%E5%81%9C%E8%BB%8A%E3%83%91%E3%82%BF%E3%83%BC%E3%83%B3%E6%83%85%E5%A0%B1%EF%BC%89%E3%80%80%E3%80%90%E4%BB%BB%E6%84%8F%E3%80%91/), [GTFS-JP v4 spesifikasyonu](https://www.mlit.go.jp/commmmons/document/007/), [v3-v4 fark belgesi](https://www.mlit.go.jp/commmmons/document/007/commmons_doc_007-03_ver01.pdf).

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
| `routes_jp.txt` | v3'te yok; eski v2 feed'leri için parser/sinyal ve legacy JPN_015/JPN_016 korunur | V4 ana standardında yok | Legacy uyumluluk | v3/auto: JPN_015, JPN_016; v4: — | Interop / Quality | [v3 PDF](https://www.mlit.go.jp/sogoseisaku/transport/content/001981081.pdf) / v4 farkı | Profil başına eski dosyanın sonucu |
| `pattern_jp.txt` | Opsiyonel duruş paterni dosyası | Ana v4 standardından çıkarıldı; v3 uzantısı olarak referans bölümünde | Opsiyonel dosya | v3/auto: JPN_017, JPN_018; v4: — | Interop | pattern rehberi / v4 farkı | V4'te masterless `jp_pattern_id` kabul edilir |
| `pattern_jp.jp_pattern_id` | Dosya mevcutsa zorunlu ve tekil | V4 ana standardında `pattern_jp` master'ı yok; v4'teki `jp_pattern_id` alanıyla aynı ilişki varsayılmaz | Dosya mevcutsa zorunlu | v3/auto: JPN_017; v4: — | Interop | pattern rehberi / v4 farkı | Profil başına eksik ve tekrar eden kimlik |
| `pattern_jp.route_update_date` | Varsa geçerli `YYYYMMDD` | V4 ana standardında yok; v3/legacy alanı | Opsiyonel; mevcutsa biçim | v3/auto: JPN_016; v4: — | Quality | v3 PDF / pattern rehberi / v4 farkı | Profil başına tarih sonucu |
| `trips.jp_pattern_id` | `pattern_jp.txt` mevcutsa `pattern_jp.jp_pattern_id` referansı; dosya yokken alan opsiyonel/iç kod olabilir | V4'te opsiyonel JP alanı korunur; `pattern_jp` master'ı v4 standardında olmadığı için foreign key uygulanmaz | Opsiyonel alan | v3/auto: JPN_018; v4: — | Interop | trips rehberi / v4 farkı | V4'te master dosyası olmadan değer kabul edilir |
| `shapes.txt` / `trips.shape_id` | `shapes.txt` opsiyonel; `shape_id` normal GTFS ilişkisi içinde kullanılır | Continuous pickup/drop-off aktifse `shape_id` koşullu zorunlu; sabit rotalarda önerilir. `shapes.txt` dosyasının yokluğu tek başına hata değildir | Koşullu zorunlu / önerilen | TRP_019: continuous aktif + boş `shape_id`; TRP_004: mevcut ID için FK | Spec | [GTFS-JP v4 farkı](https://www.mlit.go.jp/commmmons/document/007/commmons_doc_007-03_ver01.pdf) / [GTFS-JP format referansı](https://www.gtfs.jp/developpers-guide/format-reference.html) | `continuous_*` 0/2/3 + boş `shape_id` bulgu üretir; 1/boş değer sessiz kalır |
| `transfers.txt` | Opsiyonel | Önerilir; dosyanın yokluğu zorunlu hata değildir | Önerilen | Dosya mevcutsa TRF ailesi bütünlük kontrolleri | Quality / Interop | [GTFS-JP v4 farkı](https://www.mlit.go.jp/commmmons/document/007/commmons_doc_007-03_ver01.pdf) / [GTFS-JP format referansı](https://www.gtfs.jp/developpers-guide/format-reference.html) | Dosya yokken ceza yok; mevcut dosyada geçersiz durak/sefer referansı kontrol edilir |
| `translations.txt` kana satırları | `ja-Hrkt` okumaları ve GTFS-JP v3 referans bütünlüğü | V4'te standart translations dosyasıdır; `ja-Hrkt` okuması zorunlu, tablo ve alt kimlik kuralları genişletilmiştir | Profil kurallarına göre | JPN_001, JPN_008–010, JPN_019, JPN_021 | Quality / Interop | format reference / v4 farkı | V3 kana eksikliği, V4 `record_sub_id` semantiği, geçersiz kayıt ve çelişki |
| `stops.location_type` | V3'te opsiyonel; boş değer normal durak gibi yorumlanır | V4'te zorunlu; değer 0–4 enumundan biri olmalı | V4'te zorunlu | JPN_022 eksik/boş alan; STP_008 enum/biçim | Interop / Spec | v4 farkı / [GTFS-JP format referansı](https://www.gtfs.jp/developpers-guide/format-reference.html) | V4 JP feed'inde `location_type` sütunu veya satır değeri boş; non-numeric/enum dışı değer STP_008 |
| `stops.parent_station` | V3'te opsiyonel; hiyerarşi kısıtları koşullu | V4'te location type 2/3/4 için koşullu zorunlu; parent türü ve istasyon hiyerarşisi korunur | Koşullu zorunlu | STP_009/010/011/021/032/036 | Spec | v4 farkı / GTFS Reference | `location_type=2/3/4` + boş parent; yanlış parent türü; istasyonun parent taşıması |
| `feed_info` / `agency` ana alanları | V3'te `feed_start_date`, `feed_end_date`, `feed_version` opsiyonel; `agency_lang` sabit JP alanı | V4'te `feed_start_date`, `feed_end_date`, `feed_version` ve `agency_lang` zorunlu | V4'te zorunlu | JPN_022; FIN_005/006/007; AGN_006 | Interop / Quality / Spec | v4 farkı | V4 JP feed'inde dört alan eksik; dolu ama biçimi hatalı |
| `fare_attributes.agency_id` | V3'te alan standardın bu sürümünde yok | Birden fazla agency tanımlıysa koşullu zorunlu | Koşullu zorunlu | AGN_011 eksiklik; FAR_008 foreign key | Spec | v4 farkı / GTFS Reference | Tek agency'de boşluk uyarı/öneri; çoklu agency'de eksiklik bulgusu; hatalı ID FK bulgusu |
| `stop_times` Flex alanları | V3'te yok veya sınırlı kullanım | `start/end_pickup_drop_off_window` Flex lokasyonuyla koşullu zorunlu; arrival/departure ve pickup/drop-off alanlarında koşullu yasaklar uygulanır | Koşullu zorunlu / koşullu yasak | STM_037–041, STM_051–055, STM_058; RTS_028 | Spec / Interop | v4 farkı / GTFS Reference | Lokasyon + eksik pencere; pencere + arrival/departure; pencere + yasak pickup/drop-off; rota düzeyi continuous çelişkisi |
| `jp_parent_route_id` | Tanınır; otomatik `route_id` foreign key sayılmaz | V4'te isteğe bağlı JP alanı korunur; rota gruplama anlamı açıkça tarif edilir | Opsiyonel alan | - | - | v3/v4 farkı | Değerin varlığı tek başına bulgu üretmez |
| `jp_trip_desc` | Tanınır; spesifikasyonda olmayan regex uygulanmaz | V4'te isteğe bağlı JP alanı korunur | Opsiyonel alan | - | - | v3/v4 farkı | Özel biçim icat edilmez |
| `jp_trip_desc_symbol` | Tanınır; spesifikasyonda olmayan regex uygulanmaz | V4'te isteğe bağlı JP alanı korunur | Opsiyonel alan | - | - | v3/v4 farkı | Özel biçim icat edilmez |

## Zorunluluk ve skor politikası

- Opsiyonel dosyanın yokluğu tek başına analiz skorunu veya yayın engelini değiştirmez.
- Opsiyonel dosya mevcutsa hatalı kimlik, tarih veya biçim Interop/Quality seviyesinde raporlanabilir.
- Rapor GTFS-JP tespiti yapar ve seçilen kural profilini taşır; `v3`/`v4` feed sürümü iddiası üretmez.
- Varsayılan `auto` profilinde mevcut legacy/v3 davranışı korunur. `v4` profilinde `agency_jp.txt`, `office_jp.txt` ve `pattern_jp.txt` referans verisi olarak okunabilir ama ilgili v3 bulguları üretilmez. Bu seçim feed'in sürümünü otomatik kanıtlamaz.
- `pattern_jp.txt` içindeki `origin_stop`, `via_stop` ve `destination_stop` açıklayıcı metindir; `stop_id` foreign key'i değildir.
- `translations.txt` içinde `record_sub_id` yalnızca `stop_times` için kullanılır. `agency`, `stops`, `routes` ve `trips` satırlarında alan boş bırakılmalıdır; `NONE` gerçek bir alt kimlik olmadığı için V4'te geçersizdir. `stop_times` için gerçek `stop_sequence` gerekir.

## V4'ün kalan kapsamı

MLIT v4 belgesinin uzantı dosyası, `jp_pattern_id` farkı, translations alt kimlik semantiği, ana alan zorunlulukları ve `shapes`/`transfers` koşulları runtime/dokümantasyona alındı. `shapes` için mevcut TRP_019 koşullu zorunluluğu uygular; `transfers` yalnızca öneri olarak belgelenir ve yokluğu cezalandırılmaz. Tam v4 uyumluluk iddiası için sonraki sprintte:

1. V4 teknik rehberindeki uygulama rehberleri ve öneri alanlarını ayrı kalite kapsamı olarak değerlendirmek,
2. Bu kapsamın tamamı için üretici çeşitliliğini temsil eden ek fixture/korpus doğrulaması yapmak

gerekecek. UI’daki `GTFS-JP V4` rozeti “v4 uyumlu” anlamına gelmez; `--gtfs-jp-profile v4` yalnız kodlanmış v4 kapsamını açıkça seçer.
