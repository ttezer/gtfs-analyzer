# Kanıt tabanı — "GTFS Spec uygunluğu" iddiası neye dayanıyor

> **Bu belge iddiayı savunmaz, DENETLENEBİLİR kılar.** Her satırın karşısında ölçümün
> nasıl yeniden üretileceği yazılıdır. Bir iddiayı çürütmek isteyen okuyucu için
> "Bu iddiayı ne çürütür" bölümleri konmuştur.
>
> Durum: **2026-08-07** · kural sayısı **593** (38 grup)
>
> ⚠️ **Buradaki hiçbir sayı elle güncellenmez ve belge bir commit'e SABİTLENMEZ.** Önceki
> sürüm `origin/main = 62fa86b9` yazıyordu ve dört commit sonra hâlâ öyle duruyordu — dış
> denetim bunu "mevcut durum kanıtı gibi okunuyor ama eski bir durumu gösteriyor" diye
> yakaladı. Güncel rakam için **`python3 spec-audit/badge_status.py`** çalıştırın; CI'da da
> kapı olarak koşuyor.

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
| **Düzyazı hükümleri** | **147 / 147** | Spec metnindeki normatif cümlelerden feed'den doğrulanabilir olanlar |
| **Alan tablosu atomları** | **300 / 300 ÇAPALI** | Her geçerli atomda EN AZ BİR Spec notice çapası var — **semantik tamlık DEĞİL** (§5.4) |
| Yumuşak hükümler | 45 / 69 (%65,2) | **Hedef değil** — Quality sinyali |

🔴 **Düzyazı ekseni 2026-08-06'da BAĞIMSIZ DENETİMDE %100'den düştü.** Denetim iki hata
buldu ve ikisi de kabul edildi:
- `Pa1fdaa0d` (geojson/pencere/pickup eşzamanlı örtüşmesi) **KAPSAM DIŞI değil BOŞLUK'tu.**
  Gerekçemiz *"poligon geometrisi saklanmıyor"*du — bu bir **mimari eksik**, doğrulanamazlık
  değil. Payda 146 → **147**. ✅ **2026-08-07'de KAPATILDI (`STM_060`):** geometri artık
  K1 → K4 taşınıyor. ⚠️ İlk uygulama gerçek veride **21 yanlış pozitif** üretti (aynı
  `location_id`'li iki kaydı ihlal sayıyordu, oysa spec bölge-içi seyahat için onu ZORUNLU
  kılıyor ve MD aynı kuralı uyguladığı hâlde susuyor) → aynı-bölge çifti muaf, korpusta 0.
