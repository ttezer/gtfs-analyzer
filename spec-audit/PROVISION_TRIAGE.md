# Düzyazı hüküm triyajı (T5 — GTFS Spec rozeti)

Kaynak: `spec_provisions.json` (üreteç `extract_provisions.py`). O dosya **aday** üretir;
burası her adayın hüküm olup olmadığına ve karşılanıp karşılanmadığına karar verir.

Katalog toplamı **273 aday** (sert 163 · yumuşak 110). Bu belge **70'ini** adjudike eder:
1. turda 27 (`file-requirements` + `field-types` + `locations.geojson`), 2. turda 43
(`stop_times.txt`'in tamamı). Sıra rastgele değil — `file-requirements` bölümünün tamamı
düzyazıdır ve hiçbir alan tablosuna yansımaz, `stop_times.txt` ise en yoğun alan bölümüdür.

## Karar sınıfları

| sınıf | anlamı |
|---|---|
| **KANITLI** | Bir kural hükmü doğrudan ölçüyor; kod konumu yazılı. |
| **DOLAYLI** | Hüküm ihlali başka kurallar tarafından yakalanıyor ama adı konmuyor. |
| **BOŞLUK** | Hiçbir kural ölçmüyor. |
| **KAPSAM DIŞI** | Tüketici tarafını bağlar ya da betimleyicidir; feed'den doğrulanamaz. |
| **META** | Spec'in kendi terim tanımı; hüküm değil. |

---

## file-requirements (13 aday — bölümün tamamı düzyazı)

| id | hüküm | karar | dayanak |
|---|---|---|---|
| `Pc3b911a6` | Dosyalar virgülle ayrılmış metin olmalı | DOLAYLI | Ayraç virgül değilse başlık tek sütuna çöker → `ARC_025` zorunlu sütun eksik (fatal). ⚠️ **[Varsayım]** — bu yol ölçülmedi, kod okumasına dayanıyor. |
| `Pd59e5eaa` | İlk satır alan adlarını içermeli | DOLAYLI | Başlık yerine veri satırı varsa `ARC_025` + `ARC_019` + `ARC_017` birlikte ateşler. Hükmün kendisi adlandırılmamış. |
| `P28e9ee8a` | Alan değerleri sekme/CR/LF içermemeli | **KANITLI** | `ARC_030` — `k1_parse.rs:535` doc yorumu spec cümlesini birebir alıntılıyor; `ARC_021`'den ayrımı orada gerekçeli. |
| `Pfedb83cf` | Tırnak/virgül içeren değerler tırnak içine alınmalı | DOLAYLI | `ARC_013` kapanmamış tırnağı yakalar. ⚠️ **Alınmamış hâl saptanamaz:** tırnaksız virgül fazladan sütun üretir → `ARC_012`. İhlal ile geçerli veri ayrımı CSV düzeyinde kaybolur. |
| `Pa4c60372` | Değer içindeki her tırnak bir tırnakla kaçırılmalı | DOLAYLI | Aynı gerekçe (`ARC_013`). |
| `Pd6bc0278` | Alan değerleri HTML etiketi, yorum veya kaçış dizisi içermemeli | **BOŞLUK** | Kod tabanında karşılığı yok. Ayrıntı aşağıda. |
| `P5f591221` | Fazladan boşluklar kaldırılmalı *(soft)* | **KANITLI** | `DQ_016` (Orta·Quality) — dosya başına tek özet. Yumuşak hüküm, Quality sınıfı: doğru eşleşme. |
| `P536c3b95` | Her satır CRLF veya LF ile bitmeli | **KANITLI** | `ARC_026` — `k1_parse.rs:943`. |
| `Pf79a5cc1` | Dosyalar UTF-8 kodlanmalı; BOM kabul edilir *(soft)* | **KANITLI** | `ARC_003` (kodlama hatası) + `ARC_010` (BOM bilgisi). BOM'un kabul edilirliği `strip_bom` ile korunuyor. |
| `P78b1b3e8` | Tüm dosyalar birlikte zip'lenmeli | **KANITLI** | `ARC_001` — açılamayan arşiv. |
| `Pc0951256` | Dosyalar doğrudan kökte olmalı, alt klasörde değil | **KANITLI** | `ARC_024`. Tek üst klasöre sarılmış feed'ler `detect_wrapped_root` ile bilinçli tolere edilir → [[project_wrapped_root_tolerance]]. |
| `P33d5b079` | Yolcuya görünen metinler Mixed Case olmalı *(soft)* | KISMİ | `DQ_018` all-caps ölçüyor ama kapsamı dar (`stop_id` scope). Route adları ve headsign'lar kapsam dışı. |
| `Pf6b6138a` | Kısaltmalardan kaçınılmalı *(soft)* | **KAPSAM DIŞI** | "St." kısaltma mı gerçek ad mı ayrımı sözlük işi; spec'in kendisi istisna tanıyor ("JFK Airport"). Yanlış pozitif oranı kabul edilemez. |

## field-types (8 aday)

| id | hüküm | karar | dayanak |
|---|---|---|---|
| `P9ca98eaa` | Renk değerinde baştaki `#` bulunmamalı | **KANITLI** | `is_hex_color_6` (`common.rs:243`) `len()==6` şartı koyar → `#FF0000` (7) reddedilir. `RTS_006`/`RTS_007`. |
| `Pa6731197` | Parasal hesaplar decimal tipiyle yapılmalı *(soft)* | **KAPSAM DIŞI** | Tüketici yazılımını bağlar, feed içeriğini değil. |
| `P43b15497` | Yalnız yazdırılabilir ASCII önerilir *(soft)* | KISMİ | `ARC_021` kontrol/yazdırılamaz karakterleri yakalar ama geçerli Unicode'u **bilinçli** muaf tutar (VBB Almanca metin kararı). Spec'in ASCII tavsiyesi bundan daha dardır ve uygulanmıyor — doğru karar. |
| `P698baeb3` | "unique ID" etiketli alan dosya içinde benzersiz olmalı | **KANITLI** | `DQ_021` (genel birincil-anahtar kuralı) + varlık düzeyi ikizleri. |
| `P86b889a8` | Latitude −90.0 ≤ x ≤ 90.0 | **KANITLI** | `stops.rs:213` (`STP_003`) + `shapes.rs:375` (`SHP_002`). |
| `P7e9096c8` | Longitude −180.0 ≤ x ≤ 180.0 | **KANITLI** | `stops.rs:255` (`STP_004`) + `shapes.rs:414` (`SHP_003`). |
| `P8681b6f4` | Text tipi insan tarafından okunabilir olmalı | KISMİ | `ARC_021` yaklaşık ölçer. "İnsan okunabilir" makine tanımı yoktur; daha ileri gitmek yanlış pozitif üretir. |
| `P5f72fb5a` | URL `http://` veya `https://` içeren fully qualified URL olmalı | **BOŞLUK** | Ölçüldü, ayrıntı aşağıda. |

## locations.geojson (6 aday)

| id | hüküm | karar | dayanak |
|---|---|---|---|
| `Pd84a0bcb` | Her poligon OpenGIS Simple Features 6.1.11'e göre geçerli olmalı | KISMİ | `LOC_004` yalnız **ring kapalılığını** ölçer. Kendini kesen ring, ters yönlü delik, sıfır alanlı ring ölçülmüyor. Tam OpenGIS geçerliliği geometri kütüphanesi ister — açık kalem, bugün kapatılmıyor. |
| `P2f283161` | Dosya bir FeatureCollection içermeli | **KANITLI** | `LOC_001` — `k1_parse.rs:1460`. |
| `P7f589103` | Her Feature'ın `id`'si olmalı | **KANITLI** | `LOC_003`. |
| `P042ba79f` | `id`, stop_id / geojson id / location_group_id genelinde benzersiz olmalı | **KANITLI** | `XFL_031` — `k4_cross_ref.rs:3816`; spec cümlesi doc yorumunda birebir alıntılı, üç kaynak da taranıyor. |
| `Pf7585903` | Feature'lar tablodaki anahtarları taşımalı *(soft)* | **KANITLI** | `LOC_008` (type) · `LOC_009` (properties) · `LOC_010` (coordinates). |
| `P1afc582a` | `location_group_id` aynı isim alanında benzersiz olmalı | **KANITLI** | `XFL_031` (aynı kural, hükmün location_groups tarafı). |

---

## Bulgular

### 1. `looks_like_url` şema kontrolü yapmıyor — **BOŞLUK** (`P5f72fb5a`)

Spec: *"URL - A fully qualified URL that includes http:// or https://."*
Kod: `looks_like_url(v) = Url::parse(v).is_ok()` (`k2/common.rs:200`) — şemayı **hiç** kontrol etmez.

Ölçüldü (geçici probe, sonra geri alındı):

```
http://a.com        -> true      mailto:info@a.com   -> true   ← spec'e göre geçersiz
https://a.com       -> true      ftp://a.com/x       -> true   ← spec'e göre geçersiz
www.example.com     -> false     foo:bar             -> true   ← spec'e göre geçersiz
a.com               -> false     javascript:alert(1) -> true   ← spec'e göre geçersiz
                                 file:///etc/passwd  -> true   ← spec'e göre geçersiz
```

Etki alanı: `agency_url` · `agency_fare_url` · `stop_url` · `route_url` · `booking_url` ·
`info_url` · `attribution_url` · `feed_publisher_url` — URL biçimi ölçen her kural.

⚠️ Bu bir yanlış-**negatif**: bugün sessiz kalıyor, düzeltilince yeni bulgu üretecek. Yönü
önemli — geçerli feed'i yayından alıkoymuyor, yani `PTH_017` sınıfı bir hata değil. Ama
`javascript:` şemasının yolcuya görünen bir alanda geçerli sayılması yalnız spec ihlali değil,
tüketici tarafında zararlı bir değer.

**Yapılmadı, bilinçli:** düzeltmeden önce gerçek feed'lerde kaç `mailto:`/şemasız değer
olduğu ölçülmeli. Korpus koşusu kullanıcı kararıdır ([[feedback_no_feed_tests]]); statik
tarama yeterli olabilir ama başlık tuzağına dikkat.

### 2. HTML etiketi / yorum / kaçış dizisi hiç ölçülmüyor — **BOŞLUK** (`Pd6bc0278`)

Spec: *"Field values must not contain HTML tags, comments or escape sequences."*
Kod tabanında karşılığı yok — `ARC_021` kontrol karakterlerini, `ARC_030` sekme/satır sonunu
ölçer; ikisi de `<b>` veya `&nbsp;` görmez.

Bu hüküm sert (`must not`) ve alan tablosunda hiç görünmez, yani bugünkü 302 atomun
tamamen dışında. Gerçek bir kural adayı; yanlış pozitif riski düşünülmeli (`&` içeren meşru
ad, `<` içeren metin).

### 3. Kataloğun kör noktası: modal taşımayan hükümler

`file-requirements` bölümünde **"All file and field names are case-sensitive."** cümlesi var.
Bu bir hükümdür ama `must`/`shall`/`Required` taşımadığı için katalog onu **yakalamıyor**.
Üreteç modal-tabanlıdır ve bu sınıfı yapısal olarak kaçırır.

Yani 246 sayısı bir **alt sınırdır**, tam payda değil. Rozet cümlesi kurulurken bu
açıkça yazılmalı; "246 hükmün N'si" demek, katalogun kendi kör noktasını gizler.

---

---

# 2. tur — `stop_times.txt` (43 aday, tamamı)

⚠️ Bu tur, üretecin **alan adı taşımaması** yüzünden önce yapılamadı: "Required for
timepoint=1" hem `arrival_time` hem `departure_time` altında geçer ve karşılıkları farklı
kurallardır. Üreteç düzeltildi (`a8cc7354`), katalog 246 → **273 aday**.

## Sert hükümler (26)

| id | alan | hüküm | karar | dayanak |
|---|---|---|---|---|
| `P9c00a4c1` | arrival_time | İlk ve son durakta zorunlu | **KANITLI** | `STM_015` (ilk) + `STM_016` (son), ikisi de Kritik·Spec. |
| `P63ebcd1c` | arrival_time | `timepoint=1` için zorunlu | **KANITLI** | `STM_047`. |
| `P7d083b18` | departure_time | `timepoint=1` için zorunlu | **KANITLI** | `STM_047` (aynı kural iki alanı birlikte denetler). |
| `P9fb3b94e` | arrival_time | Flex penceresi varken yasak | **KANITLI** | `STM_037`. |
| `P5c565dc7` | departure_time | Flex penceresi varken yasak | **KANITLI** | `STM_037`. |
| `P22458c92` | stop_id | Referans verilen yer durak/platform olmalı (`location_type` 0 veya boş) | **KANITLI** | `STP_012` — `k4_cross_ref.rs:567`. |
| `Pc13251f4` | stop_id | `location_group_id` ve `location_id` yoksa zorunlu | **KANITLI** | `STM_041` (üçünün karşılıklı dışlaması). |
| `Pf4e87f1d` | stop_id | Diğer ikisi tanımlıysa yasak | **KANITLI** | `STM_041`. |
| `P1c390f1f` | location_group_id | `stop_id`/`location_id` varken yasak | **KANITLI** | `STM_041`. |
| `P51a00951` | location_id | `stop_id`/`location_group_id` varken yasak | **KANITLI** | `STM_041`. |
| `P845d30d4` | stop_sequence | Değerler sefer boyunca artmalı, ardışık olmak zorunda değil | **KANITLI** | Azalma → `STM_036`; yinelenen → `STM_032`. ✅ **İzin tarafı da doğrulandı:** ardışık olmayan `stop_sequence`'e ateşleyen kural YOK — spec'in açık iznini ihlal etmiyoruz (`PTH_017` sınıfı hata aranıp bulunamadı). |
| `Pc12c3c45` | start_pickup_drop_off_window | `location_group_id`/`location_id` tanımlıysa zorunlu | **KANITLI** | `STM_039`. |
| `P2de6ce3d` | end_pickup_drop_off_window | Aynı koşul | **KANITLI** | `STM_039`. |
| `Pe9ff5c29` | start_…_window | Diğer pencere tanımlıysa zorunlu | **KANITLI** | `STM_039` (çift eksikliğini de tek pencereyi de kapsar). |
| `P5409e4cb` | end_…_window | Aynı | **KANITLI** | `STM_039`. |
| `Pa99a9040` | start_…_window | `arrival_time`/`departure_time` tanımlıysa yasak | **KANITLI** | `STM_037` (hükmün ters yönü, aynı çelişki). |
| `Pd63bf170` | end_…_window | Aynı | **KANITLI** | `STM_037`. |
| `P56388e97` | pickup_type | Pencere varken `pickup_type=0` yasak | **KANITLI** | `STM_051`. |
| `P4280b626` | drop_off_type | Pencere varken `drop_off_type=0` yasak | **KANITLI** | `STM_052`. |
| `P19ab03d5` | continuous_pickup | Pencere varken 1/boş dışı yasak | **KANITLI** | `STM_054`. |
| `Pf1672264` | continuous_drop_off | Pencere varken 1/boş dışı yasak | **KANITLI** | `STM_055`. |
| `Pf2eca25e` | shape_dist_traveled | `stop_sequence` ile artmalı; ters seyahat göstermemeli | **KANITLI** | `STM_056` (artmıyor) + `STM_030` (negatif/sayı değil). |
| `P8df924de` | stop_id | Seferde hizmet verilen tüm duraklar `stop_times.txt`'te kayıtlı olmalı | **KAPSAM DIŞI** | Feed'den doğrulanamaz: hangi durağa gerçekte hizmet verildiği ancak dış bilgiyle bilinir. Eksik kaydı "eksik" diye gören bir ölçüm yok. |
| `P97d265ec` | location_group_id | Aynı hüküm, location group tarafı | **KAPSAM DIŞI** | Aynı gerekçe. |
| `P301db728` | location_id | Aynı hüküm, GeoJSON tarafı | **KAPSAM DIŞI** | Aynı gerekçe. |
| `Pe1d776e1` | stop_headsign | Birden çok satırda geçersiz kılmak için değer her satırda tekrarlanmalı | **KAPSAM DIŞI** | Alanın nasıl kullanılacağını anlatan betimleyici cümle; ihlal edilebilir bir yasak koymuyor. |

## Yumuşak hükümler (17)

Aynı tavsiye birden çok alanın açıklamasında tekrar eder; alan adı kimliğe girdiği için
her biri ayrı satırdır (spec metni gerçekten her alanın altında tekrar yazar).

| id | alan | tavsiye | karar |
|---|---|---|---|
| `Peaccedd4` · `P07ae073c` | arrival_time · departure_time | Ayrı zaman yoksa ikisi aynı olmalı | KISMİ — `STM_013` karışık zamanları ölçer, bu tavsiyenin tam karşılığı değil. |
| `Pc6b3be8f` | timepoint | Zamanı olan her kayıtta `timepoint` dolu olmalı | **KANITLI** — `STM_050` (sütun var ama satırda boş). |
| `Pd0952f0f` · `Pd6d4d60e` | arrival_time · departure_time | Kesin zaman yoksa `timepoint=0` ile tahmini zaman verilmeli | KISMİ — `STM_050` yakınsıyor. |
| `P7c96867d` | shape_dist_traveled | Döngü/iç içe geçen hatlarda önerilir | KISMİ — `STM_017` mesafe eksikliğini ölçer, döngü koşuluna bakmaz. |
| `P71243b3e` | pickup_booking_rule_id | `pickup_type=2` iken önerilir | **BOŞLUK** — aşağıda. |
| `P504bde32` | drop_off_booking_rule_id | `drop_off_type=2` iken önerilir | **BOŞLUK** — aşağıda. |
| `Pc45300d2` | stop_headsign | Tüm sefer boyunca aynıysa `trips.trip_headsign` kullanılmalı | KISMİ — `TRP_020` komşu alanı ölçüyor; birebir değil. |
| `P5c5b9def` | — | Tüketici ara kayıtları yok saymalı | **KAPSAM DIŞI** — tüketiciyi bağlar. |
| `Pf991d0a1` · `Pf400fbfa` · `P95949c3b` | stop_id · location_group_id · location_id | Tüketici seyahati mümkün varsaymalı | **KAPSAM DIŞI** — tüketiciyi bağlar. |
| `P1db24034` · `P42feba23` · `Pb3167e34` | stop_id · location_group_id · location_id | Talep-üzerine hizmet, hizmetin sunulduğu sırayla referanslanmalı | **KAPSAM DIŞI** — gerçek hizmet sırası feed dışı bilgi. |
| `P02809a76` | — | Örnek sayfasına atıf | **META** — hüküm değil, belge atfı. |

## Bulgu 4: `STM_040` spec'in koşulunu değil, başka bir koşulu ölçüyor

Spec `pickup_booking_rule_id` için *"Recommended when pickup_type=2"* der (ve `drop_off_type=2`
için ikizini). `STM_040` ise **Flex penceresi varken** booking rule eksikliğini ölçüyor
(`k2/stop_times.rs:1571` — `has_any_window && pbr_raw.is_empty() && dobr_raw.is_empty()`).

İkisi farklı popülasyon: `pickup_type=2` ("ajansı telefonla arayın") Flex penceresi olmadan da
kullanılır — klasik dial-a-ride. O satırlarda bugün hiçbir şey söylenmiyor.

Yumuşak hüküm olduğu için karşılığı **Quality** sınıfına gider, `Spec` değil — `PTH_017`
dersinin doğrudan uygulaması. Kural yazılmadı; `STM_040`'ı genişletmek mi yeni kural mı
sorusu 3. tura bırakıldı, çünkü `STM_040`'ın kendi koşulu da meşru ve ikisini tek kuralda
birleştirmek `emit_identity` kapısını düşürebilir.

---

## Sonraki tur

Kalan ~210 aday. Yoğunluk: `stops.txt` 20 · `routes.txt` 14 · `pathways.txt` 14 ·
`transfers.txt` 13 · `translations.txt` 13 · `booking_rules.txt` 12.

Açık kalemler: URL şeması (ölçüm bekliyor) · HTML etiketi (kural adayı) · OpenGIS poligon
geçerliliği (geometri kütüphanesi kararı) · `pickup_type=2` booking rule tavsiyesi (kural şekli).
