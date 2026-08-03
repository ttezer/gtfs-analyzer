# Düzyazı hüküm triyajı (T5 — GTFS Spec rozeti)

Kaynak: `spec_provisions.json` (üreteç `extract_provisions.py`). O dosya **aday** üretir;
burası her adayın hüküm olup olmadığına ve karşılanıp karşılanmadığına karar verir.

Katalog toplamı **273 aday** (sert 163 · yumuşak 110). Bu belge **107'sini** adjudike eder:
1. tur 27 (`file-requirements` + `field-types` + `locations.geojson`) · 2. tur 43
(`stop_times.txt`) · 3. tur 37 (`stops.txt` + `routes.txt`). Bölümler tam bitirilir, yarım
bırakılmaz — kalan sayısı böyle güvenilir kalır.

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

# 3. tur — `stops.txt` (22) + `routes.txt` (15), ikisi de tamamı

## stops.txt

| id | alan | hüküm | karar | dayanak |
|---|---|---|---|---|
| `P2d2a4fe2` | stop_id | Üç kaynak genelinde benzersiz | **KANITLI** | `XFL_031` (1. turda `P1afc582a` ile aynı hüküm, `stops` tarafı). |
| `P2d5798e5` | stop_name | `location_type` 0/1/2 için zorunlu | **KANITLI** | `STP_003`. |
| `Pe929fc21` · `Pdb1fcb2a` | stop_lat · stop_lon | Aynı üç tip için zorunlu | **KANITLI** | `STP_006` · `STP_007`. |
| `Pae80c9ab` | parent_station | `location_type` 2/3/4 için zorunlu | **KANITLI** | `STP_011`. |
| `P84671c38` · `P9963363e` | parent_station | İstasyonda boş olmalı / yasak | **KANITLI** | `STP_036` (aynı hükmün iki cümlesi). |
| `Pac2e68e4` | stop_access | Geçerli değerler 0/1/2 | **KANITLI** | `STP_024` (ham alan) + `STP_026`. |
| `Paa26a036` | stop_access | İstasyon/giriş/düğüm için yasak | **KANITLI** | `STP_043` — `k4_cross_ref.rs:669` `bad_type` kolu. |
| `Pb9ee0cd1` | stop_access | `parent_station` boşsa yasak | **KANITLI** | `STP_043` — aynı yerin `no_parent` kolu. |
| `P66bbc659` | stop_url | agency/route URL'sinden farklı olmalı *(soft)* | **KANITLI** | `STP_034` · `STP_035`. |
| `P1904b638` · `P0deb828b` | stop_lat · stop_lon | Koordinat yolcunun bindiği yer olmalı | **KAPSAM DIŞI** | Sert cümle ama feed dışı gerçeğe atıf; koordinatın "doğru yer" olduğu doğrulanamaz. `GEO_009` yalnız shape'ten sapmayı ölçer, farklı olgu. |
| `P4dec6139` | stop_name | Ajansın yolcuya gösterdiği adla eşleşmeli *(soft)* | **KAPSAM DIŞI** | Basılı tarife/dış kaynak bilgisi gerekir. |
| `P2f98361b` | stop_name | Boarding area'da biniş alanının adı olmalı *(soft)* | KISMİ | `STP_041` alt durak/üst istasyon ad ilişkisini ölçer; boarding area'ya özel değil. |
| `P4aab119d` | location_type | Giriş birden çok istasyona aitse veri sağlayıcı birini parent seçmeli | **KAPSAM DIŞI** | Şema zaten tek `parent_station` alanı verir → ihlal edilemez; cümle modelleme rehberi. |
| `P740a5096` · `Pb857b7b0` · `Pa5a48cf2` | stop_access | Girişten erişilmeli; pathway varsa kullanılmalı; tüketici yön üretmeli | **KAPSAM DIŞI** | Tüketici davranışını bağlar. `STP_027` komşu olguyu (pathway istasyonunda `stop_access` eksik) ölçer. |
| `P8cb6a9cc` | stop_code | Yolcuya sunulan kodu olmayan yerlerde **boş bırakılmalı** *(soft)* | ⚠️ GERİLİM | Aşağıda. |
| `P3af6af7b` · `Pb24eacd3` | platform_code | Yalnız tanımlayıcı olmalı; "platform"/"track" kelimesi geçmemeli *(soft)* | **BOŞLUK** | `platform_code` kod tabanında hiç geçmiyor. `STP_040` aynı olguyu `stop_name` için ölçüyor — desen mevcut, alan kapsanmamış. |