- `Pd84a0bcb` (OpenGIS 6.1.11 geçerliliği) **KANITLI değil KISMİ'dir.** `LOC_011` hükmün bir
  kısmını ölçüyor; kuralın KENDİ KARTI dört maddeyi yanlış-negatif diye sayıyor (delik-delik
  kesişimi · deliğin kısmen dışarıda olması · ring yönü · iç kısmın bağlantılılığı).
  Pay 146 → **145**. ✅ **2026-08-07'de KAPATILDI (issue #74) → KANITLI, pay 147.** Üç madde
  08-06/07'de, madde 5 (iç kısmın bağlantılılığı) 08-07'de kapandı: ölçüt "iki ring iki
  noktada dokunuyor" özel hâlinden **ring temas grafiğinde çevrim**e çevrildi, ring'in
  kendine teması öz-döngü olarak eklendi. ⚠️ **Ring yönü listeden ÇIKARILDI, ölçülmüyor:**
  6.1.11 yönelim şart koşmaz (o kural RFC 7946 §3.1.6'nın serileştirme kuralıdır), ölçmek
  `PTH_017` hatasını tekrarlardı. ⚠️ İlk uygulama gerçek veride **3 feed'de yanlış pozitif**
  üretti (aynı koordinat ardışık ÜÇ kez yazılıyordu) → ring ardışık yinelemelerden
  arındırılıyor; `locations.geojson` taşıyan 6 feed'in tamamında bulgu farkı **0**.

⚠️ **İki eksen ÖRTÜŞÜR, TOPLANMAZ.** Aynı hüküm hem düzyazıda hem alan tablosunda
görünebilir. "446 hüküm karşılanıyor" gibi bir toplam **yanlıştır**.

**Yeniden üretim:**
```
python3 spec-audit/badge_status.py                 # düzyazı ekseni
cargo test -- --ignored anchor_granularity_report --nocapture   # alan tablosu ekseni
```
Yüzde elle yazılmaz; `badge_status.py` `PROVISION_TRIAGE.md`'yi okuyup hesaplar.
Adjudike edilmemiş hüküm varsa **çıkış kodu 1** döner — CI'da kapı olarak kullanılabilir.
⚠️ 2026-08-06 denetimine kadar bu yalnız EKRANA BASILAN BİR UYARIYDI ve betik `return 0`
ile başarıyla çıkıyordu; yani "geçersiz sayar" ifadesi insan yorumuna bağlıydı, kapı değildi.

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
✅ **Kaynak SABİTLENMİŞTİR** (2026-08-06 denetiminden sonra): `spec_provisions.json`
içindeki `_source` alanı indirilen HTML'in SHA-256'sını, bayt boyutunu ve spec'in kendi
**"Revised April 27, 2026"** satırını taşır.

🔴 **AMA O HASH ÇAPA DEĞİLDİ — 2026-08-07'de ÇÜRÜTÜLDÜ.** "Hash tutmuyorsa katalog başka
bir metinden üretilmiştir" yazıyordu; ölçüldü ki hash **HİÇBİR ZAMAN tutmuyor**: sayfa arka
arkaya üç kez indirildi, üç farklı özet çıktı. Fark yalnız Cloudflare'in enjekte ettiği iki
satırdı (`data-cfemail` token'ı + `__CF$cv$params` ray id'si). Yani üçüncü bir taraf o
değeri asla yeniden üretemezdi.

✅ **Çapa artık `_source.provisions_sha256`:** adayların kanonik JSON'undan hesaplanır, site
iskeletinden bağımsızdır. İki AYRI indirmede AYNI çıktığı ölçüldü (sayfa özeti farklıyken).
Tutmuyorsa katalog başka bir metinden üretilmiştir ve yüzde o metne ait değildir.
`sha256`/`bytes` alanları kayıtta kalır ama iddiası **"bu baytları gördük"**tür,
doğrulanabilir bir çapa değil.
✅ **UPSTREAM SÜRÜM DE SABİT** (2026-08-07, issue #72): `_source.upstream_commit` +
`upstream_commit_date`, gtfs.org'un render ettiği kaynağa — `google/transit` deposundaki
`gtfs/spec/en/reference.md` — dokunan en yeni commit'i taşır. Bugünkü kayıt
`ac663b57` (2026-04-30).

🔴 **İKİ ALAN AYNI ŞEYİ SABİTLEMEZ, biri diğerinin yerine geçmez:**
| alan | neyi sabitler | sınırı |
|---|---|---|
| `sha256` + `bytes` | **AYRIŞTIRDIĞIMIZ baytlar** (render edilmiş HTML) | site iskeleti değişince de değişir; normatif metin aynı kalabilir |
| `upstream_commit` | o metnin **KAYNAK sürümü** (markdown) | katalog markdown'dan DEĞİL HTML'den üretilir → commit "hangi spec sürümü"nü söyler, "hangi baytlar"ı değil |

⚠️ Commit çözülemezse `extract_provisions.py` **exit 1** verir; sessizce boş alan yazmaz.
Ağsız üretim için `--no-upstream` vardır ve kayıt bunu AÇIKÇA yazar ("çözülemedi" ile
"hiç denenmedi" ayrımı — `AGN_001` dersi).

### 2.1 Sürüklenme (drift) tespiti — haftalık, issue AÇAR, build KIRMAZ

Katalog canlı bir sayfadan üretiliyor; spec revize edilirse yüzde sessizce eski metne ait
olur. `.github/workflows/spec-drift.yml` haftada bir `spec-audit/spec_drift.py` koşar
(issue #73).

🔴 **HAM SHA-256 KAPISI TEK BAŞINA YANLIŞ ALARM ÜRETİR — ÖLÇÜLDÜ.** İlk tasarım "sayfanın
sha'sı değiştiyse issue aç" idi. 2026-08-07'de sayfanın sha'sı ZATEN farklıydı
(295.559 → 295.827 bayt) ama **katalog birebir aynıydı**: 299 adayın 299'u, aynı kimlikler,
aynı `spec_revision`. Fark mkdocs yeniden derlemesiydi. Kapı ilk koşusunda yanlış alarm
verecekti — yani insanları görmezden gelmeye alıştıracaktı.

Bu yüzden dört sınıf AYRI raporlanır ve yalnız ikisi issue açar:

| durum | anlamı | issue? |
|---|---|---|
| `same` | sayfa da katalog da aynı | hayır |
| `cosmetic` | sayfa değişti, **katalog aynı** | hayır (özete yazılır) |
| `catalogue` | aday eklendi/kayboldu ya da `spec_revision` değişti | **evet** |
| `upstream` | `reference.md`'ye yeni commit girdi (sayfa henüz derlenmemiş olabilir) | **evet** |

⚠️ **Sürüklenme build'i KIRMAZ** (spec revizyonu bu depoda regresyon değil, haberdir) ama
**HİÇBİR SİNYAL ÖLÇÜLEMEZSE KIRAR**: betik **exit 2** döner. Sessizce yeşil kalan bir
denetim denetim değildir — `zip_peek.py` ile aynı ders: *"yok"* ile *"bakılamadı"*
karıştırılmaz.

🔴 **CI SAYFAYA ULAŞAMIYOR — ilk koşumda ölçüldü: HTTP 403.** gtfs.org Cloudflare arkasında
ve barındırılan koşucuları engelliyor (tarayıcı User-Agent'ı da yetmedi). Bu yüzden iş
`--mode upstream` koşar: **`reference.md`'ye dokunan commit'i** izler, ki bu zaten sayfanın
render edildiği KAYNAKTIR. Katalog düzeyindeki karşılaştırma (aday farkı) geliştirici
makinesinde koşar. Mod açıkça verilir; `auto`'nun sessizce düşmesine bırakılmaz — düştüğü
zaman bile rapor ve issue gövdesi hangi sinyalin ölçülMEDİĞİNİ yazar.

Sınıfların hepsi defter bozularak doğrulandı (`--baseline` bayrağı bunun içindir); ağsız
düşüş yolu da ayrıca denendi.

### Bu iddiayı ne çürütür
Spec'te modal taşıyan ama katalogda olmayan **tek bir cümle** göstermek. Bugün bu üç kez
yapıldı (9 + 20 cümle) ve üçünde de payda düzeltildi.

---

## 3. Kural başına kanıt — 7 kapı

Her kuralın kanıtı **testle** bağlanır; "yazıldı" demek yetmez.

| kapı | ne garanti eder | bugünkü durum |
|---|---|---|
| `emit_proof` | Her kuralın notice ürettiği bir fixture var | 13 test · ⚠️ **6 kuralın fixture'ı YOK** (§5.3) |
| `badge_status` | Adjudike edilmemiş hüküm varsa **exit 1** | ✅ CI'da kapı (2026-08-06'da eklendi) |
| `ledger_header_counts_match_catalogue` | Defter başlığı katalogla tutuyor + paya sayılan satırda varsayım yok | ✅ (2026-08-06'da eklendi) |
| `spec_conformance` | Spec metni ile kural davranışı | geçiyor |
| Kapsam defteri | Alan tablosu atomu ↔ kural eşlemesi | **0 açık** |
| `emit_identity` | İki kural aynı olguyu iki kez bildirmiyor | 10 kayıtlı istisna |
| İddia defteri | Kartlardaki spec alıntıları | **0 açık** |
| `field=None` defteri | Alan yazmayan kurallar gerekçeli | 30 kayıtlı |
| `required_columns_match_the_specification` | Zorunlu sütun listeleri spec ile | geçiyor |

Ek: `card_consistency` (kart künyesi ↔ registry), `triage_ledger_has_no_stale_open_claims`
(defter ↔ durum makinesi), `file_level_provisions_doc_covers_every_conditional_file`.

**`coverage_debt` = 0** (2026-08-07, issue #71 — defter BOŞ): fixture'ı olmayan canonical
kural kalmadı. ⚠️ **Muafiyet, kanıt yokluğu demek değildir — her biri için kanıtın NEREDE
olduğu yazılıdır.** Bu kural 2026-08-02'de denetlendi: üç girişten birinin ("`ARC_004`")
kanıtı YOKTU, yazılan yer boştu → ayrı bir fatal testi eklendi.
⚠️ **Defterin boş olması "her kural her yolda kanıtlı" demek DEĞİLDİR** — fixture kuralın
BİR emit noktasını kanıtlar. `ARC_022`'nin üç emit noktası ayrıca `integration.rs::arc022_*`
ile ölçüldü. `PROOF_ALLOWLIST`'teki üç fatal-yol kuralı (`ARC_001`/`ARC_004`/`ARC_029`)
borç değildir, kanıtları integration testlerindedir.

**Yeniden üretim:** `cargo test --workspace` (21 suite, exit 0) · `tsc --noEmit` · `vitest` (98/98)

---

## 4. Gerçek veri kanıtı — 242 feed'lik korpus koşumu (2026-08-06)

Tek binary, tek tarih (`--today 20260717`), tüm korpus.

```
242 feed · 4 fatal · 0 çökme
276 kural tetiklendi · 317 kural korpusta hiç çıkmadı
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

### 5.3 Fixture varlığı = emisyon kanıtı, SEMANTİK TAMLIK kanıtı değil
`emit_proof` her kural için "beklenen `rule_id` üretilen notice kümesinde var mı" sorusunu
yanıtlar. **Tek başına şunları doğrulamaz:** kuralın bütün mantıksal dalları · sınır
değerleri · yanlış pozitif üretmemesi · hükmün tamamının mı yoksa bir bölümünün mü
ölçüldüğü. (`Pd84a0bcb` tam bu boşluktan geçmişti: `LOC_011`'in fixture'ı yeşildi ama
hükmün dörtte biri ölçülmüyordu.) Sınır değerleri ve yanlış-pozitif tarafı ayrı
`integration.rs` testleriyle ve korpus koşumuyla kapatılır — kural bazında değil.

✅ **Fixture borcu 7 → 1** (issue #71, 2026-08-07). Denetim "hâlâ 7" dediğinde listenin
GEREKÇELERİ incelendi ve çoğu yanlış çıktı:

| kural | eski gerekçe | gerçekte |
|---|---|---|
| `OPR_024` | inline yazılamaz | eşik zaten `ValidatorConfig`'te → `fx_cfg` |
| `VAT_006` · `STM_043` | inline yazılamaz | 50 / 201 satır yetiyor → `fx` |
| `SHP_026` · `STM_044` | inline yazılamaz | eşik SABİT KODLUYDU → config'e taşındı, `fx_cfg` |
| `ARC_027` | inline yazılamaz | zip ÜST VERİSİ gerekiyordu → harness'a `unix_permissions` eklendi |

⚠️ Eşik taşımaları **varsayılan davranışı değiştirmedi**; tek amaç kanıt yazılabilmesiydi.

✅ **`ARC_022` de 2026-08-07'de kapandı → borç 0.** Eşiği `k1_parse.rs`'te sabit kodluydu ve
K1'in `parse()`'ı `ValidatorConfig` almıyordu; imza `parse(&[u8], &ValidatorConfig)` oldu ve
aynı alan K2'nin iki akış yoluna (`stop_times`, `shapes`) da bağlandı — knob'ı yalnız K1'e
bağlamak, 1M satırı gerçekte aşan iki dosyada onu sessizce etkisiz bırakırdı.

🔴 **YEDİ KAYDIN YEDİSİ DE "yazılamaz" ETİKETİYLE DURUYORDU VE YEDİSİ DE DENENİNCE DÜŞTÜ.**
Defterdeki gerekçe bir ölçüm değil tahmindi. Bu dosyaya yeni satır eklenecekse gerekçe
DENENMİŞ olmalı.

✅ **Bu turda açılan boşluk AYNI GÜN kapandı (issue #75):** `trips.txt` ve
`calendar_dates.txt` K1'de stream ediliyordu ve K2'de ARC_022 emit noktaları yoktu → kaç
satır olursa olsun kural susuyordu. Denetim `k2/mod.rs`'te tek geçişe toplandı.
🔴 **Boşluk teorik değildi:** İsviçre feed'inde (`mdb-2898`) 8.442.424 satırlık
`calendar_dates.txt` ve 1.521.317 satırlık `trips.txt` sessizce geçiyordu. Korpus ölçümü
(12 aday feed, zip merkezî dizini taramasıyla seçildi): **4 yeni bulgu, kayıp yok**;
27 feed'e ulaşılamadığı için bu bir ALT SINIRDIR.

### 5.4 "300/300" ÇAPA kapsamıdır, semantik tamlık değil
Alan tablosu ekseni `Presence`/`Type`/`Primary Key` sütunlarından **kaba atomlar** üretir.
Kapsamadıkları: düzyazı koşulları · dosyalar arası koşullar · sefer içi tutarlılık ·
GeoJSON iç yapı koşullarının tamamı. Bir alana Spec notice çapalanması, o alanın ilgili
hükmünün gerçekten karşılandığını **göstermez** (çapa ALAN düzeyindedir, ATOM düzeyinde
değil — 2026-08-05 triyajı bu yüzden 12 boşluk buldu).

**Söylenebilir:** *"Üretilmiş 300 geçerli alan atomunun tamamında en az bir Spec notice
çapası var."*  **Söylenemez:** *"Alan tablosundaki 300 hükmün her biri semantik olarak
eksiksiz uygulanıyor."*

### 5.5 Korpus kanıtı — ✅ ARTIK DIŞARIDAN DOĞRULANABİLİR (2026-08-07)
Dış denetim haklıydı: komutlar `notgit/` altındaki dosyalara işaret ediyordu ve o klasör
`.gitignore`'da; korpus rakamları **geliştirici beyanıydı**. Kapatıldı.

Depoya ALINAN (`spec-audit/`):
| dosya | ne |
|---|---|
| `corpus-evidence/corpus_manifest.csv` | 242 feed · her zip'in **SHA-256**'sı · bayt boyutu · indirme URL'i |
| `corpus-evidence/rule_stats.csv` | koşumun kural bazında çıktısı |
| `corpus-evidence/parity_unexplained.csv` | MD paritesinde açıklanamayan 10 satır |
| `corpus-evidence/fatals.csv` | 4 fatal feed |
| `corpus-evidence/verify_corpus.py` | üçüncü tarafın hash doğrulaması + indirme |
| `corpus_batch.py` · `md_parity_audit.py` | koşum ve parite betikleri (gizli bilgi taşımaz; MD token env/gitignore'dan okunur) |

Depoya ALINMAYAN: zip'lerin kendisi (**2,33 GB**).

**Üçüncü taraf zinciri:**
```
python3 spec-audit/corpus-evidence/verify_corpus.py --download <dizin>
python3 spec-audit/corpus-evidence/verify_corpus.py --zips <dizin>     # SHA-256 karşılaştır
python3 spec-audit/corpus_batch.py validate --dir <dizin> --today 20260717
python3 spec-audit/corpus_batch.py report --dir <dizin>
# üretilen rule_stats.csv ↔ corpus-evidence/rule_stats.csv
```
Bugün üreticide doğrulandı: **242/242 birebir aynı** ve **242/242'sinin indirme URL'i var.**

⚠️ İlk sürümde İKİ feed'in (`PID_GTFS`, `ch-odv-flex`) `download_url` alanı BOŞTU ve
`verify_corpus.py` onları sessizce atlıyordu — yani üçüncü taraf tam korpusu kuramıyordu
ama bunu fark etmiyordu. 2026-08-07 denetimi yakaladı. Artık: manifest `build_manifest.py`
çıktısıdır (katalog → `plan.csv` → gerekçesi yazılı `EXTRA_URLS` sırasıyla), URL'siz feed
kalırsa üretici **exit 1**, `--download` da **exit 1** döner. Sessiz atlama kaldırıldı.

⚠️ **Kalan sınır:** feed'ler CANLI URL'lerden gelir. Yayıncı dosyayı güncellerse hash
tutmaz — bu bir başarısızlık değil, korpusun bir ANLIK GÖRÜNTÜ olduğunun kanıtıdır.
`verify_corpus.py` "değişmiş" ile "bozuk"u ayrı raporlar; yalnız hash'i tutan feed'ler
için üretilen sayılar bizimkiyle karşılaştırılabilir.

### 5.6 Yanlış pozitif kapısı SEKİZ sentetik feed'e dayanıyor
`spec_conformance.rs`'teki "geçerli feed" matrisi sekiz senaryodur (klasik `stop_id` ·
Flex `location_id` · Flex `location_group_id` · duraksız Flex · negatif ücret indirimi ·
`routes.network_id` · yalnız `calendar_dates` · gece yarısı sonrası raylı sefer) ve bu
feed'lerde Spec/Kritik notice çıkmamasını doğrular. **İyi bir regresyon kapısıdır ama
593 kural için genel yanlış-pozitif kanıtı değildir.**

### 5.7 Kuralların ~%53'ü korpusta hiç tetiklenmedi
276 kural tetiklendi, **317 kural hiç çıkmadı** (593'ün %53'ü). Yani kuralların yarısından
fazlası için
GERÇEK VERİ üzerinde doğru-pozitif gözlem yok; kanıtları sentetik fixture'dır.

### 5.8 Mevcut kapılar İKİ GERÇEK SEMANTİK HATAYI kaçırdı
2026-08-06'da bulunan `SHP_028` (eşik derece olarak uygulanmıştı, metre olmalıydı) ve
`STM_036` (`<` kullanıldığı için eşit `stop_sequence` kaçıyordu) hataları **fixture ağıyla
değil, korpus koşumu ve MD paritesiyle** ortaya çıktı. Bu, fixture'ın şu soruyu
yanıtladığını gösterir: *"kural herhangi bir veride ateşleyebiliyor mu?"* — ama şunu
GARANTİ ETMEZ: *"kural tüm sınır durumlarında doğru predikatı mı uyguluyor?"*

### 5.9 Yumuşak eksende 2 açık boşluk
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

> Spec'in alan tablosundan makine ile çıkarılan **300 geçerli atomun tamamında en az bir
> Spec notice çapası** vardır (semantik tamlık DEĞİL); düzyazı ekseninde kataloglanan 299
> adaydan feed'den doğrulanabilir **147 sert hükmün 145'i** ölçülmektedir (**%98,6**).
> Kurallar 242 feed'lik bir korpusta koşulmuş, MobilityData referans doğrulayıcısıyla 1446
> satırda kıyaslanmış ve **10 açıklanamayan sapma** kalmıştır.
>
> **Düzyazı ekseninde %100 iddiası GEÇERSİZDİR.** 2026-08-06 bağımsız denetimi bir hükmün
> yanlışlıkla kapsam dışı bırakıldığını (`Pa1fdaa0d` — gerekçe mimari eksikti, hüküm feed'den
> doğrulanabilir) ve bir hükmün kısmi uygulamayla kapatılmış sayıldığını (`Pd84a0bcb` —
> `LOC_011` OpenGIS 6.1.11'in dört maddesini ölçmüyor) tespit etti; ikisi de kabul edildi.
>
> Payda aynı gün **üç kez** düzeldi (iki case varsayımı + bir yanlış kapsam-dışı kararı).
> İddianın en zayıf halkası sayının kendisi değil, **paydanın nasıl kurulduğudur** — ve bu
> zayıflık ölçülmüş, gizlenmemiştir.
