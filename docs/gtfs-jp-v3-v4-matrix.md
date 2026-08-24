# GTFS-JP v3/v4 uyumluluk matrisi

Bu belge, GTFS Analyzer’ın GTFS-JP kapsamını ve sonraki v4 çalışmasının sınırlarını kayıt altına alır. Analyzer feed’in v3 veya v4 olduğunu otomatik olarak iddia etmez; runtime yalnızca GTFS-JP sinyali üretir. v3 kuralları, v4’e aitmiş gibi koşulsuz çalıştırılmaz.

v3 satırları mevcut uygulama hedefidir. v4 sütunu ilk aşamada bir fark ve kapsam kaydıdır: v4’ün Japonya-özel alanları yeniden sınıflandırabilmesi nedeniyle v4’e özel runtime kuralı bu sprintte eklenmemiştir.

Kaynaklar: [GTFS-JP v3 resmî arşiv PDF'i](https://www.mlit.go.jp/sogoseisaku/transport/content/001981081.pdf), [pattern_jp.txt rehberi](https://www.busdata.or.jp/gtfs_guide/08%E3%80%80pattern_jp-txt%EF%BC%88%E5%81%9C%E8%BB%8A%E3%83%91%E3%82%BF%E3%83%BC%E3%83%B3%E6%83%85%E5%A0%B1%EF%BC%89%E3%80%80%E3%80%90%E4%BB%BB%E6%84%8F%E3%80%91/), [GTFS-JP v4 duyurusu](https://www.mlit.go.jp/commmmons/document/007/), [v3-v4 fark belgesi](https://www.mlit.go.jp/commmmons/document/007/commmons_doc_007-03_ver01.pdf).

| Dosya / alan | v3 durumu | v4 durumu | Zorunluluk seviyesi | Kural | Sınıf | Kaynak | Test senaryosu |
|---|---|---|---|---|---|---|---|
| `agency_jp.txt` | Profil dosyası; mevcutsa işleticinin Japonya-özel bilgileri | Fark matrisi; v4 yeniden sınıflandırması ayrıca doğrulanacak | Opsiyonel dosya | JPN_003, JPN_012, JPN_013 | Interop / Quality | format reference | Dosya yok; geçerli satır; eksik kimlik; hatalı posta kodu |
| `agency_jp.agency_id` | `agency.txt` kimliğine bağlanan zorunlu alan | v4 semantiği kodlanmadı | Dosya mevcutsa zorunlu | JPN_012 | Interop | format reference | Boş değer bulgu üretir |
| `agency_jp.agency_zip_number` | Varsa 7 ASCII rakam | v4 farkı doğrulama bekliyor | Opsiyonel alan; mevcutsa biçim | JPN_013 | Quality | format reference | `1234567` geçer; `123-4567` kalır |
| `office_jp.txt` | Ofis bilgileri; dosya opsiyonel | v4 farkı doğrulama bekliyor | Opsiyonel dosya | JPN_002, JPN_005, JPN_014, JPN_020 | Interop / Quality | format reference | Dosya yok; eksik/tekrarlı kimlik; biçim hatası |
| `office_jp.office_id` | Birincil anahtar; mevcut satırda dolu ve tekil | v4 semantiği kodlanmadı | Dosya mevcutsa zorunlu ve tekil | JPN_014 | Interop | format reference | Boş ve tekrar eden kimlik |
| `office_jp.office_name` | Mevcut `office_id` için zorunlu | v4 farkı doğrulama bekliyor | Dosya mevcutsa zorunlu | JPN_005 | Interop | format reference | İsim boşsa JPN_005 |
| `office_jp.office_url` | Varsa HTTP(S) biçim kalite kontrolü | v4 farkı doğrulama bekliyor | Opsiyonel; mevcutsa biçim | JPN_020 | Quality | format reference | Geçerli ve geçersiz URL |
| `office_jp.office_phone` | Varsa temel telefon biçim kalite kontrolü | v4 farkı doğrulama bekliyor | Opsiyonel; mevcutsa biçim | JPN_020 | Quality | format reference | Rakam/sembol içeren ve geçersiz değer |
| `routes_jp.txt` | v3'te yok; eski v2 feed'leri için parser/sinyal ve legacy JPN_015 korunur | v4 fark belgesinde güncel dosya olarak yok | Legacy uyumluluk | JPN_015 (legacy) | Interop | [v3 PDF](https://www.mlit.go.jp/sogoseisaku/transport/content/001981081.pdf) | Eski dosya; route referansı; v3 kuralı değildir |
| `pattern_jp.txt` | Opsiyonel duruş paterni dosyası | v4 farkı doğrulama bekliyor | Opsiyonel dosya | JPN_017, JPN_018 | Interop | [pattern rehberi](https://www.busdata.or.jp/gtfs_guide/08%E3%80%80pattern_jp-txt%EF%BC%88%E5%81%9C%E8%BB%8A%E3%83%91%E3%82%BF%E3%83%BC%E3%83%B3%E6%83%85%E5%A0%B1%EF%BC%89%E3%80%80%E3%80%90%E4%BB%BB%E6%84%8F%E3%80%91/) | Dosya bilinmeyen dosya uyarısı üretmez |
| `pattern_jp.jp_pattern_id` | Dosya mevcutsa zorunlu ve tekil | v4 semantiği kodlanmadı | Dosya mevcutsa zorunlu | JPN_017 | Interop | pattern rehberi | Eksik ve tekrar eden kimlik |
| `pattern_jp.route_update_date` | Varsa geçerli `YYYYMMDD` | v4 farkı doğrulama bekliyor | Opsiyonel; mevcutsa biçim | JPN_016 | Quality | v3 PDF / pattern rehberi | `20260824` geçer; `20260231` kalır |
| `trips.jp_pattern_id` | `pattern_jp.txt` mevcutsa `pattern_jp.jp_pattern_id` referansı; dosya yokken alan opsiyonel/iç kod olabilir | v4’te pattern master kaldırıldığı için bu foreign key uygulanmayacak | Dosya mevcutsa koşullu foreign key | JPN_018 | Interop | pattern ve trips rehberleri | Geçerli ve kopuk referans |
| `translations.txt` kana satırları | `ja-Hrkt` okumaları ve GTFS-JP referans bütünlüğü | v4 farkı doğrulama bekliyor | Profil kurallarına göre | JPN_001, JPN_008–010, JPN_019, JPN_021 | Quality / Interop | format reference, GTFS translations reference | Eksik kana, bilinmeyen kayıt, çelişen değer |
| `jp_parent_route_id` | Tanınır; bu sprintte otomatik `route_id` foreign key sayılmaz | v4 yeniden sınıflandırması belgelenmek üzere | Semantik kural yok | - | - | v3/v4 fark çalışması | Değerin varlığı tek başına bulgu üretmez |
| `jp_trip_desc` | Tanınır; spesifikasyonda olmayan regex uygulanmaz | v4 farkı doğrulama bekliyor | Serbest metin | - | - | format reference | Özel biçim icat edilmez |
| `jp_trip_desc_symbol` | Tanınır; spesifikasyonda olmayan regex uygulanmaz | v4 farkı doğrulama bekliyor | Serbest/uygulama metni | - | - | format reference | Özel biçim icat edilmez |

## Zorunluluk ve skor politikası

- Opsiyonel dosyanın yokluğu tek başına analiz skorunu veya yayın engelini değiştirmez.
- Opsiyonel dosya mevcutsa hatalı kimlik, tarih veya biçim Interop/Quality seviyesinde raporlanabilir.
- Rapor GTFS-JP tespiti yapar; `v3`/`v4` sürüm rozeti üretmez.
- `pattern_jp.txt` içindeki `origin_stop`, `via_stop` ve `destination_stop` açıklayıcı metindir; `stop_id` foreign key'i değildir.
- `translations.txt` içinde GTFS-JP v3'ün kullandığı `record_sub_id=NONE`, alt kimlik yok anlamında kabul edilir; `stop_times` için gerçek `stop_sequence` gerekir.

## v4 sonraki sprint kapsamı

MLIT v4 belgesi satır satır karşılaştırılacak; kaldırılan, yeniden adlandırılan veya referans seviyesine çekilen Japonya-özel alanlar ayrı bir v4 profiline bağlanacak. v4 runtime kuralları, bu matris ve fixture seti onaylandıktan sonra eklenecek.
