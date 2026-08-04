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

# 6. tur — Fares ailesi (44 aday, dokuz dosya)

Bu ailenin en belirgin özelliği: **sert hükümlerin büyük kısmı tüketici algoritmasıdır.**
Spec, fare_leg_rules ve fare_transfer_rules bölümlerinde ücretin nasıl **sorgulanacağını**
adım adım anlatır ("filtrele, tam eşleşme ara, yoksa boş girdilere bak") ve bunları `must`
ile yazar. Bunlar bağlayıcıdır ama **veriyi değil tüketiciyi** bağlar — feed'den doğrulanamaz.

| id | alan | hüküm | karar | dayanak |
|---|---|---|---|---|
| `P99885c9f` | transfer_count | Leg grupları farklıysa yasak | **KANITLI** | `FTR_010`. |
| `P706ade03` | transfer_count | Leg grupları aynıysa zorunlu | **KANITLI** | `FTR_009` (koşul `fare_transfer_type`'a değil grup eşitliğine bağlı — 06-17'de MD notice tanımıyla düzeltilmişti). |
| `Pc2fcfe46` | duration_limit_type | `duration_limit` tanımlıysa zorunlu | **KANITLI** | `FTR_011`. |
| `P4882cfae` | duration_limit_type | `duration_limit` boşsa yasak | **KANITLI** | `FTR_007`. |
| `P14538146` | duration_limit | Süre sınırı yoksa boş olmalı | **META** | Tautoloji; ihlal edilebilir bir durum tanımlamıyor. Değer geçerliliği `FTR_006`. |
| `P9cee47a9` | leg_group_id | Aynı giriş (`leg_group_id` hariç) birden çok leg grubuna ait olamaz | **KANITLI** | `DQ_021` — `k6_analytics.rs:4051`, altı alanlık bileşik anahtarı (`network_id\|from_area_id\|to_area_id\|from_timeframe_group_id\|to_timeframe_group_id\|fare_product_id`) birebir denetliyor. Spec'in birincil anahtarı zaten `leg_group_id` içermiyor. |
| `Pdb20f308` · `P1efa6e43` | from_network_id · to_network_id | Biri varsa diğeri de olmalı | **KANITLI** | `FLJ_001` · `FLJ_002`. ⚠️ Korpusta `fare_leg_join_rules.txt` taşıyan feed YOK → gerçek veri kanıtı yok. |
| `P278f9cc2` · `Pd90f72d8` | from_stop_id · to_stop_id | Biri varsa diğeri de zorunlu | **KANITLI** | `FLJ_003` · `FLJ_004` (aynı uyarı). |
| `Pe15614d2` | agency_id | Çoklu ajans varsa zorunlu | **KANITLI** | `AGN_011` — 08-02'de tam bu hüküm için `fare_attributes`'a genişletilmişti. |
| `P31c88c29` · `P71c9ce23` | payment_method | Ödemenin ne zaman yapılacağını belirtir; 1 = binişten önce | **KANITLI** | `FAR_004` (enum geçerliliği) + `FAR_011` (eksik). Cümleler enum tanımı. |
| `Pa586fe5b` | — | Aynı `timeframe_group_id` + `service_id` için örtüşen aralık olamaz | **KANITLI** | `TFR_005`. |
| `P6fa9925a` · `P4f2430ed` · `P8cc81b88` · `P7b4083a2` | start_time · end_time | Biri varsa diğeri zorunlu, aksi yasak | **KANITLI** | `TFR_007` (tek kural, çift yönlü). |
| `P1d9a0191` | is_default_fare_category | Bir `fare_product_id` için tam bir varsayılan kategori olmalı | **KANITLI** | `RCT_006` + `FPD_006` (aynı olgunun iki dosyadaki ucu). |
| `P79f202e6` | rider_category_id | Aynı hüküm, `fare_products` tarafı | **KANITLI** | `FPD_006`. |
| `P152c436d` | is_default_fare_category | Varsayılan kategori tanımı *(soft)* | **META** | Geçerlilik `RCT_003`. |
| `P32e7117f` | fare_media_name | Kart (2) ve mobil uygulama (4) için önerilir *(soft)* | **KANITLI** | `FMD_003` — koşul birebir. |
| `Pc29f644f` | agency_id | Aksi hâlde önerilir *(soft)* | KISMİ | `RTS_025`'in `routes` için yaptığını `fare_attributes` için yapan kural yok. Yumuşak, Quality; düşük değer. |
| `Pcc091079` | contains_id | Bölge örneği ("c sınıfı 5, 6, 7 bölgelerinden geçer") | **META** | Örnek. Geçerlilik `FRL_005`. |
| `P8cf7f90f` · `P3c1a4156` | — | V1 ve V2 birlikte bulunabilir; tüketici birini seçmeli, V2 tercih edilmeli *(soft)* | **KAPSAM DIŞI** | Tüketiciyi bağlar. |
| `Pbf8f7f62` | — | Belirtilmemişse ajans saat dilimi kullanılmalı *(soft)* | **KAPSAM DIŞI** | Tüketici çözümlemesi. |
| `P346477ee` · `P08611e2a` | from/to_timeframe_group_id | Saat dilimi çözümlemesinde kalkış/varış durağı kullanılmalı *(soft)* | **KAPSAM DIŞI** | Tüketici çözümlemesi. |
| `Pa13f977d` · `P25918782` · `P4e4360f8` · `P203b24af` · `Pbbc4e012` | — | fare_leg_rules sorgulama algoritması (filtrele → tam eşleşme → boş girdiler) | **KAPSAM DIŞI** | Sert (`must`) ama tüketici algoritması. |
| `P4dead8a8` · `P34b16ddb` · `P2b1854d3` · `Pb6d9a2c2` · `Pe8b16d97` | — | fare_transfer_rules sorgulama algoritması | **KAPSAM DIŞI** | Aynı gerekçe. |
| `P9a687aee` · `P801d96f0` · `P9bb795c0` · `P66fe5cb3` | — | fare_leg_join_rules eşleştirme algoritması | **KAPSAM DIŞI** | Aynı gerekçe. |
| `P38cd4e78` | amount | Tutar, para biriminin ISO 4217 ondalık basamak sayısını taşımalı | **BOŞLUK** | Aşağıda. |

