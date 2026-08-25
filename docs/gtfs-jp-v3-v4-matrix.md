# GTFS-JP v3/v4 uyumluluk matrisi

Bu belge, GTFS Analyzer’ın GTFS-JP v3 kapsamını ve GTFS-JP v4 ile arasındaki farkları kayıt altına alır. Analyzer feed’in v3 veya v4 olduğunu otomatik olarak iddia etmez; runtime yalnızca GTFS-JP sinyali üretir. Kural kapsamı artık açık profil seçimiyle kontrol edilir: `auto` mevcut davranışı korur, `v3` eski Japonya-özel uzantıları doğrular, `v4` ise v3 uzantı dosyalarını referans kapsamı olarak bırakır.

v3 satırları mevcut uygulama hedefidir. MLIT’nin 19 Mart 2026 tarihli v4 spesifikasyonu, v3’teki `agency_jp.txt`, `office_jp.txt` ve `pattern_jp.txt` dosyalarını ana standardın dışına çıkarıp v3 uzantıları için referans bölümüne taşır. Bu fark runtime’a işlendi: v4 profilinde bu dosyalara bağlı JPN kuralları çalışmaz; çeviri/kana ve temel GTFS-JP kontrolleri çalışmaya devam eder. V4’ün ana GTFS alanlarında değiştirdiği tüm zorunluluk sınıfları henüz “tam v4 uyumluluk rozeti” olarak ilan edilmiyor.

## Runtime profil kapısı

| Profil | Sürüm tespiti | `*_jp` uzantı kuralları | Çeviri/kana kuralları | Varsayılan |
|---|---|---|---|---|
| `auto` | Yapılmaz; yalnız GTFS-JP sinyali | Mevcut v3/legacy davranışı | Çalışır | Evet |
| `v3` | Kullanıcı seçer | `JPN_002/003/005/012–018/020` çalışır | Çalışır | Hayır |
| `v4` | Kullanıcı seçer | Bu uzantılar referans kapsamıdır; yukarıdaki kurallar çalışmaz | `JPN_001/004/006–011/019/021` çalışır | Hayır |

CLI: `gtfs-analyzer validate feed.zip --gtfs-jp-profile v4`

JSON config delta: `{"gtfs_jp_profile":"v4"}`. WASM tarafında aynı alan mevcut config delta sözleşmesiyle verilir. Profil feed içeriğinden otomatik çıkarılmaz.

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
| `translations.txt` kana satırları | `ja-Hrkt` okumaları ve GTFS-JP v3 referans bütünlüğü | V4'te standart translations dosyasıdır; `ja-Hrkt` okuması zorunlu, tablo ve alt kimlik kuralları genişletilmiştir | Profil kurallarına göre | JPN_001, JPN_008–010, JPN_019, JPN_021 | Quality / Interop | format reference / v4 farkı | V3 kana eksikliği, geçersiz kayıt ve çelişki |
| `jp_parent_route_id` | Tanınır; otomatik `route_id` foreign key sayılmaz | V4'te isteğe bağlı JP alanı korunur; rota gruplama anlamı açıkça tarif edilir | Opsiyonel alan | - | - | v3/v4 farkı | Değerin varlığı tek başına bulgu üretmez |
| `jp_trip_desc` | Tanınır; spesifikasyonda olmayan regex uygulanmaz | V4'te isteğe bağlı JP alanı korunur | Opsiyonel alan | - | - | v3/v4 farkı | Özel biçim icat edilmez |
| `jp_trip_desc_symbol` | Tanınır; spesifikasyonda olmayan regex uygulanmaz | V4'te isteğe bağlı JP alanı korunur | Opsiyonel alan | - | - | v3/v4 farkı | Özel biçim icat edilmez |

## Zorunluluk ve skor politikası

- Opsiyonel dosyanın yokluğu tek başına analiz skorunu veya yayın engelini değiştirmez.
- Opsiyonel dosya mevcutsa hatalı kimlik, tarih veya biçim Interop/Quality seviyesinde raporlanabilir.
- Rapor GTFS-JP tespiti yapar; `v3`/`v4` sürüm rozeti üretmez.
- `auto` ve `v3` profillerinde `agency_jp.txt`, `office_jp.txt` ve `pattern_jp.txt` mevcutsa legacy/v3 kuralları çalışır; `v4` profilinde bu dosyalar referans verisi olarak okunabilir ama ilgili v3 bulguları üretilmez. Bu seçim feed'in sürümünü otomatik kanıtlamaz.
- `pattern_jp.txt` içindeki `origin_stop`, `via_stop` ve `destination_stop` açıklayıcı metindir; `stop_id` foreign key'i değildir.
- `translations.txt` içinde GTFS-JP v3'ün kullandığı `record_sub_id=NONE`, alt kimlik yok anlamında kabul edilir; `stop_times` için gerçek `stop_sequence` gerekir.

## V4'ün kalan kapsamı

MLIT v4 belgesinin uzantı dosyası ve `jp_pattern_id` farkı runtime’a alındı. Tam v4 uyumluluk iddiası için sonraki sprintte:

1. v4'te ana standarda alınan/eklenen GTFS dosyaları ve alanları için ayrı kapsam matrisi ve kaynak çapasını oluşturmak,
2. v4 translations kurallarını v3 `record_sub_id=NONE` davranışından ayrı modellemek,
3. `jp_pattern_id` alanının v4'teki master'sız kullanımını v3 JPN_017/JPN_018'den ayırmak,
4. v4 ana standardındaki değişen zorunluluk sınıfları (`feed_info` tarih/sürüm alanları, `agency_lang`, koşullu `shapes`/`transfers` ve yeni standart dosyaları) için ayrı kural/fixture seti eklemek

gerekecek. Bu işler tamamlanmadan UI’da “v4 uyumlu” rozeti üretilmeyecek; `--gtfs-jp-profile v4` yalnız kodlanmış v4 kapsamını açıkça seçer.
