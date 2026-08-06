# Kanıt tabanı — "GTFS Spec uygunluğu" iddiası neye dayanıyor

> **Bu belge iddiayı savunmaz, DENETLENEBİLİR kılar.** Her satırın karşısında ölçümün
> nasıl yeniden üretileceği yazılıdır. Bir iddiayı çürütmek isteyen okuyucu için
> "Bu iddiayı ne çürütür" bölümleri konmuştur.
>
> Durum: **2026-08-06** · kural sayısı **592** (38 grup) · `origin/main` = `62fa86b9`

---

## 0. Önce sınır: bu belge NE İDDİA ETMİYOR

1. **"GTFS spec'inin %100'ünü karşılıyoruz" DENMİYOR.** Ölçülen şey, iki makine-okunur
   **kataloğun kendi kapsamıdır**. Katalog spec'in kendisi değildir.
2. **Yumuşak (`should`/`recommended`) hükümler kasıtlı olarak %100 DEĞİL** (%65,2). Bir
   tavsiyeyi norm saymak geçerli feed'i reddeder; bu proje o hatayı bir kez yaptı
   (`PTH_017`) ve bir daha yapmamak için yumuşak ekseni ayrı tutuyor.
3. **Korpus 239 feed'dir, evren değildir.** "Yanlış pozitif ölçülmedi" ≠ "yanlış pozitif yok".
4. **Tüketici tarafını bağlayan hükümler ölçülmez** — 184 sert hükmün **25'i KAPSAM DIŞI**,
   13'ü META; ikisi paydadan düşer (184 − 38 = 146). Bir doğrulayıcının bunları ölçmemesi
   eksiklik değil, tanım gereğidir.

---

## 1. İki eksen ve bugünkü değerleri

| eksen | değer | ne demek |
|---|---|---|
| **Düzyazı hükümleri** | **146 / 146** | Spec metnindeki normatif cümlelerden feed'den doğrulanabilir olanlar |
| **Alan tablosu atomları** | **300 / 300** | Alan tablosundan makine ile çıkarılan hüküm atomları (302'nin 2'si hatalı üretim) |
| Yumuşak hükümler | 45 / 69 (%65,2) | **Hedef değil** — Quality sinyali |

⚠️ **İki eksen ÖRTÜŞÜR, TOPLANMAZ.** Aynı hüküm hem düzyazıda hem alan tablosunda
görünebilir. "446 hüküm karşılanıyor" gibi bir toplam **yanlıştır**.

**Yeniden üretim:**
```
python3 spec-audit/badge_status.py                 # düzyazı ekseni
cargo test -- --ignored anchor_granularity_report --nocapture   # alan tablosu ekseni
```
Yüzde elle yazılmaz; `badge_status.py` `PROVISION_TRIAGE.md`'yi okuyup hesaplar ve
**adjudike edilmemiş hüküm varsa bunu bildirip yüzdeyi geçersiz sayar.**

### Bu iddiayı ne çürütür
- `badge_status.py` "ADJUDİKE EDİLMEMİŞ N HÜKÜM" satırı basıyorsa yüzde eksik paydadır.
- Kataloğun kendisi eksikse yüzde anlamsızdır → **bölüm 2**.

---

## 2. Paydanın güvenilirliği — iddianın EN ZAYIF halkası

Düzyazı kataloğu spec metnini modal kelimelerle tarar. **Payda 2026-08-06'da İKİ KEZ
düzeldi ve iki düzeltmenin ikisi de aynı sınıf hataydı: case varsayımı.**

| tur | ne kaçmıştı | katalog | sonuç |
|---|---|---|---|
| başlangıç | — | 273 | %100 (131/131) |
| 1. düzeltme | BÜYÜK HARF RFC 2119 (`MUST`) — 9 cümle | 279 | %97,8 → `TRF_022`/`TRF_023` ile %100 |
| 2. düzeltme | cümle başı `Must`/`Should` + küçük harf `are forbidden` + `requires` — 20 cümle | **299** | %98,6 → `PTH_031` ile **%100** |