---

## Bulgu 12: ISO 4217 ondalık basamak sayısı ölçülmüyor — **BOŞLUK** (`P38cd4e78`)

Spec: *"The currency amount must contain the number of decimal places specified by the norm
ISO 4217 for the accompanying Currency code."*

Kodda **para birimi kodunun** geçerliliği ölçülüyor (`FPD_003`, `FAR_003` — üç harfli ISO 4217
listesi) ama **minor-unit tablosu hiç yok**: `grep -rn "4217\|decimal_digits\|minor_unit"`
yalnız hata mesajlarını buluyor.

Somut ihlaller: `JPY 100.00` (yen'in ondalığı 0), `USD 2.5` (2 basamak olmalı: `2.50`),
`KWD 1.5` (dinar 3 basamak). Bunlar bugün sessiz geçiyor.

Sert hüküm. Kural yazılabilir ama **ISO 4217 minor-unit tablosu gerekir** (~180 satır sabit
veri, çoğu 2). Değeri: yanlış ondalık, tüketicide 100× fiyat hatasına dönüşebilir.

⚠️ `FPD_002`/`FAR_002` yalnız negatif/sayısal olmayan değeri ölçüyor; bu farklı bir olgu.

## Bulgu 13: İkinci yanlış alarm da kod okumasıyla önlendi

`P9cee47a9` için "leg grubu benzersizliği ölçülmüyor" diyecektim — `fare_leg_rules.rs`'te
benzersizlik araması boş döndü. Ama olgu `DQ_021`'de, **dosya bazında değil genel birincil
anahtar kuralı olarak** (`k6_analytics.rs:4051`), ve altı alanlık bileşik anahtarı birebir
yazıyor.

Bu, aynı oturumda `BKR_012`'den sonra **ikinci** yanlış alarm denemesi. İkisinin de deseni
aynı: **hükmün karşılığı, aradığım dosyada değil genel bir kuralda.** `grep` dosya bazlı
düşünmeye itiyor; bu repoda `DQ_021` gibi çapraz kurallar tam bu boşlukları kapatıyor.

---

# 7. tur — kalan 57 aday (on iki bölüm)

## agency.txt · attributions.txt · frequencies.txt · levels.txt · location_groups.txt

| id | alan | hüküm | karar | dayanak |
|---|---|---|---|---|
| `P5a1c0ffc` | agency_id | Birden çok ajans varsa zorunlu | **KANITLI** | `AGN_014`. |
| `Pb135ca49` | agency_id | Aksi hâlde önerilir *(soft)* | **KANITLI** | `AGN_011` (`FIN_013` ile birlikte). |
| `Pf27a2fd0` | agency_timezone | Çoklu ajansın hepsi aynı saat dilimini taşımalı | **KANITLI** | `AGN_005`. |
| `Pbb82f19e` | agency_phone | Çevrilebilir metin izinli, başka açıklama **yasak** | **KANITLI** | `AGN_007` (+ `AGN_016` yer-tutucu numaralar). `looks_like_phone` harf içeren değeri reddediyor — TriMet'in "503-238-RIDE" biçimi ⚠️ **spec'te açıkça izinli ama bizim doğrulayıcımız reddeder.** Aşağıda. |
| `P130f8673` | agency_email | Yolcunun ulaşabileceği doğrudan adres olmalı *(soft)* | KISMİ | `AGN_009` biçimi ölçer; "doğrudan temas noktası" doğrulanamaz. |
| `P722127e8` · `P3005b227` | cemv_support | `routes.cemv_support` önceliklidir; fare dosyalarıyla çelişmemeli | **KANITLI** | `XFL_017` (route↔agency çelişkisi) + `XFL_028`/`XFL_030`. |
| `P572bb984` | cemv_support | Yalnız tüm hizmetler cEMV kabul ediyorsa bildirilmeli *(soft)* | KISMİ | `XFL_028` yakınsıyor; "tüm hizmetler" koşulu ölçülmüyor. |
| `P7917f3c5` | agency_id | `agency_id`/`route_id`/`trip_id` attribution'larından biri varsa diğerleri boş olmalı | **KANITLI** | `ATR_009`. |
| `Pebcbebf1` | is_producer | `is_producer`/`is_operator`/`is_authority`'den en az biri 1 olmalı *(soft)* | **KANITLI** | `ATR_003`. |
| `Pec6b6920` | headway_secs | Aynı sefer için birden çok headway tanımlanabilir ama **çakışamaz** | **KANITLI** | `FRQ_011`. |
| `P257db6b1` | exact_times | `end_time`, son istenen sefer başlangıcından büyük olmalı | KISMİ | `FRQ_005` (end < start) ve `FRQ_009` komşu; cümlenin `exact_times=1`'e özgü inceliği ölçülmüyor. |
| `P1ecc6733` | level_index | Zemin 0, üstü pozitif, altı negatif *(soft)* | KISMİ | `LVL_002` sayısal geçerliliği ölçer; zemin referansı feed dışı bilgi. |
| `P24d95df2` | location_group_id | Üç kaynak genelinde benzersiz | **KANITLI** | `XFL_031` (hükmün üçüncü ucu; 1. turda `P042ba79f`/`P1afc582a`). |

## shapes.txt

| id | alan | hüküm | karar | dayanak |
|---|---|---|---|---|
| `P67d6bd72` | shape_pt_sequence | Artmalı, ardışık olmak zorunda değil | **KANITLI** | `SHP_004` + `SHP_008` (yinelenen). ✅ **İzin tarafı doğrulandı:** ardışık olmayan diziye ateşleyen kural yok — `stop_sequence` için yapılan aynı kontrol. |
| `P2a0dbcd7` | shape_dist_traveled | `shape_pt_sequence` ile artmalı, ters seyahat göstermemeli | **KANITLI** | `SHP_005` + `SHP_021` + `SHP_028`. |
| `Pdef3d025` | shape_dist_traveled | Birimler `stop_times.txt` ile tutarlı olmalı | **KANITLI** | `STM_024` (birim tutarsızlığı) + `SHP_024`/`SHP_025`. |
| `P984ec73b` | shape_dist_traveled | Döngü/iç içe hatlarda önerilir *(soft)* | KISMİ | `SHP_017` komşu; döngü koşulu ölçülmüyor (`stop_times` tarafındaki `P7c96867d` ile aynı durum). |
| `Pd03c58da` | — | Duraklar shape'i tam kesmese de küçük mesafede olmalı *(soft)* | **KANITLI** | `SHP_012` · `SHP_014` · `GEO_009`. |
| `Pecd3617b` | — | Rota tabanlı hizmetler için `shapes.txt` bulunmalı, bölge tabanlı DRT için gerek yok *(soft)* | **KANITLI** | `RTS_017` (shape'siz hat) + `ARC_020`'nin DRT muafiyeti ([[project_arc020_drt_exemption]]) — istisna da doğru modellenmiş. |

## trips.txt

| id | alan | hüküm | karar | dayanak |
|---|---|---|---|---|
| `P03987db8` | shape_id | Rotada/stop_times'ta continuous pickup/drop-off varsa zorunlu | **KANITLI** | `TRP_019` (08-03'te Quality→**Spec**). |
| `P6f633bee` | trip_headsign | Araçta headsign gösteren tüm hizmetler için önerilir *(soft)* | **KANITLI** | `TRP_011`. |
| `Pfe2abba1` | trip_short_name | Verilmişse servis günü içinde benzersiz olmalı *(soft)* | KISMİ | `TRP_014` uzunluğu ölçer; servis günü içi benzersizlik ölçülmüyor. |
| `P98d7e2a2` | trip_short_name | Yolcular sefer adı kullanmıyorsa boş olmalı *(soft)* | **KAPSAM DIŞI** | `STP_022` ile aynı desen — "kullanılmıyorsa boş" feed'den bilinemez. |
| `Pfe1eb2fc` | direction_id | Yönlendirmede kullanılmamalı *(soft)* | **KAPSAM DIŞI** | Tüketiciyi bağlar. |
| `Pa2571719` | block_id | In-seat aktarma için `transfer_type=4` tercih edilmeli *(soft)* | **KAPSAM DIŞI** | Modelleme tercihi. `TRF_014`/`Pf8c9263a` komşu. |
| `P2c5055df` · `P7411f6b1` | — | Talep-üzerine bölüm hesaplaması; sapmalı-sabit hizmet *(soft)* | **KAPSAM DIŞI** | Tüketici hesaplaması. |

## feed_info.txt · calendar_dates.txt

| id | alan | hüküm | karar | dayanak |
|---|---|---|---|---|
| `P3e2e5c05` | feed_start_date | `feed_end_date`, `feed_start_date`'ten önce olamaz | **KANITLI** | `FIN_012`. |
| `Pc78e53ef` · `P3834f860` | feed_contact_email · feed_contact_url | En az biri sağlanmalı *(soft)* | **KANITLI** | `FIN_018`. |
| `P11f6523f` | feed_start_date | Bu dönem dışında da veri verilmesi önerilir *(soft)* | KISMİ | `FIN_016`/`FIN_017`/`CAL_019` komşu pencereleri ölçüyor. |
| `P637f73ea` | default_lang | Tüketici yolcunun dilini bilmiyorsa kullanılacak dil *(soft)* | **KANITLI** | `FIN_004` (geçerlilik). Cümle alan tanımı. |
| `P7cf5ea86` · `Pcbd2b455` · `Pa71d256d` | feed_lang | Çok dilli veride `mul` kullanılmalı ve çeviriler `translations.txt`'te olmalı; tek dilliyse `mul` kullanılmamalı *(soft)* | KISMİ ⚠️ | Aşağıda. |
| `Pd7cb6983` | — | `calendar_dates.txt` `calendar.txt` ile birlikte istisna tanımlamak için kullanılmalı *(soft)* | **KANITLI** | `ARC_008` (takvim çifti) + `CAL_006`/`CAL_018`. |

## dataset-publishing-general-practices (11, tamamı soft)

| id | tavsiye | karar | dayanak |
|---|---|---|---|
| `Pc6e7b067` · `Pa73d1591` | Kalıcı ve genel URL'de, zip adıyla yayınlanmalı | **KANITLI** | `ARC_028` — `k6_analytics.rs:3562`, sorgu dizesini ayıklayıp `.zip` uzantısını denetliyor. |
| `P308aa743` | Yayındaki veri en az 7 gün geçerli olmalı | **KANITLI** | `FIN_019` · `FIN_020` · `CAL_024` · `TRP_023`. |
| `P7a8ab180` | Süresi dolmuş takvimler kaldırılmalı | **KANITLI** | `CAL_013` · `CAL_009`. |
| `P5cb1a139` | Mümkünse 30 günü kapsamalı | KISMİ | 7 günlük eşik ölçülüyor, 30 günlük ayrı eşik yok. |
| `P0d80d05e` · `P99b3d7c3` | Login'siz indirilebilmeli; kapalı dağıtım istisnası | **KAPSAM DIŞI** | Barındırma politikası. (Açık issue #55 auth'lu feed'ler bu alanla ilgili.) |
| `P3f03b3f2` · `P194640d1` · `P1d4f536a` | Yinelemeli yayın; birleşik veri seti; 7 gün içindeki değişiklik `calendar_dates` ile | **KAPSAM DIŞI** | Yayın süreci; tek bir feed'den doğrulanamaz. |
| `Pc692d295` | `stop_id`/`route_id`/`agency_id` sürümler arası kalıcı olmalı | **KAPSAM DIŞI** | İki sürüm karşılaştırması gerekir — ürünün "karşılaştırma" özelliği bunu yapar ama kural değil. |
| `P65ebae47` | Web sunucu dosya değişiklik tarihini doğru bildirmeli | **KAPSAM DIŞI** | HTTP sunucu yapılandırması. |

## presence · term-definitions (9)

| id | içerik | karar |
|---|---|---|
| `P092897e2` · `P2f336837` · `Pf1ef25c1` · `Pb7cdf168` · `P3df40438` | `Required` / `Conditionally Required` / `Conditionally Forbidden` / `Recommended` tanımları | **META** — spec'in kendi terim sözlüğü. Bu tanımların **uygulaması** 302 hüküm atomudur ([[project_spec_conformance_2026_08]]); tanımın kendisi ölçülecek bir şey değil. |
| `P90cd23c4` | "Effective Fare Leg" tanımı | **META** |
| `Pa73d1591` | (yayın URL'si — yukarıda) | — |
| `P8bc2d625` · `Pd556e682` | TTS alanı üst alanla aynı bilgiyi taşımalı; kısaltmalar açılmalı *(soft)* | **KANITLI** — `STP_023` (`tts_stop_name` geçersiz). |

---

## Bulgu 14: `agency_phone` — spec'in açıkça izin verdiği biçimi reddediyoruz

Spec: *"Dialable text (for example, TriMet's **"503-238-RIDE"**) is permitted, but the field
must not contain any other descriptive text."*

`looks_like_phone` (`common.rs:227`) yalnız rakam ve `+ - ( ) . boşluk` kabul eder; **harf
içeren her değeri reddeder.** Spec'in kendi örneği (`503-238-RIDE`) bugün `AGN_007` ateşler.

⚠️ **Bu `PTH_017` sınıfı bir hata:** spec'in açıkça *permitted* dediği bir biçimi ihlal
sayıyoruz. Yönü önemli — geçerli veriyi yanlış işaretlemek, kaçırmaktan ağırdır.

Hafifletici: `AGN_007` **Quality** sınıfında (kontrol edildi), yani R1 yayın kapısını
etkilemiyor. Yine de yanlış pozitiftir ve ABD feed'lerinde vanity numaralar yaygındır.

**Yapılmadı:** düzeltme, "dialable text" ile "descriptive text" ayrımını gerektirir —
`503-238-RIDE` geçerli, `Call us at 503-238-1234 during business hours` değil. Sezgisel bir
kural (harf grupları telefon tuş takımına eşlenebiliyorsa ve ayrı kelime yoksa) yazılabilir.
Önce korpusta kaç feed'in etkilendiği ölçülmeli.

## Bulgu 15: `feed_lang = mul` — yanlış pozitif YOK, ama eşlik eden hüküm ölçülmüyor

Önce riskli olanı kontrol ettim: `mul` reddediliyor mu? **Hayır** — `looks_like_bcp47`
(`common.rs:216`) alt etiketlerin alfanumerik ve ≤8 karakter olmasını istiyor, `mul` geçiyor.
Yani `FIN_003` yanlış pozitif üretmiyor.

Ölçülmeyen kısım: spec `mul` kullanıldığında çevirilerin `translations.txt`'te bulunmasını
ister, ve tersine tek dilli veride `mul` kullanılmamasını. Bugün ikisi de sessiz.

Yumuşak (*should*) → Quality. Değeri orta: `mul` ilan edip çeviri koymayan bir feed,
tüketiciye adların dilini söyleyememiş olur.

---

# ✅ KATALOG TAMAMLANDI — 273/273

Aşağıdaki dağılım **sayıldı, tahmin edilmedi** — defterdeki her satırın karar etiketi
ile o satırdaki aday kimlikleri eşleştirilerek (`P` kimliği başına bir kez):

| karar | aday | oran |
|---|---|---|
| **KANITLI** — kural doğrudan ölçüyor | 155 | %57 |
| **KAPSAM DIŞI** — tüketiciyi bağlar / feed'den doğrulanamaz | 57 | %21 |
| KISMİ — komşu kural var, tam örtüşmüyor | 27 | %10 |
| **META** — tanım, örnek, rehber | 18 | %7 |
| **BOŞLUK** — hiçbir kural ölçmüyor | 11 | %4 |
| DOLAYLI — başka kurallarca yakalanıyor, adı konmamış | 4 | %1 |

⚠️ **11 boşluk ADAYI = 9 ayrı HÜKÜM.** Spec aynı tavsiyeyi birden çok cümleye bölüyor
(`platform_code` iki cümle, `feed_lang mul` üç cümle); aşağıdaki liste hükümleri sayar.

## Boşluklar — ölçüldü, sonra kapatıldı (durum: 2026-08-03)

Üreteç: `spec-audit/measure_gaps.py` (statik zip+csv taraması, doğrulayıcı koşulmaz).
Başlıklar `csv.reader` ile okunup normalize edilir — 08-02'de iki kez uydurma sonuç veren
tırnak/boşluk tuzağına karşı.

⚠️ **İlk yazımda bu tablo dokuz satırdı ve BİR BOŞLUĞU ATLIYORDU** (`pickup_type=2`
booking rule tavsiyesi, 2. turda bulgu olarak yazılmıştı ama özete girmemiş, dolayısıyla
issue de açılmamıştı). Doğru sayı **10 hüküm / 12 aday**.

| # | hüküm | sert | feed | bulgu | durum |
|---|---|---|---|---|---|
| 1 | Değerde **HTML etiketi** (`Pd6bc0278`) | ✅ | 4 | 1955 | ✅ **KAPANDI → `ARC_032`** (`0ed19f93`) |
| 2 | `traversal_time` tavsiyesi (`Pd21dea02`) | — | 4 | 24 | ✅ **KAPANDI → `PTH_029`** (`509c5b54`) |
| 3 | `platform_code` (`P3af6af7b`·`Pb24eacd3`) | — | 1 | 1 | ✅ **KAPANDI → `STP_044`** (`509c5b54`) |
| 4 | URL şeması (`P5f72fb5a`) | ✅ | 1 | 116 | ✅ **KAPANDI → `looks_like_url`** (`cfd8f7d4`) |
| 5 | ISO 4217 ondalık (`P38cd4e78`) | ✅ | 3 | 715 | 🔴 **KAPATILMAYACAK** — ölçüm kararı, aşağıda |
| 6 | `record_sub_id` gereklilik yönü (`P9373b1fa`) | ✅ | **0** | 0 | ⚪ AÇIK → issue **#66** |
| 7 | `start_day`/`duration_max` (`P5a5cced5`) | ✅ | **0** | 0 | ⚪ AÇIK → issue **#66** |
| 8 | Boarding area + platform pathway (`P2264440d`) | ✅ | **0** | 0 | ⚪ AÇIK → issue **#66** |
| 9 | OpenGIS poligon geçerliliği (`Pd84a0bcb`) | ✅ | — | ölçülmedi | ⚪ AÇIK → issue **#67** (bağımlılık kararı) |
| 10 | `pickup_type=2` booking rule (`P71243b3e`·`P504bde32`) | — | ölçülmedi | — | ⚪ **AÇIK, ISSUE YOK** — aşağıda |

**Ayrıca iki yanlış pozitif kapandı** (`50032512`): `AGN_007` artık vanity numarayı kabul
ediyor (spec'in kendi örneği), `STP_022`'nin kartı boş `stop_code`'un doğru olabileceğini
yazıyor. Hiçbir feed'in davranışı değişmedi.

### 10. boşluk: `STM_040` spec'in koşulunu ölçmüyor

Spec `pickup_booking_rule_id` için *"Recommended when `pickup_type=2`"* der (ve
`drop_off_type=2` için ikizini). `STM_040` ise **Flex penceresi** varken booking rule
eksikliğini ölçüyor (`k2/stop_times.rs:1571`). İkisi farklı popülasyon: `pickup_type=2`
("ajansı telefonla arayın") Flex penceresi olmadan da kullanılır.

Bu boşluk 2. turda bulgu olarak yazıldı ama özet tabloya girmedi ve issue açılmadı —
**tablonun kendisi bir kapsam kaybı yaşadı**, tam da bu defterin önlemek için var olduğu
şey. Ölçülmedi; yapılacak ilk iş `measure_gaps.py`'a eklemek.

### Ölçüm iki kararı değiştirdi

**HTML etiketi gerçek çıktı ve tahminimden büyük.** `mdb-1924` (Hong Kong) durak adlarında
`<BR>` taşıyor — 1176 durak: `[KMB+CTB] HIU TSUI STREET/<BR>HIU TSUI STREET, SIU SAI WAN ROAD`.
`mdb-3235` 774 daha. Bu adlar yolcuya gösteriliyor ve `<BR>` ham metin olarak görünüyor.
Spec'in *"must not contain HTML tags"* hükmünün tam hedefi.

**ISO 4217 için kural YAZILMAMALI.** İlk ölçümde `dec != want` kullandım → **2.001.806**
eşleşme, 38 feed. Sebep: `CZK 39` gibi ondalıksız tam sayılar korpusta evrensel ve tamamen
meşru okunuyor. Daraltıp yalnız **fazla** basamağı ölçtüm (`dec > want`) → 715 bulgu, 3 feed;
ama kalanların çoğu `HKD 0.0000` — biçimsel fazlalık, anlam kaybı yok. Değeri yok, gürültüsü
var.

### İki yanlış pozitifin ölçümü — bulgu 14 zayıfladı

`agency_phone`'da harf içeren değer korpusta **yalnız 2 feed'de** var ve ikisi de aynı
metni taşıyor: `80000078 (Liepājā); 80000079 (Pierīgā); 80700004 (…)`.

**Bu vanity numara DEĞİL** — spec'in *yasakladığı* "other descriptive text"tir. Yani
`AGN_007` bu iki feed'de **doğru** ateşliyor.

Spec'in örneklediği `503-238-RIDE` biçimi 239 feed'de **hiç geçmiyor.** Bulgu 14 teorik
olarak geçerli (izinli biçimi reddediyoruz) ama **pratikte hiçbir feed'i etkilemiyor.**
Düzeltmenin önceliği buna göre düşük; "dialable ↔ descriptive" ayrımını yazmak, ölçülen
hiçbir sorunu çözmez ve gerçek ihlalleri (yukarıdaki iki feed) kaçırma riski taşır.

⚠️ **Statik taramanın sınırı:** predicate'ler kural mantığını yaklaşık taklit eder ve
korpus 239 feed'dir — "0 bulgu" *"böyle veri yok"* değil, *"bu korpusta yok"* demektir.
`fare_leg_join_rules.txt` taşıyan feed'in olmaması gibi.

Ayrıca **iki yanlış pozitif** bulundu ve defterlendi: `agency_phone` vanity numaraları
(bulgu 14, `PTH_017` sınıfı) ve `STP_022`'nin spec'in açık iznine ters yönü (bulgu 5).
İkisi de Quality sınıfında olduğu için yayın kapısını etkilemiyor.

## Kataloğun kör noktası ÖLÇÜLDÜ (2026-08-03)

Defter baştan beri şunu yazıyordu: üreteç modal-tabanlıdır, *"All file and field names are
case-sensitive"* gibi modalsiz hükümleri **yapısal olarak kaçırır**, dolayısıyla 273 bir
**alt sınırdır**. Bu kör noktanın büyüklüğü artık tahmin değil.

Üreteç: `spec-audit/measure_modalless.py` — `extract_provisions.py`'ın ELEDİĞİ cümleleri
toplar ve normatif-olabilir sinyallere göre ayırır.

```
modal TAŞIYAN cümle (katalogda)   : 273
modal TAŞIMAYAN cümle             : 888
  ├─ normatif sinyal taşıyan      : 124
  │    ├─ betimleyici kalıp da var:  20   (düşük olasılık)
  │    └─ SAF ADAY                : 104
  └─ hiç sinyal yok               : 764   (büyük olasılıkla betimleyici)
```

### 104 saf adayın kategorileri — ve neden payda büyümüyor

| kategori | adet | değerlendirme |
|---|---|---|
| koşul / miras semantiği | ~45 | *"If this field is empty, the stop_time inherits any continuous pickup behavior…"* — tüketiciye **nasıl yorumlayacağını** söyler. İhlal edilemez, dolayısıyla ölçülemez. |
| öncelik / geçersiz kılma | 9 | *"This field overrides the default trips.trip_headsign…"* — aynı sınıf. |
| enum değer tanımı | ~13 | *"2 - cEMVs are not supported…"*, *"1 - Vehicle can accommodate at least one rider"* — değerin **anlamı**, hüküm değil. Geçerlilik zaten enum kurallarında. |
| tip tanımı | ~9 | *"Non-negative - Greater than or equal to 0."* — alan tablosu ekseninde `numeric` atomu olarak ölçülüyor. |
| bölüm başlığı | 14 | *"The following requirements apply to…"* — cümle değil, giriş. |
| **gerçek kısıt** | **3** | **üçü de kapsanıyor** ↓ |
| **case-sensitivity** | **1** | **kapsanıyor** ↓ |

### Dört gerçek modalsiz hüküm — hepsi zaten karşılanıyor

| hüküm | karşılık |
|---|---|
| *"Primary key (none) means that the file allows only one row"* (`feed_info`) | `FIN_015` — birden fazla `feed_info` kaydı |
| *"Each (`service_id`, `date`) pair may only appear once"* | `DQ_021` — `calendar_dates.txt` PK'sı `service_id`+`date` |
| *"A `route_id` can only be defined in one `network_id`"* | `k6_analytics.rs:4006` — *"bir hat yalnız bir ağa ait olabilir"* |
| *"All file and field names are case-sensitive"* | `KNOWN_FILES.contains(&raw_name)` tam eşleşmedir → `Agency.txt` tanınmaz, `agency.txt` eksik sayılır |

### Sonuç: payda anlamlı ölçüde büyümüyor

104 saf adayın içinde **henüz kapsanmamış tek bir ölçülebilir hüküm bulunamadı.** Modalsiz
kör nokta gerçekti ama **boş çıktı** — çünkü spec, bağlayıcı bir kısıt koyarken neredeyse her
zaman bir modal kullanıyor; modalsiz cümleler semantik, tanım ve rehberlik taşıyor.

⚠️ **Yöntemin sınırı:** 104 aday kategori örnekleriyle değerlendirildi, tek tek adjudike
edilmedi; 764 sinyalsiz cümleye hiç bakılmadı. Bu bir **alt sınır ölçümü**dür — "yeni hüküm
yok" değil, "aramanın bu turunda çıkmadı" demektir.

## ⚠️ Bu sayı ne demek DEĞİL

**"Spec'in %54'ünü karşılıyoruz" DENEMEZ.** Payda bu kataloğun kendi kapsamıdır ve
üç sınırı vardır:

1. **Modal taşımayan hükümler görünmez.** *"All file and field names are case-sensitive"*
   bağlayıcıdır ama `must`/`shall` içermediği için katalogda yok. 273 bir **alt sınırdır**.
2. **Kaba cümle bölme.** Bir cümle birden çok hüküm taşıyabilir (`P44a6984b` örneği), ve
   bazı adayların bağlamı önceki cümlelerdedir.
3. **KAPSAM DIŞI %23'tür ve bu doğrudur** — spec tüketici davranışını da `must` ile yazar.
   Bir doğrulayıcının bunları ölçmemesi eksiklik değil, tanım gereğidir.

Dürüst cümle şudur: **spec'in alan tablosundan çıkan 302 hüküm atomunun 296'sı makine
kanıtlı; düzyazı ekseninde 273 adaydan feed'den doğrulanabilir olanların 148'i ölçülüyor,
9'u ölçülmüyor.**

Açık kalemler (kural yazımı bekleyen, hiçbiri yazılmadı): URL şeması (ölçüm bekliyor) ·
HTML etiketi · OpenGIS poligon · `pickup_type=2` booking rule · `platform_code` ·
`traversal_time` tavsiyesi · boarding area/platform pathway yasağı (ölçüm bekliyor).
