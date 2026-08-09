# #115 — `fast_travel_between_far_stops` değerlendirmesi

**Karar:** Yeni bir GTFS Analyzer kuralı eklenmedi. MobilityData kodundaki notice
aktif bir upstream davranış olarak incelendi, ancak güncel kurallar sayfasında
**deprecated** işaretlendiği ve 1000-feed örnekleri zaman, geometri ve meşru ekspres
servis sinyallerini birbirine karıştırdığı için bu bir **bilinçli Analytics coverage
gap** olarak bırakıldı. `STM_012` veya `STM_014` ile alias yapılması yanlış parity olur.

## Upstream algoritması

v8.0.1 tag'ındaki ve güncel `master` kaynağındaki `StopTimeTravelSpeedValidator`
şunları yapar:

1. Aynı route, stop dizisi ve zaman imzasına sahip trip'leri birlikte değerlendirir.
2. Stop çiftleri için ardışık stoplar arasındaki düz çizgi mesafelerini toplar; sonra
   her başlangıç/bitiş indeksini tarar. Bu nedenle `stop_sequence` farkı 1 olmak zorunda
   değildir.
3. Hız, route type eşiğini aşıyor ve kümülatif mesafe **10 km'den büyükse**
   `fast_travel_between_far_stops` üretir; aynı trip için en fazla bir notice verir.
4. Eşikler v8.0.1 source ile aynıdır: light rail 100, subway/bus/trolleybus/monorail
   150, rail 500, ferry 80, cable tram 30, aerial lift/funicular 50, bilinmeyen 200 km/h.
   Tam dakika zamanlarında bir dakikalık tampon, negatif/sıfır sürede ise 60 saniyelik
   güvenlik davranışı vardır.

Kaynaklar:

- [v8.0.1 `StopTimeTravelSpeedValidator.java`](https://github.com/MobilityData/gtfs-validator/blob/d74d7177f9f7c6bc7adc69508bb939362f2cf770/main/src/main/java/org/mobilitydata/gtfsvalidator/validator/StopTimeTravelSpeedValidator.java)
- [güncel `StopTimeTravelSpeedValidator.java`](https://github.com/MobilityData/gtfs-validator/blob/master/main/src/main/java/org/mobilitydata/gtfsvalidator/validator/StopTimeTravelSpeedValidator.java)
- [MobilityData kural sayfası — deprecated notice](https://gtfs-validator.mobilitydata.org/rules.html)

## 20 pozitif feed örneği

Örnekler, 2026-07-17 tarihli 1000-feed parity çıktısındaki ilk MD sample notice
ve ilgili feed zip'i birlikte incelenerek sınıflandırıldı. `valid time` sınıfı,
ham saat sırası bakımından bir cascade kanıtı bulunmadığını gösterir; bu tek başına
koordinatların doğru olduğunu iddia etmez.

| Feed | Örnek route type | Mesafe km | Hız km/h | Stop seq | Ham saat | Sınıf |
|---|---:|---:|---:|---|---|---|
| mdb-1135 (ES, Bizkaibus) | 3 | 10.51 | 242.6 | 19→21 | 08:46:47→08:49:23 | bağımsız hız anomalisi |
| mdb-1229 (ID, Bogor) | 3 | 10.00 | 600.2 | 1→19 | 23:51→00:19 | gece yarısı cascade |
| mdb-1292 (DK, Rejseplanen) | 3 | 30.15 | 164.5 | 30→31 | 20:02→20:12 | bağımsız hız anomalisi |
| mdb-1828 (TR, İZBAN) | 2 | 10.70 | 641.9 | 11→60 | 19:08:20→18:50 | azalan zaman cascade'i |
| mdb-1831 (TH, Bangkok) | 3 | 19.12 | 764.9 | 1→2 | 00:00→00:01:30 | bağımsız hız anomalisi |
| mdb-1916 (CY, OSEA) | 3 | 10.39 | 207.9 | 28→29 | 10:13→10:15 | bağımsız hız anomalisi |
| mdb-1918 (CY, NPT) | 3 | 15.03 | 150.3 | 11→15 | 12:42→12:47 | bağımsız hız anomalisi |
| mdb-2015 (LV, passenger rail) | 2 | 32.10 | 963.1 | 10→11 | 09:24→09:25 | bağımsız hız anomalisi |
| mdb-2021 (IT, Brindisi) | 3 | 11.03 | 661.7 | 22→23 | 14:27→14:27 | sıfır süre cascade'i |
| mdb-2155 (CZ, IDS JMK) | 3 | 29.38 | 1763.1 | 1→2 | 14:42→14:42 | sıfır süre cascade'i |
| mdb-2316 (US, Nebraska) | 3 | 99.59 | 373.5 | 2→3 | 16:30→16:45 | geometri/konum artefaktı şüphesi |
| mdb-2385 (LT, Šiauliai) | 3 | 11.47 | 344.0 | 1→5 | 14:36→14:37 | bağımsız hız anomalisi |
| mdb-2867 (IN, passenger rail) | 2 | 12.48 | 748.6 | 1→2 | 14:20→14:20 | sıfır süre cascade'i |
| mdb-2898 (CH, SBB) | 101 | 119.89 | 224.8 | 7→8 | 18:16→18:47 | meşru ekspres/uzun stop aralığı |
| mdb-2904 (CZ, airport route) | 800 | 5734.03 | 172020.8 | 10→11 | 06:39→06:40 | geometri/konum artefaktı şüphesi |
| mdb-2922 (TR, FlixBus) | 3 | 66.74 | 154.0 | 19→20 | 25:30→25:55 | bağımsız hız anomalisi |
| mdb-2933 (HK, CTB) | 3 | 10.69 | 320.6 | 3→4 | 00:37:35→00:39:35 | bağımsız hız anomalisi |
| mdb-2995 (IT, Sardinia ferry) | 1000 | 357.60 | 351.7 | 1→2 | 11:00→12:00 | geometri/konum artefaktı şüphesi |
| mdb-3108 (GE, route 6446) | 2 | 6387.22 | 29479.5 | 1→2 | 08:10→08:22 | geometri/konum artefaktı şüphesi |
| mdb-3131 (IN, BLRTransit) | 3 | 10.17 | 610.0 | 2→30 | 07:40→07:40 | sıfır süre cascade'i |

Özet: **20 örnek = 9 bağımsız hız anomalisi, 1 meşru ekspres/uzun aralık,
4 muhtemel geometri/konum artefaktı, 6 zaman cascade'i.** `unknown` sınıfına
zorlamalı atama yapılmadı; geometri sınıfı “muhtemel” olarak işaretlendi ve
normatif doğruluk iddiası taşımıyor. Bu dağılım yeni bir kuralın ham MD notice'ını
doğrudan yayın/kalite bulgusuna çevirmesi için yeterli güven vermiyor.

## Uygulama kararı ve regression kilidi

- Yeni `STM_*` veya `Analytics` rule eklenmedi.
- `fast_travel_between_far_stops` `spec-audit/md_parity_mapping.py` içinde
  `genuine-gap` olarak kalır; `STM_012`/`STM_014` map'ine eklenmez.
- `spec-audit/test_md_parity_audit.py` bu ayrımı ve explicit adjudication'ı test eder.
- `STM_012` ve `STM_014` kartları far-stop notice'ını exact parity olarak sunmaz.
- Gece yarısı ve express-service davranışları için yeni bir Analyzer emit'i
  eklenmediğinden, bu issue kapsamında bastırma/emit regression fixture'ı yoktur;
  mevcut `STM_048`, `STM_008`, `STM_012` ve `STM_014` testleri kendi kök nedenlerini
  korur.