## routes.txt

| id | alan | hüküm | karar | dayanak |
|---|---|---|---|---|
| `P5545f72a` | agency_id | Çoklu ajans varsa zorunlu | **KANITLI** | `AGN_011` (08-02'de `fare_attributes`'u da kapsayacak şekilde genişletildi). |
| `P38ea8952` | agency_id | Aksi hâlde önerilir *(soft)* | **KANITLI** | `RTS_025` (Quality) — tavsiye/norm ayrımı doğru kurulmuş. |
| `P58b03291` · `Pd236e553` | route_short_name · route_long_name | Biri boşsa diğeri zorunlu | **KANITLI** | `RTS_003` (tek kural, iki alan; `field` artık boru konvansiyonuyla ikisini de yazıyor). |
| `P16dc75b8` | route_short_name | 12 karakterden uzun olmamalı *(soft)* | **KANITLI** | `RTS_010` — eşik `char_len > 12` (`routes.rs:89`), spec'le birebir. `RTS_021` ayrıca Google'ın 6 karakterlik eşiğini ölçer. |
| `P067f2da6` · `Pe69f3d33` | route_color · route_text_color | Yeterli kontrast olmalı *(soft)* | **KANITLI** | `RTS_008` — `wcag_contrast_ratio` (`common.rs:247`). |
| `P9068b265` · `P0003b8f1` | continuous_pickup · continuous_drop_off | Flex penceresi varken 1/boş dışı yasak | **KANITLI** | `RTS_028` (08-03'te Interop→**Spec**; otorite MD paritesi değil spec'tir). |
| `P419062b4` | network_id | `route_networks.txt` veya `networks.txt` varsa yasak | **KANITLI** | `XFL_019` — iki kolu da denetler. |
| `P69d68d06` | cemv_support | Fare dosyalarıyla çelişki olmamalı | **KANITLI** | `XFL_026` · `XFL_027`. |
| `P803cfa49` | cemv_support | Yalnız tüm hizmetler cEMV kabul ediyorsa bildirilmeli *(soft)* | KISMİ | `XFL_029` yakınsıyor; "tüm hizmetler" koşulu ölçülmüyor. |
| `P6802b122` | route_short_name | Kısa hizmet tanımı varsa önerilir *(soft)* | KISMİ | `RTS_003` yalnız ikisinin birden boş olmasını ölçer. |
| `P86914315` | cemv_support | Çakışmada `routes.cemv_support` geçerlidir | **KAPSAM DIŞI** | Öncelik kuralı — tüketiciye hangi değeri okuyacağını söyler, ihlal edilebilir bir yasak koymaz. |
| `P69d3a653` | route_sort_order | Küçük değerli hatlar önce gösterilmeli *(soft)* | **KAPSAM DIŞI** | Görüntüleme davranışı. `RTS_029` alanın geçerliliğini ölçer, sıralama beklentisini değil. |

---

## Bulgu 5: `STP_022` spec'in tavsiyesiyle ters yönde çalışıyor

Spec `stop_code` için: *"This field should be left empty for locations without a code presented
to riders."* Yani **kodu olmayan yerde boşluk doğru davranıştır.** `STP_022` ise eksikliği
bildiriyor (`k2/stops.rs:130` — `stop_code.is_none() && is_stop_or_station`).

⚠️ **Bu `PTH_017` sınıfı bir hata DEĞİL, çünkü sınıfı doğru:** `STP_022` Orta·**Quality**.
R1 kapısı saf `Spec ∧ Kritik` olduğu için hiçbir feed'i yayından alıkoymuyor, ve Quality
"üretici kalitesi sinyali" demek — spec ihlali iddiası değil. Kural bilinçli bir üretici
tavsiyesi olarak duruyor.

Yine de defterlenmesi gerekiyordu: spec bir alanın boş olmasını **açıkça meşru** sayarken
biz onu eksiklik diye raporluyoruz. Sınıf bunu meşrulaştırıyor, ama kart metni bu nüansı
taşımalı — aksi hâlde kullanıcı düzeltilmesi gerekmeyen bir şeyi düzeltir.

**Yapılmadı:** `STP_022`'ye dokunulmadı. Sınıfı doğru, davranışı bilinçli.