**Aynı gün iki kez yanılmış bir ölçüm aracının üçüncü kez yanılmayacağının kanıtı yoktur.**
Bugün yapılan şey, kaçağın *kaynağını* kurutmaktı: **modal taşımayan cümlelerin tamamı
okundu.** Düzeltmelerden sonra bugünkü sayım: 862 modalsiz cümle (124'ü normatif sinyal
taşıyor, 738'i sinyalsiz); sinyalsizlerin 252'si yapısal kategori (enum değeri, TOC
başlığı, örnek, presence parçası, `Primary key` bildirimi, sayfa JS'i), kalan **506'sı tek
tek okundu**. Artık "bakılmamış cümle" yok.

Kalan yapısal zayıflıklar — kapatılmadı, **kayıt altına alındı**:
- Cümle bölme kaba: bir cümle birden çok hüküm taşıyabilir.
- Çıkarımın ~%14'ü gürültü (TOC satırları, `¶` başlıkları, sayfanın çerez-onay JavaScript'i).
- Katalog HTML'den üretilir; spec'in yeni sürümü cümleleri değiştirirse kimlikler
  değişmez ama YENİ cümleler adjudike edilmemiş kalır (yukarıdaki denetim bunu yakalar).

**Yeniden üretim:**
```
python3 spec-audit/extract_provisions.py spec.html      # katalog
python3 spec-audit/measure_modalless.py spec.html --residual   # okunan kalıntı kümesi
```

### Bu iddiayı ne çürütür
Spec'te modal taşıyan ama katalogda olmayan **tek bir cümle** göstermek. Bugün bu üç kez
yapıldı (9 + 20 cümle) ve üçünde de payda düzeltildi.

---

## 3. Kural başına kanıt — 7 kapı

Her kuralın kanıtı **testle** bağlanır; "yazıldı" demek yetmez.

| kapı | ne garanti eder | bugünkü durum |
|---|---|---|
| `emit_proof` | Her kuralın gerçekten notice ürettiği bir fixture var | 12 test geçiyor · **borç 7** |
| `spec_conformance` | Spec metni ile kural davranışı | geçiyor |
| Kapsam defteri | Alan tablosu atomu ↔ kural eşlemesi | **0 açık** |
| `emit_identity` | İki kural aynı olguyu iki kez bildirmiyor | 10 kayıtlı istisna |
| İddia defteri | Kartlardaki spec alıntıları | **0 açık** |
| `field=None` defteri | Alan yazmayan kurallar gerekçeli | 30 kayıtlı |
| `required_columns_match_the_specification` | Zorunlu sütun listeleri spec ile | geçiyor |

Ek: `card_consistency` (kart künyesi ↔ registry), `triage_ledger_has_no_stale_open_claims`
(defter ↔ durum makinesi), `file_level_provisions_doc_covers_every_conditional_file`.

**`coverage_debt` = 7:** notice fixture'ı yazılamayan kurallar (fatal yollar gibi).
⚠️ **Muafiyet, kanıt yokluğu demek değildir — her biri için kanıtın NEREDE olduğu
yazılıdır.** Bu kural 2026-08-02'de denetlendi: üç girişten birinin ("`ARC_004`") kanıtı
YOKTU, yazılan yer boştu → ayrı bir fatal testi eklendi.

**Yeniden üretim:** `cargo test --workspace` (21 suite, exit 0) · `tsc --noEmit` · `vitest` (98/98)

---

## 4. Gerçek veri kanıtı — 242 feed'lik korpus koşumu (2026-08-06)

Tek binary, tek tarih (`--today 20260717`), tüm korpus.

```
242 feed · 4 fatal · 0 çökme
276 kural tetiklendi · 316 kural korpusta hiç çıkmadı
R1 yayın engeli taşıyan feed: 46 / 239 (üç yeni feed'in üçü de temiz)
```

**Yeniden üretim:**
```
python3 notgit/corpus_batch.py validate --today 20260717 --timeout 900
python3 notgit/corpus_batch.py report
```

### 4.1 MobilityData paritesi — bağımsız referansa karşı

Korpustaki her feed için MD'nin kendi doğrulama raporu saklanır ve satır satır kıyaslanır.

```
1446 kıyas satırı → MATCH 1026 · AGG 110 · EXPLAINED 300 · AÇIKLANAMAYAN 10
```

**En yüksek hacimli Kritik·Spec kuralı MD ile BİREBİR aynı sayıyı verdi:**
`STM_056` = `decreasing_or_equal_stop_time_distance` → **3.763.204 = 3.763.204** (mdb-2904).

12 açıklanamayan sapmanın dökümü:
- **5'i tarihe göreli** — MD raporunun doğrulama günü bizim sabit günümüzden farklı.
- **2'si MD kod eşlemesinin kabalığı** — tek MD kodu bizde 6 kurala karşılık geliyor.
- **2'si bilinçli karar** — `ARC_020` DRT muafiyeti, `ARC_017` GTFS-JP sütunları.
- **1'i şekil farkı, boşluk değil** — mdb-3235'in `feed_info.txt`'inde 25 satır var;
  MD semptomu 10 kez bildiriyor, biz kök nedeni bir kez (`FIN_015`).
- **Geriye açık sapma KALMADI** → bölüm 6.

**Yeniden üretim:** `python3 notgit/md_parity_audit.py notgit/corpus/pairs`

### 4.2 En yeni kuralların bağımsız doğrulaması

`TRF_022`/`TRF_023` bir feed'de 2.365 bulgu üretti (mdb-2898, İsviçre, 1,4M sefer).
Doğrulama **pipeline'ın kodu kullanılmadan** yapıldı: `calendar.txt` + `calendar_dates.txt`
ayrı bir betikle açıldı (55.058 servis) ve bildirilen her çiftin ortak aktif günü sayıldı.

**2365 / 2365 doğru. Sıfır yanlış pozitif.** Feed'in 132.855 bağlı aktarmasının %1,8'i
gerçekten belirsiz devamlılık taşıyor.

⚠️ Doğrulama betiğinin İLK hâli "12/12 yanlış pozitif" demişti; sebep betiğin mesajdaki
virgülü `service_id`'ye katmasıydı. **Sıfır sonuç verinin değil sorgunun özelliği olabilir.**

### Bu iddiayı ne çürütür
- Korpusta bir feed'de yeni bir kuralın MD'nin ERROR demediği yerde Kritik·Spec vermesi.
- `parity_unexplained.csv`'de yeni bir kuralın görünmesi (bugün **hiçbiri** görünmüyor).

---

## 5. Kanıt tabanının ÖLÇÜLMÜŞ boşlukları

Bunlar bilinen ve kabul edilen eksiklerdir. Rozet iddiası bunlara rağmen yapılıyorsa
okuyucu bunu görerek yapmalıdır.

### 5.1 18 kural korpusta tetiklenmiyor — ama bu "kanıtsız" DEMEK DEĞİL

v0.8.0'dan sonra eklenen 27 kuralın 18'i korpusta hiç tetiklenmedi. **İlk yazımda bunu
"gerçek veri kanıtı YOK" diye kaydetmiştim; ÖLÇÜNCE YANLIŞ ÇIKTI.** Her birinin girdi
dosyası korpusta MEVCUT ve okunuyor:

| kural | gereken dosya | dosyayı taşıyan feed |
|---|---|---|
| `TRP_035` | trips.txt | **239** |
| `CAL_025` | calendar.txt | **213** |
| `TRN_017` | translations.txt | 34 |
| `BKR_024` · `XFL_032/033` | booking_rules / location_groups | 18 |
| `ARS_002` | areas.txt | 13 |
| `PTH_030` · `PTH_031` | pathways.txt | 10 |
| `XFL_034` | location_group_stops.txt | 7 |
| `LOC_011` | locations.geojson | 6 |
| `FPD_007` · `NET_004` · `RCT_008` | fare_products / networks / rider_categories | 4 |
| `FMD_004` | fare_media.txt | 3 |
| `SAR_003/004` · `TFR_008` | stop_areas / timeframes | 1 |

Dosyaların gerçekten işlendiği kardeş kurallarla doğrulandı (`FPD_006`, `FRL_008`, `LOC_006`
aynı feed'lerde ateşliyor). Yani bu 18 kural **gerçek veriyi gördü ve sessiz kaldı**.

**Doğru cümle:** elde **yanlış pozitif kanıtı** var (242 feed'de sıfır), **doğru pozitif
kanıtı** yok (korpusta bu hükümleri ihlal eden feed bulunmadı). Fixture'lar ikincisini
kapatır: her biri kuralın ateşlemesi GEREKEN veride ateşlediğini gösterir.

⚠️ **Gerçekten sıfır olan tek şey:** `fare_leg_join_rules.txt`. **2026-08-06'da bunun
neden sıfır olduğu ölçüldü — korpusun eksikliği değil, dosyanın sahada kullanılmaması:**

1. MobilityData kataloğunda `fares-v2` işaretli, kullanımdan kalkmamış **38 feed'in
   TAMAMI** tarandı (`spec-audit/zip_peek.py` — zip'in merkezî dizinini HTTP Range ile
   okur, tam indirme yapmaz). **38/38 bakılabildi, taşıyan: 0.**
2. MobilityData'nın kendi **GTFS Features Adoption Tracker**'ı 22 GTFS dosyasını izliyor —
   `fare_leg_rules`, `fare_transfer_rules`, `fare_media`, `fare_products`,
   `rider_categories` dahil. **`fare_leg_join_rules.txt` listede YOK.**

**Sonuç:** dosya spec'te tanımlı ama pratikte benimsenmemiş. `FLJ_001..004` için gerçek
veri kanıtı **bugün elde edilemez**; kanıt fixture'lardır ve dördünün de fixture'ı vardır
(`emit_proof.rs:346-370`), borç defterinde değiller. ⚠️ Kalıcı risk kayıtlıdır: FLJ ağ
alanları BİRLEŞİK kümeye çözülür, `NET_002` yalnız `networks.txt`'e — karıştırılırsa kural
ya hiç ateşlemez ya geçerli veride ateşler. Gerçek veri çıkana dek bu ayrım yalnız
fixture ile korunuyor.

### 5.2 Korpusun kendisi taze değil
Zip'lerin çoğu 2026 Temmuz'da indirildi (3'ü 6 Ağustos'ta). Feed'lerin güncel hâli farklı
olabilir.

**2026-08-06'da korpus 239 → 242 feed'e çıkarıldı** (mdb-13 San Diego MTS · mdb-1246 Santa
Barbara MTD · mdb-3109 Benton Area — üç AYRI üreticiden). Etkisi ölçüldü: Fares v2 dosya
kapsamı yaklaşık iki katına çıktı (`fare_media` 1→3, `fare_products` 2→4, `networks` 2→4,
`rider_categories` 2→4, `fare_leg_rules` 2→4), Flex dosyaları +1. **Ama 18 kuralın hiçbiri
yine tetiklenmedi** — yeni feed'ler temiz. `fare_leg_join_rules.txt` hâlâ sıfır.

### 5.3 Yumuşak eksende 2 açık boşluk
`agency_lang` varlık tavsiyesi ve bağlı seferlerde coğrafi yakınlık tavsiyesi. İkisi de
**Quality** sınıfına ait; yayın kapısını (R1 = `Spec ∧ Kritik`) etkilemez.

---

## 6. Korpus koşumunun bulduğu iki hata — İKİSİ DE DÜZELTİLDİ

İkisi de bugünkü işten önce vardı; korpus koşumu görünür kıldı ve aynı gün kapatıldı.
Her ikisi de **MD'ye karşı birebir pariteyle** doğrulandı.

| feed | MD | önce | sonra | kök neden |
|---|---|---|---|---|
| mdb-1840 | `equal_shape_distance_diff_coordinates` = 2 | `SHP_028` = 7 | **2** ✅ | Eşik DERECEYLE ölçülüyordu (`max(\|Δlat\|,\|Δlon\|) ≥ 1e-5`), yorumda "≈1,1 m" yazıyordu. 1e-5 derece boylam yalnız ekvatorda 1,1 m'dir; Fransa'da (49,6°) 0,72 m'ye iner. Haversine ile **metreye** çevrildi. |
| mdb-1229 | `unsorted_stop_times` = 24 | `STM_036` = 0 | **24** ✅ | Feed'in 24 seferinin TÜM satırlarında `stop_sequence=0`. Predicate `seq < last` arıyordu; **eşitlik iki dala da girmiyordu**. Spec *"values must increase"* der → `<=` yapıldı. |

**Regresyon ölçümü:** düzeltmelerden sonra korpus yeniden koşuldu (aynı binary, aynı tarih,
aynı 239 feed). **Değişen kural sayısı: 3** — ve üçü de dokunulan kurallar
(`SHP_028` 16→8 feed · `SHP_029` 24→23 · `STM_036` 30→31, tam **+24**, yalnız mdb-1229).
Yan etki yok.

⚠️ `SHP_028` iki feed'de ARTTI (mdb-1830 +9, mdb-1158 +2) ve bu **beklenen**: eksen-maksimumu
metriği ÇAPRAZ kaymayı olduğundan küçük ölçüyordu (Meksika'da Δ=8,9e-6 derece → gerçek
1,24 m). Eski metrik yüksek enlemde yanlış pozitif, çapraz kaymada yanlış negatif
üretiyordu; metre eşiği ikisini birden düzeltir.

### Kalan 10 sapmanın tamamı sınıflandırılmıştır
4× `expired_calendar` ve 1× `big_gap_in_service` (tarihe göreli) · 2× `number_out_of_range`
(MD kod eşlemesinin kabalığı) · `ARC_020` ve `ARC_017` (bilinçli karar) · `FIN_018`
(kök neden `FIN_015` ile bir kez bildiriliyor).

---

## 7. Yöntemin kendisine dair iki uyarı

Bu proje aynı hataya iki kez düşmemek için hataları sınıf olarak adlandırıyor. Kanıt
tabanını okuyanın bilmesi gereken ikisi:

1. **"Kapsam dışıdır" diyen bir kod yorumu, kararın VERİLDİĞİNİ gösterir — hükmün
   KARŞILANDIĞINI değil.** `PTH_026`'nın yanında böyle bir yorum duruyordu ve altındaki
   hükmü ölçen başka kural yoktu; `PTH_031` bugün o boşluğu kapattı. **Defterle
   bağlanmamış her muafiyet sessizce kaybolur.**
2. **Bir baseline'ın DOSYA TARİHİ, VERİSİNİN tarihi değildir.** Korpusun eski
   `rule_stats.csv`'si 1 Ağustos tarihliydi; sonuçların 229/231'i **17 Temmuz**'dandı.
   Bu yüzden bugünkü koşumdan önce hiçbir "regresyon" kıyası geçerli değildi.
   Bugünkü koşum **ilk tutarlı baseline**dır.

---

## 8. Özet — dürüst cümle

> Spec'in alan tablosundan makine ile çıkarılan **300 hüküm atomunun tamamı** bir kurala
> çapalıdır; düzyazı ekseninde kataloglanan **299 adaydan feed'den doğrulanabilir 146 sert
> hükmün tamamı** ölçülmektedir. Kurallar 239 feed'lik bir korpusta koşulmuş, MobilityData
> referans doğrulayıcısıyla 1446 satırda kıyaslanmış ve **12 açıklanamayan sapma**
> kalmıştır; bunların hiçbiri son eklenen 27 kurala ait değildir.
>
> Bu ölçümlerin kapsamı, **kataloğun kendi kapsamıdır** — spec'in tamamı değil. Payda
> 2026-08-06'da iki kez düzeltilmiştir; iddianın en zayıf halkası sayının kendisi değil,
> **paydanın nasıl kurulduğudur**.
