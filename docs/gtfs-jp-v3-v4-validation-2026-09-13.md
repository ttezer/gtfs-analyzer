# GTFS-JP V3/V4 doğrulama kaydı — 2026-09-13

Bu kayıt, GTFS-JP V3/V4 düzeltme paketinin aynı arşiv ve sabit tarihli önce/sonra doğrulamasıdır. Önceki binary `7b4d6b8d` commit'inden, sonraki binary bu paketin son çalışma ağacından release profilinde derlendi. Tüm analizlerde `--today 20260913` kullanıldı. Kaynak ve alan bazındaki kararlar [V3/V4 uyumluluk matrisinde](gtfs-jp-v3-v4-matrix.md) kayıtlıdır.

## Kalıcı regresyon kapsamı

Başlangıç incelemesindeki 12 durum kalıcı testlere taşındı:

| Başlangıç senaryosu | Kalıcı kanıt |
|---|---|
| `baseline`, `non_jp_explicit` | Auto/V3/V4 ve JP sinyali var/yok profil matrisi |
| `blank_location` | Eksik kolon, boş hücre ve geçersiz `location_type` ayrımı |
| `station_kana`, `short_name_skipped`, `ascii_source` | İstasyon, kısa/uzun hat adı ve yalnız Japonca kaynak kapsamı |
| `kanji_reading`, `kana_with_digit` | Yalnız Kanji reddi; yarım genişlik Katakana ve kana+rakam kabulü |
| `wrong_jp_values` | `feed_lang`, `agency_lang`, saat dilimi ve para birimi sabitleri |
| `route_agency_missing`, `v3_train` | `routes.agency_id` zorunluluğu ve geçerli V3 demiryolu `route_type=2` |
| `v4_fare_missing` | V4 karmaşık ücret istisnası mesajı, ücretsiz ve hatta eşlenmiş sabit ücretler |

Bunlara ek olarak `record_id`, `field_value`, yanlış `record_sub_id`, farklı seferlerde aynı `stop_sequence`, `jp_trip_desc`, `feed_publisher_url`, emit-proof ve profil ayrıntısı/locale çözümlemesi test edildi.

## Katori — iki sabit arşiv

| Arşiv | SHA-256 | Profil | Önce JPN / skor | Sonra JPN / skor | `pub_score` / `publishable` |
|---|---|---|---:|---:|---|
| `gtfs-katori20260901.zip` | `09e43675…b5a1c85` | Auto | 0 / 94,2 | 0 / 94,2 | 100 / `true` → aynı |
| aynı | aynı | V3 | 0 / 94,2 | 1.183 × JPN_030 / 93,1 | 100 / `true` → aynı |
| aynı | aynı | V4 | 0 / 94,2 | 0 / 94,2 | 100 / `true` → aynı |
| `jbda-katoricity-katori-junkan-202607010132.zip` | `82fd7997…f68e7` | Auto | 1 × JPN_008 / 94,4 | aynı | 100 / `true` → aynı |
| aynı | aynı | V3 | 1 × JPN_008 / 94,4 | JPN_008: 1, JPN_030: 1.159 / 93,3 | 100 / `true` → aynı |
| aynı | aynı | V4 | 1 × JPN_008 / 94,4 | aynı | 100 / `true` → aynı |

V3 farkı, feed'lerde kana satırlarının bulunmasına karşılık V3'ün zorunlu `language=ja` eşlerinin bulunmamasıdır. Auto sonucu byte düzeyindeki sürüm çıkarımına çevrilmedi; V4 de V3 zorunluluğunu uygulamadı. K4 ölçümü bu küçük feed'lerde zamanlayıcının 1 ms çözünürlüğünün altında kaldı.

## Japonya korpusu — aynı indirme üzerinde önce/sonra

2026-08-25 manifestindeki 592 Japon feed kimliği yeniden kullanıldı. Her kimlik için ZIP bir kez indirildi, SHA-256 kaydedildi ve iki binary aynı geçici dosya üzerinde çalıştırıldı. 590 ZIP iki tarafta da tamamlandı. `mdb-1057` ile `mdb-874` katalog URL'leri önceki ölçümde olduğu gibi ZIP olmayan payload döndürdüğü için aggregate dışında kaldı.

| Ölçüm | Önce | Sonra |
|---|---:|---:|
| Tamamlanan aynı-arşiv feed | 590 | 590 |
| Toplam JPN bulgusu | 95.765 | 2.005.321 |
| Ortalama genel skor | 93,528814 | 93,237966 |
| Skoru değişen feed | - | 456 |
| Yeni engellenen feed | - | 0 |
| Yeni yayınlanabilir feed | - | 7 |

JP kural gruplarının tamamı:

