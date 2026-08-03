# Düzyazı hüküm triyajı (T5 — GTFS Spec rozeti)

Kaynak: `spec_provisions.json` (üreteç `extract_provisions.py`). O dosya **aday** üretir;
burası her adayın hüküm olup olmadığına ve karşılanıp karşılanmadığına karar verir.

Katalog toplamı **246 aday** (sert 142 · yumuşak 104). Bu belge **27'sini** adjudike eder;
kalanı sonraki turlarda. Sıra rastgele değil: `file-requirements` bölümünün tamamı düzyazıdır
ve hiçbir alan tablosuna yansımaz, yani ölçümün en kör noktasıydı.

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

## Sonraki tur

Kalan ~219 aday çoğunlukla `table_desc` kaynaklı (169 tane) — alan açıklamalarındaki koşullar.
Yoğunluk sırası: `stop_times.txt` 36 · `stops.txt` 20 · `routes.txt` 14 · `pathways.txt` 14 ·
`transfers.txt` 13 · `translations.txt` 13.

Üç açık kalem yukarıda: URL şeması (ölçüm bekliyor) · HTML etiketi (kural adayı) ·
OpenGIS poligon geçerliliği (geometri kütüphanesi kararı).