## Bulgu 6: `platform_code` hiç ölçülmüyor — **BOŞLUK** (soft)

Spec iki tavsiye yazar: değer yalnız platform tanımlayıcısı olmalı, ve *"platform"* / *"track"*
(veya feed dilindeki karşılığı) kelimesi **içermemeli**.

`platform_code` kod tabanında hiç geçmiyor — ne parse ediliyor ne ölçülüyor. İlginç olan,
**aynı olgunun `stop_name` için ölçülüyor olması**: `STP_040` "durak adı gereksiz stop/station
sözcüğü içeriyor" der. Desen mevcut, dil listesi mevcut, yalnız bu alana uygulanmamış.

Yumuşak hüküm → karşılığı **Quality**. Kural yazılmadı; `STP_040`'ın sözcük listesi
`platform`/`track` için genişletilebilir mi, yoksa ayrı kural mı — 4. tura bırakıldı.

---

# 4. tur — `transfers.txt` (18) + `pathways.txt` (14)

## transfers.txt

| id | alan | hüküm | karar | dayanak |
|---|---|---|---|---|
| `P8e01aabc` · `P2f6c441c` | from_stop_id · to_stop_id | `transfer_type` boş/0/1/2/3 ise zorunlu | **KANITLI** | `TRF_001` · `TRF_002`. |
| `P985bdc2c` · `Pf42894d6` | from_stop_id · to_stop_id | `transfer_type` 4/5 ise durak (`location_type=0`) olmalı | **KANITLI** | İki kuralın **birleşimi** tam: `TRF_021` her zaman 2/3/4'ü yasaklar (`k4_cross_ref.rs:1799`), `TRF_015` 4/5'te ayrıca 1'i yasaklar (`:1820`). Geriye yalnız 0/boş kalır. Kod yorumu ayrımı gerekçelendiriyor. |
| `P8423efa1` · `P4139d9ac` | from_trip_id · to_trip_id | `transfer_type` 4/5 ise zorunlu | **KANITLI** | `TRF_014` (in-seat aktarma için sefer yok). |
| `P0fa71358` · `P585a24d4` · `P134a2d40` · `P446972ca` | from/to_route_id · from/to_trip_id | İkisi birlikte tanımlıysa trip route'a ait olmalı | **KANITLI** | `TRF_017` (sefer aktarması yanlış hat). Cümlenin "trip_id öncelik alır" kısmı öncelik kuralı → ihlal edilemez. |
| `P35e92ee9` · `P04a7709e` | transfer_type | 5 = ardışık seferler arası in-seat aktarma **yasak**; yolcu inip yeniden binmeli | **KANITLI** | `TRF_015` · `TRF_019` (in-seat aktarmada farklı `route_type`). |
| `Pd69ec0d5` | transfer_type | Geçerli değerler 0/empty…5 *(soft biçimde yazılmış)* | **KANITLI** | `TRF_004`. |
| `P320bcb49` | min_transfer_time | Saniye cinsinden, aktarmaya izin verecek süre | **META** | Alanın tanımı; ihlal edilebilir bir yasak koymuyor. Değer geçerliliği `TRF_005`/`TRF_010`. |
| `P4eb0f714` | min_transfer_time | Tipik yolcunun yürümesine yetmeli *(soft)* | **KANITLI** | `TRF_020` (gereken yürüme hızı çok yüksek) + `TRF_011` (mesafe uzak). |
| `P64ae76a9` | — | Aynı sefer çifti için eşit özgüllükte iki transfer olmamalı *(soft)* | **KANITLI** | `TRF_012` (yinelenen aktarma) + `TRF_016` (çelişkili koşul). |
| `Pf8c9263a` | — | Linked trips ile `block_id` çelişirse linked trips geçerlidir | **KAPSAM DIŞI** | Öncelik kuralı — tüketiciye hangisini okuyacağını söyler. |
| `P44a6984b` | — | n-to-n devamlılıkları her iki kısıtı da sağlamalı | KISMİ ⚠️ | `TRF_016` çelişkili koşulu ölçüyor ama bu cümlenin bağlamı (spec'in devamlılık örneği) tek başına muğlak. **Kaba cümle bölmenin sınırı** — hüküm gövdesi önceki cümlelerde. |

## pathways.txt

| id | alan | hüküm | karar | dayanak |
|---|---|---|---|---|
| `Pc8bfc111` | is_bidirectional | Çıkış kapısı (`pathway_mode=7`) çift yönlü olamaz | **KANITLI** | `PTH_016`. |
| `Pde6fb4a5` | — | "No locked platforms": pathway'i olan istasyonda her platform/boarding area bir girişe zincirle bağlı olmalı | **KANITLI** | `PTH_012` — `k6_analytics.rs:7240`; iç içe modelleme (platform → boarding area) hesaba katılıyor (`:7287`). |
| `Pcc725763` | max_slope | Yalnız yürüme yolu (1) ve yürüyen bant (3) ile kullanılmalı *(soft)* | **KANITLI** | `PTH_028` (08-03'te `PTH_017`'den ayrıldı — tavsiye/norm ayrımı). |
| `P3eeff781` | length | Walkway (1), fare gate (6), exit gate (7) için önerilir *(soft)* | **KANITLI** | `PTH_025` — koşul `matches!(pathway_mode, Some(1 \| 6 \| 7))` (`pathways.rs:70`), spec'le **birebir**. |
| `P707340ae` | stair_count | Merdiven (`pathway_mode=2`) için önerilir *(soft)* | **KANITLI** | `PTH_008` (feed düzeyi özet; yürüme yolu sayılmıyor). |
| `P24907079` | pathway_mode | 6 = ödeme kanıtı gereken alana geçiş | **META** | Enum değerinin tanımı. Geçerlilik `PTH_023`. |
| `P41bda33f` · `P5b7bc846` · `P3aa1cfdf` | — | Pathway varsa tüm bağlantılar tanımlı sayılmalı; sarkan konum olmamalı; platformun tüm boarding area'larına atanmalı *(soft)* | **KANITLI** | `PTH_012` (erişilemez) + `PTH_019` (dead-end generic node). |
| `P6863d663` | stair_count | Tahminse kat başına ~15 basamak varsayılmalı *(soft)* | **KAPSAM DIŞI** | Üreticiye tahmin yöntemi öneriyor; feed'den doğrulanamaz. |
| `P16572364` | signposted_as | Metin tabelada yazdığı gibi olmalı *(soft)* | **KAPSAM DIŞI** | Fiziksel tabela bilgisi gerekir. Uzunluk `PTH_018`. |
| `Pd498d7a2` | min_width | Genişlik 1 metreden azsa önerilir *(soft)* | **KAPSAM DIŞI** | Alan boşken gerçek genişlik bilinmediği için koşul değerlendirilemez — mantıksal olarak ölçülemez tavsiye. |
| `Pd21dea02` | traversal_time | Yürüyen bant (3), yürüyen merdiven (4), asansör (5) için önerilir *(soft)* | **BOŞLUK** | Aşağıda. |
| `P2264440d` | — | Platformun boarding area'ları varsa platforma pathway atanmamalı | **BOŞLUK** | Aşağıda. |

---

## Bulgu 7: `traversal_time` tavsiyesi ölçülmüyor — **BOŞLUK** (soft)

Spec `traversal_time` için *"recommended for moving sidewalks (3), escalators (4) and
elevator (5)"* der. Kodda `PTH_007` var ama o **değer varsa geçerliliğini** ölçüyor
(`parse_positive_u32`, `pathways.rs:76`) — eksikliği değil.

Bu, `length` için `PTH_025`'in yaptığı işin birebir analoğudur ve `PTH_025`'in koşulu
spec'le tam örtüşüyor (`pathway_mode` 1/6/7). Aynı desen `stair_count` için de var
(`PTH_008`). Üç öneri alanından ikisi ölçülüyor, `traversal_time` atlanmış.

Yumuşak → Quality. Kural şekli hazır: `PTH_025` deseninin `pathway_mode` 3/4/5 kopyası.

## Bulgu 8: Boarding area varken platforma pathway atanması — **BOŞLUK**

Spec, platformun boarding area'ları modellendiğinde pathway'lerin **boarding area'lara**
bağlanmasını ister ve ekler: *"In such cases, the platform must not have pathways assigned."*
Bu sert bir yasak (`must not`).

Kod bu modellemeyi **tolere ediyor** — `k6_analytics.rs:7287` iç içe yapıda asıl pathway
düğümünü çözerken platform ve boarding area'yı birlikte değerlendiriyor — ama platforma
pathway atanmasını **yasak olarak ölçmüyor**.

⚠️ Kural yazılmadan önce ölçülmeli: bu modelleme vahşi doğada yaygınsa yeni kural gürültü
üretir. `PTH_014`/`PTH_026` komşu olguları ölçüyor, çakışma riski var.

---

# 5. tur — `translations.txt` (17) + `booking_rules.txt` (16)

## translations.txt

| id | alan | hüküm | karar | dayanak |
|---|---|---|---|---|
| `P7c9deec0` · `P5885e3d5` · `P20d92e6a` | record_id · record_sub_id · field_value | `table_name=feed_info` ise yasak | **KANITLI** | `TRN_013`. |
| `Pcc1b72af` · `P65f312f1` | record_id · record_sub_id | `field_value` tanımlıysa yasak | **KANITLI** | `TRN_009`. |
| `P608c78cf` | field_value | `record_id` tanımlıysa yasak | **KANITLI** | `TRN_009` (aynı karşılıklı dışlama). |
| `Pdc0b679c` · `Pf93f15ba` | record_id · field_value | Diğeri boşsa zorunlu | **KANITLI** | `TRN_015` (08-02'de eklendi; `table_name` geçersizse susacak şekilde daraltılmıştı). |
| `Pcf0597aa` | record_id | Tablonun birincil anahtarının ilk/tek alanı olmalı | **KANITLI** | `TRN_004` (record_id bulunamadı) — çözümleme birincil anahtar üzerinden. |
| `P7c7134fe` · `P4adfb063` · `P502d68a6` | field_name · record_id · record_sub_id | Diğer tiplerdeki/tablolardaki alanlar çevrilmemeli *(soft)* | **KANITLI** | `TRN_011` (field_name çevrilebilir değil) + `TRN_002`. |
| `P2c840d38` | field_value | Alan tam olarak `field_value`'daki değere sahip olmalı | KISMİ | `XFL_014` çözümlenemeyen çeviri referansını yakalar; `field_value` eşleşmesinin **tam** olduğunu doğrulayan ayrı ölçüm yok. |
| `Pf3f9b74e` · `P95d8f3ea` | record_id · record_sub_id | Tablo başına önerilen kullanım listesi *(soft)* | **META** | Rehber tablosu; `TRN_004`/`TRN_014` zaten yapıyı denetliyor. |
| `P403cf2f3` | field_value | `record_id` yerine alternatif kullanım *(soft)* | **META** | Alanın ne işe yaradığını anlatıyor. |
| `P9373b1fa` | record_sub_id | `table_name=stop_times` **ve** `record_id` tanımlıysa **zorunlu** | **BOŞLUK** | Aşağıda. |

## booking_rules.txt

| id | alan | hüküm | karar | dayanak |
|---|---|---|---|---|
| `Pffdf614c` | prior_notice_duration_min | `booking_type=1` için zorunlu | **KANITLI** | `BKR_007`. |
| `P075bfafe` | prior_notice_duration_min | Aksi hâlde yasak | **KANITLI** | İki kural bölüşüyor: `BKR_012` `booking_type=2` kolunu (`booking_rules.rs:172`), `BKR_004` `booking_type=0` kolunu (`:144`, tüm prior_notice alanları birlikte). Kod yorumu bölüşmeyi açıkça yazıyor. |
| `P9cf0fc58` | prior_notice_duration_max | `booking_type` 0 ve 2 için yasak | **KANITLI** | `BKR_005` (type=2) + `BKR_004` (type=0) — aynı bölüşme. |
| `P3bb1febc` · `Pc17af87b` | prior_notice_last_day | `booking_type=2` için zorunlu, aksi yasak | **KANITLI** | `BKR_008` + `BKR_001` (`btype != 2 && has_last_day`, yani 0 ve 1'i birlikte kapsar). |
| `P64e8ffb7` · `P7d35adee` | prior_notice_last_time | `last_day` tanımlıysa zorunlu, aksi yasak | **KANITLI** | `BKR_009` + `BKR_013`. |
| `P285e1ced` | prior_notice_start_day | `booking_type=0` için yasak | **KANITLI** | `BKR_004`. |
| `P5db20825` · `Pa2eda5e0` | prior_notice_start_time | `start_day` tanımlıysa zorunlu, aksi yasak | **KANITLI** | `BKR_010` + `BKR_003`. |
| `P1d4947e7` · `Pd78aeee0` | prior_notice_service_id | `booking_type=2` ile opsiyonel, aksi yasak | **KANITLI** | `BKR_014`. |
| `Pe467fc5a` · `P7dbb186e` | prior_notice_last_day · last_time | Kodlama örneği ("1 gün önce 17:00'ye kadar") | **META** | Örnek; ihlal edilebilir bir yasak koymuyor. |
| `P3b3a8cc2` | message | Yolcuya iletilecek asgari bilgiyi taşımalı | **META** | Alan tanımı. |
| `P5a5cced5` | prior_notice_start_day | `booking_type=1` iken `prior_notice_duration_max` tanımlıysa **yasak** | **BOŞLUK** | Aşağıda. |

---

## Bulgu 9: Yanlış alarm verecektim — kod yorumu zaten cevaptı

`BKR_012` yalnız `btype == 2` denetliyor, oysa spec *"Required for booking_type=1. **Forbidden
otherwise**"* der — `booking_type=0` da yasak demektir. İlk okumada boşluk sandım.

Kuralın kendi doc yorumu cevabı yazıyordu: *"type=0 kolu BKR_004'te (tüm prior_notice alanları)
— burada tekrar edilmez."* Ve `booking_rules.rs:144` gerçekten `btype==0 && (has_duration_min ||
has_duration_max || has_last_day || …)` denetliyor. Aynı bölüşme `BKR_005` için de geçerli.

**Ders (tekrar):** MD/parite listelerinin %38'i yanlış alarmdı; burada da grep sonucuna bakıp
"eksik" demek üzereydim. **Kuralın doc yorumu, kuralın kapsamı hakkındaki ilk kaynaktır** —
bu repoda yorumlar spec cümlesini ve bölüşmeyi düzenli olarak yazıyor.

## Bulgu 10: `record_sub_id` gereklilik yönü ölçülmüyor — **BOŞLUK** (`P9373b1fa`)

Spec iki yönlü hüküm koyar:
- **Yasak:** `stop_times` dışındaki tablolarda `record_sub_id` kullanılamaz → `TRN_014` ✅
  (`translations.rs:229` — `table_name != "stop_times" && record_sub_id.is_some()`).
- **Zorunlu:** `table_name=stop_times` **ve** `record_id` tanımlıysa `record_sub_id` gerekir →
  **ölçülmüyor.**

`stop_times.txt`'in birincil anahtarı bileşiktir (`trip_id` + `stop_sequence`); `record_id`
yalnız ilk alanı taşır, `record_sub_id` ikincisini. İkincisi olmadan çeviri **hangi satıra**
ait olduğunu söyleyemez — çözümlenemeyen değil, **belirsiz** bir referans.

Sert hüküm (`Conditionally Required`). Kural adayı; `TRN_014`'ün ters kolu olarak yazılabilir.

## Bulgu 11: `start_day` / `duration_max` çakışması ölçülmüyor — **BOŞLUK** (`P5a5cced5`)

Spec: `prior_notice_start_day` *"Forbidden for booking_type=1 if prior_notice_duration_max is
defined."* Yani aynı gün rezervasyonda üst süre sınırı varken başlangıç günü çelişkilidir.

`prior_notice_duration_max` kodda yalnız üç yerde geçiyor (`booking_rules.rs:129/137/157`) ve
hiçbiri `start_day` ile ilişkilendirilmiyor. `BKR_002` komşu ama farklı hükmü ölçer
(`start_day` yalnız `last_day` ile kullanılabilir).

Sert hüküm. `BKR_005`'in yanına yazılabilir; `booking_type=1` dalı bugün tamamen sessiz.

---

## Sonraki tur

Kalan 101 aday. Yoğunluk: `fare_transfer_rules.txt` 10 · `dataset-publishing-general-practices` 11 ·
`agency.txt` 8 · `trips.txt` 8 · `feed_info.txt` 8 · `fare_leg_rules.txt` 8 ·
`fare_leg_join_rules.txt` 8 · `shapes.txt` 6 · `fare_attributes.txt` 6 · `timeframes.txt` 6.

Açık kalemler (kural yazımı bekleyen, hiçbiri yazılmadı): URL şeması (ölçüm bekliyor) ·
HTML etiketi · OpenGIS poligon · `pickup_type=2` booking rule · `platform_code` ·
`traversal_time` tavsiyesi · boarding area/platform pathway yasağı (ölçüm bekliyor).
