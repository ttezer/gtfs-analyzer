# GTFS-JP v3/v4 uyumluluk matrisi

Bu belge, GTFS Analyzer’ın GTFS-JP v3 kapsamını ve GTFS-JP v4 ile arasındaki farkları kayıt altına alır. Analyzer feed’in v3 veya v4 olduğunu otomatik olarak iddia etmez; runtime yalnızca GTFS-JP sinyali üretir. Mevcut JPN kuralları v3 profil kurallarıdır ve v4 uyumluluğu iddiasıyla koşulsuz çalıştırılmaz.

v3 satırları mevcut uygulama hedefidir. MLIT’nin 19 Mart 2026 tarihli v4 spesifikasyonu, v3’teki `agency_jp.txt`, `office_jp.txt` ve `pattern_jp.txt` dosyalarını ana standardın dışına çıkarıp v3 uzantıları için referans bölümüne taşır. Bu nedenle v4 sütunu artık “doğrulanacak” değil, kodlanmamış fark kaydıdır; v4 runtime kuralları bu sprintte eklenmemiştir.

Kaynaklar: [GTFS-JP v3 resmî arşiv PDF'i](https://www.mlit.go.jp/sogoseisaku/transport/content/001981081.pdf), [GTFS-JP format referansı](https://www.gtfs.jp/developpers-guide/format-reference.html), [pattern_jp.txt rehberi](https://www.busdata.or.jp/gtfs_guide/08%E3%80%80pattern_jp-txt%EF%BC%88%E5%81%9C%E8%BB%8A%E3%83%91%E3%82%BF%E3%83%BC%E3%83%B3%E6%83%85%E5%A0%B1%EF%BC%89%E3%80%80%E3%80%90%E4%BB%BB%E6%84%8F%E3%80%91/), [GTFS-JP v4 spesifikasyonu](https://www.mlit.go.jp/commmmons/document/007/), [v3-v4 fark belgesi](https://www.mlit.go.jp/commmmons/document/007/commmons_doc_007-03_ver01.pdf).

| Dosya / alan | v3 durumu | v4 durumu | Zorunluluk seviyesi | Kural | Sınıf | Kaynak | Test senaryosu |
|---|---|---|---|---|---|---|---|
| `agency_jp.txt` | Profil dosyası; mevcutsa işleticinin Japonya-özel bilgileri | Ana v4 standardından çıkarıldı; v3 uzantısı olarak referans bölümünde | Opsiyonel dosya | JPN_003, JPN_012, JPN_013 | Interop / Quality | format reference / v4 farkı | Dosya yok; geçerli satır; eksik kimlik; hatalı posta kodu |
| `agency_jp.agency_id` | `agency.txt` kimliğine bağlanan zorunlu alan | V4 ana standardında yok; v3 alanı | Dosya mevcutsa zorunlu | JPN_012 | Interop | format reference / v4 farkı | Boş değer bulgu üretir |
| `agency_jp.agency_zip_number` | Varsa 7 ASCII rakam | V4 ana standardında yok; v3 alanı | Opsiyonel alan; mevcutsa biçim | JPN_013 | Quality | format reference / v4 farkı | `1234567` geçer; `123-4567` kalır |
| `office_jp.txt` | Ofis bilgileri; dosya opsiyonel | Ana v4 standardından çıkarıldı; v3 uzantısı olarak referans bölümünde | Opsiyonel dosya | JPN_002, JPN_005, JPN_014, JPN_020 | Interop / Quality | format reference / v4 farkı | Dosya yok; eksik/tekrarlı kimlik; biçim hatası |
| `office_jp.office_id` | Birincil anahtar; mevcut satırda dolu ve tekil | V4 ana standardında yok; v3 alanı | Dosya mevcutsa zorunlu ve tekil | JPN_014 | Interop | format reference / v4 farkı | Boş ve tekrar eden kimlik |
| `office_jp.office_name` | Mevcut `office_id` için zorunlu | V4 ana standardında yok; v3 alanı | Dosya mevcutsa zorunlu | JPN_005 | Interop | format reference / v4 farkı | İsim boşsa JPN_005 |
| `office_jp.office_url` | Varsa HTTP(S) biçim kalite kontrolü | V4 ana standardında yok; v3 alanı | Opsiyonel; mevcutsa biçim | JPN_020 | Quality | format reference / v4 farkı | Geçerli ve geçersiz URL |
| `office_jp.office_phone` | Varsa temel telefon biçim kalite kontrolü | V4 ana standardında yok; v3 alanı | Opsiyonel; mevcutsa biçim | JPN_020 | Quality | format reference / v4 farkı | Rakam/sembol içeren ve geçersiz değer |
| `routes_jp.txt` | v3'te yok; eski v2 feed'leri için parser/sinyal ve legacy JPN_015/JPN_016 korunur | V4 ana standardında yok | Legacy uyumluluk | JPN_015, JPN_016 (legacy) | Interop / Quality | [v3 PDF](https://www.mlit.go.jp/sogoseisaku/transport/content/001981081.pdf) / v4 farkı | Eski dosya; route referansı ve tarih biçimi; v3 kuralı değildir |
| `pattern_jp.txt` | Opsiyonel duruş paterni dosyası | Ana v4 standardından çıkarıldı; v3 uzantısı olarak referans bölümünde | Opsiyonel dosya | JPN_017, JPN_018 | Interop | pattern rehberi / v4 farkı | Dosya bilinmeyen dosya uyarısı üretmez |
| `pattern_jp.jp_pattern_id` | Dosya mevcutsa zorunlu ve tekil | V4 ana standardında `pattern_jp` master'ı yok; v4'teki `jp_pattern_id` alanıyla aynı ilişki varsayılmaz | Dosya mevcutsa zorunlu | JPN_017 | Interop | pattern rehberi / v4 farkı | Eksik ve tekrar eden kimlik |
| `pattern_jp.route_update_date` | Varsa geçerli `YYYYMMDD` | V4 ana standardında yok; v3/legacy alanı | Opsiyonel; mevcutsa biçim | JPN_016 | Quality | v3 PDF / pattern rehberi / v4 farkı | `20260824` geçer; `20260231` kalır |
| `trips.jp_pattern_id` | `pattern_jp.txt` mevcutsa `pattern_jp.jp_pattern_id` referansı; dosya yokken alan opsiyonel/iç kod olabilir | V4'te opsiyonel JP alanı korunur; `pattern_jp` master'ı v4 standardında olmadığı için v3 foreign key'i uygulanmaz | v3'te dosya mevcutsa koşullu foreign key | JPN_018 | Interop | trips rehberi / v4 farkı | V3'te geçerli ve kopuk referans |
| `translations.txt` kana satırları | `ja-Hrkt` okumaları ve GTFS-JP v3 referans bütünlüğü | V4'te standart translations dosyasıdır; `ja-Hrkt` okuması zorunlu, tablo ve alt kimlik kuralları genişletilmiştir | Profil kurallarına göre | JPN_001, JPN_008–010, JPN_019, JPN_021 | Quality / Interop | format reference / v4 farkı | V3 kana eksikliği, geçersiz kayıt ve çelişki |
| `jp_parent_route_id` | Tanınır; otomatik `route_id` foreign key sayılmaz | V4'te isteğe bağlı JP alanı korunur; rota gruplama anlamı açıkça tarif edilir | Opsiyonel alan | - | - | v3/v4 farkı | Değerin varlığı tek başına bulgu üretmez |
| `jp_trip_desc` | Tanınır; spesifikasyonda olmayan regex uygulanmaz | V4'te isteğe bağlı JP alanı korunur | Opsiyonel alan | - | - | v3/v4 farkı | Özel biçim icat edilmez |
| `jp_trip_desc_symbol` | Tanınır; spesifikasyonda olmayan regex uygulanmaz | V4'te isteğe bağlı JP alanı korunur | Opsiyonel alan | - | - | v3/v4 farkı | Özel biçim icat edilmez |

## Zorunluluk ve skor politikası

- Opsiyonel dosyanın yokluğu tek başına analiz skorunu veya yayın engelini değiştirmez.
- Opsiyonel dosya mevcutsa hatalı kimlik, tarih veya biçim Interop/Quality seviyesinde raporlanabilir.
- Rapor GTFS-JP tespiti yapar; `v3`/`v4` sürüm rozeti üretmez.
- `agency_jp.txt`, `office_jp.txt` ve `pattern_jp.txt` mevcutsa bu çalışma ağacında v3 kuralları çalışır; bu durum feed'in v4 olduğunu göstermez.
- `pattern_jp.txt` içindeki `origin_stop`, `via_stop` ve `destination_stop` açıklayıcı metindir; `stop_id` foreign key'i değildir.
- `translations.txt` içinde GTFS-JP v3'ün kullandığı `record_sub_id=NONE`, alt kimlik yok anlamında kabul edilir; `stop_times` için gerçek `stop_sequence` gerekir.

## v4 sonraki sprint kapsamı

MLIT v4 belgesinin ilk fark taraması tamamlandı. Sonraki v4 sprintinde:

1. v4'te ana standarda alınan/eklenen GTFS dosyaları ve alanları için ayrı kapsam matrisi ve kaynak çapasını oluşturmak,
2. v4 translations kurallarını v3 `record_sub_id=NONE` davranışından ayrı modellemek,
3. `jp_pattern_id` alanının v4'teki master'sız kullanımını v3 JPN_017/JPN_018'den ayırmak,
4. v4 için ayrı fixture ve ayrı profil kapısı eklemek

gerekecek. Bu işler tamamlanmadan v4 uyumluluk rozeti veya v4 kuralı üretilmeyecek.