| Kural | Feed önce→sonra | Bulgu önce→sonra | Açıklama |
|---|---:|---:|---|
| JPN_001 | 40→46 | 12.830→19.633 | İstasyon kapsamı ve açık profil kapısı; geçersiz `location_type` kapsam dışı |
| JPN_004 | 7→12 | 7→12 | Açık V4 seçimi algılama olmasa da doğrulamayı çalıştırıyor |
| JPN_006 | 15→19 | 15→19 | Açık profil kapısı ve V4 istisna mesajı |
| JPN_007 | 0→5 | 0→5 | Açık profil seçimiyle eksik `feed_info.txt` görünür |
| JPN_008 | 490→496 | 5.017→6.160 | Japonca kısa ve uzun hat adları bağımsız Field bulguları |
| JPN_009 | 480→484 | 77.729→78.037 | Japonca kaynak filtresi ile açık profil kapısının net etkisi |
| JPN_010 | 148→152 | 148→161 | Japonca kaynak filtresi ile açık profil kapısının net etkisi |
| JPN_011 | 1→1 | 1→2 | Aynı feed'de `agency.txt` yanında `routes.agency_id` eksikliği korundu |
| JPN_019 | 1→1 | 1→1 | Genel referans kapsamı değişmedi |
| JPN_021 | 0→13 | 0→14 | Yalnız Kanji okumalar artık geçersiz; yarım genişlik Katakana kabul ediliyor |
| JPN_022 | 17→6 | 17→6 | Boş `location_type` yanlış alarmları kaldırıldı; gerçek kolon/alan eksikleri kaldı |
| JPN_029 | 0→435 | 0→1.901.271 | V4'ün önerilen stop-time/attribution kana kapsamı eklendi |

JPN_023–026 bu korpusta yeni bulgu üretmedi; mevcut geçerli değerler Japonya sabitleriyle uyumluydu. JPN_028/JPN_030 yalnız açık V3 kuralıdır ve V4 korpus koşumunda çalışmadı. JPN_027 registry'ye eklenmedi.

Yayın kararı için planlanan “tam sabitlik” ölçümde yedi iyileşme istisnası verdi; hiçbir feed yeni engellenmedi. Bunun nedeni yeni JPN kuralları değil, V3 kapsamı için gerekli `trips.jp_trip_desc` ve `feed_info.feed_publisher_url` çeviri hedeflerinin genel TRN_002 tarafından artık hatalı biçimde “geçersiz alan” sayılmamasıdır. `jbda-keisei-transitbus-keiseitransitbus` `feed_publisher_url`, altı Mie Kotsu feed'i `jp_trip_desc` nedeniyle önce yanlış `Kritik/Spec` engeli taşıyordu. Bu yedi feed `false→true`; diğer 583 feed'in `pub_score` ve `publishable` sonucu aynı kaldı.

Ham karşılaştırma repo dışındadır: `/Users/tacettintezer/GTFS/tmp/jp-package-20260913/jp-corpus-v4-final.jsonl`.

## Ağır feed ve performans

`mdb-865` arşivi (`8c527718…4567d0`, 76 MiB sıkıştırılmış, 1.176.280.707 bayt açılmış) Auto profiliyle iki kez önce/sonra ölçüldü. Her iki çiftte JSON çıktısı byte-byte aynıydı: 77.394 notice, JPN bulgusu yok, genel skor 74,9, `pub_score=83,3`, `publishable=false`.

| Ölçüm | Önce | Sonra |
|---|---:|---:|
| K4 süre aralığı | 1.347–1.379 ms | 1.133–1.210 ms |
| Toplam süre aralığı | 10,82–12,02 sn | 10,55–10,65 sn |
| Tepe RSS aralığı | 3,50–4,17 GB | 3,59–3,60 GB |

İlk uygulamadaki gereksiz Auto çeviri kapsam indeksi bu ölçüm sırasında yakalanıp kapatıldı. Son ölçümde Auto sonuçları aynı, K4 daha hızlı ve tepe bellek önceki doğal ölçüm aralığının içinde kaldı. `jp_trip_desc` yalnız dolu satırlar için seyrek yan haritada tutuluyor; ham `stop_times` satırları kopyalanmıyor.

## Sınırlar

- Bu çalışma tam MLIT uyumluluk sertifikası değildir; yalnız matriste listelenen kapsamı doğrular.
- Korpus mutable `latest.zip` URL'lerinden aynı anda indirilen arşivleri iki binary arasında sabitler; 2026-08-25 byte'larıyla özdeş olduklarını varsaymaz.
- V4'te JPN_029 öneri kapsamı doğal olarak çok sayıda satır bulgusu üretir; skor etkisi Quality sınıfında kalır ve yeni yayın engeli oluşturmaz.
