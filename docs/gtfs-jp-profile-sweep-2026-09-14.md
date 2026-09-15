# GTFS-JP profil taraması — Japon feed'leri × auto/v3/v4 (2026-09-14)

Bu kayıt iki aşamayı birlikte tutar. İlk dört bölüm, 14 Eylül'deki ilk profil
tarama binary'sinde görülen sorunları ve ölçümleri tarihsel olarak korur;
`Uygulama sonucu` ve sonraki bölümler bu sorunların hangi düzeltmelerle
kapatıldığını ve hangi platform sorusunun açık kaldığını gösterir. Güncel
full-catalog audit'i `--profiles` seçimini destekler; varsayılan `auto` parity
koşusudur, açık `v3,v4` geçişleri ise ayrıca yayımlanan profil çıktılarıdır.

## Yöntem

- **635 Japon feed'i.** 18. koşumdan (`34718902532`) ülkesi JP olan veya `JPN_*` bulgusu üreten
  her feed; iki küme de aynı 635'e çıktı.
- Her feed **üç kez** koşuldu: `--gtfs-jp-profile auto|v3|v4`, hepsinde `--today 20260820`.
- Binary `cd5fda99`, release, **macOS arm64**. Korpus koşumu GitHub'ın **x86_64 Linux**
  makinesinde koşar — 3. maddedeki açık soru buradan doğuyor.
- Her arşivin sha256'sı hesaplandı ve 18. koşumun kaydıyla karşılaştırıldı: **622 aynı, 13 değişmiş.**
- **MobilityData bu taramada YOK.** MD'nin profil kavramı yoktur ve bu makinede Java kurulu değil.
  Soru "biz üç profilde ne yapıyoruz", "MD ne diyor" değil.
- Hata: **0**.

## Tarihsel ilk tarama — doğrulanan üç şey

| ölçüm | auto | v3 | v4 |
|---|---:|---:|---:|
| toplam bulgu | 236.441 | 5.183.145 | 2.802.386 |
| feed başına medyan | 127 | 1.191 | 639 |
| ortalama genel skor | 93,41 | 92,05 | 93,10 |
| `publishable` feed | **600** | **600** | **600** |

1. **Profil seçimi yayın kararını DEĞİŞTİRMİYOR.** Üç profilde de 600 feed yayınlanabilir.
   Skor düşüşü V3'te ortalama 1,36 puan, en çok 4,5 (`mdb-1303`).
2. **Profil seçimi genel GTFS doğrulamasına DOKUNMUYOR.** JPN dışı hiçbir kural profile göre
   oynamıyor — karşılaştırma tablosunda o bölüm tamamen boş.
3. **Profil metriği plana uygun.** V3/V4'te 635 feed'in 635'i seçilen profili raporluyor; auto'da
   630 feed `auto`, JP sinyali olmayan 5 feed boş. Tespit bayrağı üç profilde de sabit (630/5),
   yani seçim ile tespit birbirinden bağımsız.

Ayrıca sürüm kapıları doğru: `JPN_002`/`003`/`013`/`016`/`018` V4'te sıfırlanıyor, `JPN_022`
yalnız V4'te konuşuyor (8 feed).

---

# Tarihsel takip bulguları ve durumları

## 1. ✅ `stop_times.stop_headsign` satır başına basılıyor — hacmin %99,9'u (uygulandı)

En hacimli 20 feed'in `JPN_028`/`JPN_029`/`JPN_030` bulgularının **%99,9'u** tek alandan:

| kural | dosya · alan | bulgu | pay |
|---|---|---:|---:|
| `JPN_028` | `stop_times.stop_headsign` | 1.582.080 | %34,8 |
| `JPN_029` | `stop_times.stop_headsign` | 1.582.080 | %34,8 |
| `JPN_030` | `stop_times.stop_headsign` | 1.377.856 | %30,3 |
| `JPN_028`/`JPN_030` | `stops.stop_desc` | 709 + 709 | %0,0 |
| `JPN_028`/`JPN_030` | `routes.route_desc` | 290 + 290 | %0,0 |
| `JPN_028`/`JPN_030` | `trips.jp_trip_desc` | 257 + 257 | %0,0 |
| `JPN_028`/`JPN_030` | `feed_info.feed_publisher_name` | 17 + 17 | %0,0 |

Yani kapsamın geri kalanı (durak/hat açıklamaları, sefer metinleri, yayıncı adı) toplamda
**birkaç yüz** bulgu; hacmin tamamı `stop_headsign`'dan geliyor çünkü emisyon `stop_times`
**satırı** başına yapılıyor.

**Kanıt:** `jbda-sankobus-sankobus` V3'te **1.089.382** bulgu üretiyor; 531.569'u `JPN_028`,
531.569'u `JPN_030`, yani %97,6'sı tek alandan. Feed 13.940 sefer taşıyor, sefer başına ~38 durak.
`capped_totals` **boş** — hiçbir kapak devreye girmiyor.

🔑 **Künye ile emisyon uyuşmuyor.** Üç kuralın da registry tekilleştirmesi **`Field`**, yani kimlik
"tablo + alan". Emisyon satır başına. Bu, aynı turda `TRP_004` (70.135 → 151) ve `TRF_011`
(829 → 57) için düzeltilen şeklin birebir aynısı: kural kimliğini alan düzeyinde ilan ediyor,
satır düzeyinde basıyor.

**Nerede:** `crates/pipeline/src/k4_cross_ref.rs` içindeki `JpTranslationCoverage`,
`record_v3_translation_pair` ve V3/V4 `iter_trips()` döngüleri. Her `stop_time` için
aynı alanın kaynak değeri biriktirilip K4 sonunda toplu notice'a dönüştürülür.

**Uygulanan düzeltme:** alan başına toplulama. `TRP_004` deseni kullanılarak döngüde biriktirildi,
sonra `(tablo, alan, source_value)` başına TEK notice basıldı; `details`'a etkilenen satır sayısı
ve örnek kayıt taşındı. Beklenen etki, 1.582.080 satır-bulgudan aynı kaynak değerin tekrarları
için tek notice'a düşüştür; bir feed'de kalan notice sayısı kaynak değer grubu sayısına bağlıdır.

⚠️ Kapsamı daraltma önerisi DEĞİL: `stop_headsign` planda bilinçli olarak kapsamda. Sorun
kapsamda değil, granülerlikte.

## 2. ✅ `JPN_028` ile `JPN_030` aynı satırları iki kez sayıyor (uygulandı)

V3 çeviri döngüsü aynı yardımcıyı aynı `(tablo, alan, kayıt)`
için **iki kez** çağırıyor: biri `ja-Hrkt` (kana) için `JPN_028`, biri `ja` için `JPN_030`.
Yani çift basma tasarımın kendisi.

Ölçüm:

| durum | feed |
|---|---:|
| `JPN_028` == `JPN_030`, sayılar birebir eşit | **519** |
| ikisi de var, sayılar farklı | 40 |
| yalnız `JPN_030` (kana tam, `ja` eksik) | 59 |
| yalnız `JPN_028` (`ja` tam, kana eksik) | **0** |

Toplamda `JPN_028` 2.572.255 · `JPN_030` 2.369.613 — ikisi birlikte V3 toplamının **%95,3'ü**.

⚠️ **"Çevirisi hiç yok" açıklaması GEÇERSİZ.** Bu feed'lerin hiçbirinde `translations.txt`
eksik değil: `JPN_004` kökü olan **0** feed var, 618'inin de dosyası var ama kapsamı eksik.
Üreticiler bir alanı ya iki dilde birden çeviriyor ya hiç çevirmiyor.

🔑 Asimetri anlamlı: `ja` tam olup kana eksik olan **hiçbir** feed yok. Yani `JPN_030`
pratikte `JPN_028`'i kapsıyor.

**Uygulanan karar:** aynı kaynak değerde iki dil de eksikse tek `JPN_028`
`aggregate_both` bildirimi; yalnız `language=ja` eksikse toplu `JPN_030`
korunur. Böylece aynı üretici işi iki kez raporlanmaz.

## 3. 🟡 Üç kural açıklanamayan yönde oynadı — platform farkı olabilir

Auto profili 18. koşumla karşılaştırıldığında (aynı arşivli 622 feed) on bir kural hareket etti.
Sekizi beklenen: `JPN_001` +1.542, `JPN_008` +248, `JPN_021` +8, `JPN_010` −1, `JPN_011` +1,
`JPN_019` −1 (JP paketi) ve `TRN_002` −52, `TRN_011` −50 (çeviri kuralı düzeltmesi).

Üçü açıklanamıyor ve **iki yönlü**:

| kural | feed | değişim |
|---|---|---|
| `SHP_017` | `jbda-kagaminotown-kagaminotownbus` | 11 → 12 |
| `SHP_017` | `jbda-kimitsucity-Local_buses_via_Kimitsu_City` | 6 → 3 |
| `SHP_017` | `jbda-tokushima-miyoshicity-miyoshicitybus` | 2 → 1 |
| `STM_014` | `jbda-nantocity-nanbus` | 4 → 6 |
| `STM_014` | `jbda-tokushima-miyoshicity-miyoshicitybus` | 6 → 5 |
| `OPR_008` | `jbda-nantocity-nanbus` | 1 → 2 |

Bu turdaki iki kod commit'i (`654efaf4`, `aafd7c19`) bu kuralların hiçbirine dokunmuyor.
Yerel determinizm test edildi: aynı feed üç koşumda da **aynı** sonucu verdi, yani rastgelelik
değil.

**Kalan hipotez [Varsayım]:** 18. koşum x86_64 Linux'ta, bu tarama macOS arm64'te koştu; şekil
mesafesi ve hız eşiklerinde kayan nokta sonuçları sınırda farklı düşüyor olabilir.

**Ayırt etme yolu:** bu altı feed'i Linux'ta (CI runner veya Docker) aynı binary sürümüyle
koşup karşılaştırmak. Fark kayboluyorsa platform, sürüyorsa gerçek bir kusur.

⚠️ Bu, 1. ve 2. maddelerden **bağımsız** bir kalem; onları düzeltmek bunu çözmez.

## 4. ✅ Korpus denetimi açık profilleri hiç çalıştırmıyor (uygulandı)

İlk binary'de `run_shard.py` profil argümanı vermiyordu. Bu nedenle
`JPN_023`–`JPN_026`, `JPN_028`, `JPN_029`, `JPN_030` ilk full-catalog
koşumunda ölçülmedi; bu kayıt boşluğu elle doldurdu.

**Uygulanan sonuç:** workflow `auto` veya `auto,v3,v4` seçimini kabul ediyor;
aynı ZIP bir kez indirilip profiller arasında yeniden kullanılıyor ve explicit
sonuçlar `analyzer_profiles`, `profile-summary.json` ve `profile-rules.json`
alanlarına yazılıyor. Parity sonuçları yine Auto'dan okunuyor.

## 5. ℹ️ `JPN_023`–`JPN_026` hiç konuşmuyor (kusur değil)

635 Japon feed'inin hiçbirinde, iki profilde de **sıfır** bulgu. Doğrulama kaydı bunu 590 feed'de
söylüyordu; tam kümede ve her iki profilde doğrulandı. Japon feed'leri `feed_lang`, `agency_lang`,
`agency_timezone` ve `currency_type` sabitlerine zaten uyuyor. Kurallar gerçek bir hükmü kodluyor,
bugünkü korpusta örneği yok.

---

## Uygulama sırası

1. **Tamamlandı:** `JPN_028`/`JPN_030` toplulaması ve `aggregate_both` kararı.
2. **Tamamlandı:** ortak tespit kapısından sonra katı V3 `JPN_027` ve `JPN_026` para birimi toplulaması.
3. **Tamamlandı:** audit profil dispatch'i ve Auto parity ayrımı.
4. **Açık:** aynı güncel binary ile Linux/macOS SHP/STM/OPR platform karşılaştırması.

## Uygulama sonucu

Bu takip maddeleri uygulandı:

- V3 `JPN_028` ve `JPN_030`, `(table_name, field_name, source_value)` başına toplulaştırıldı. `affected_records` kaynak satırlarının tamamını koruyor; beş deterministik örnek raporda kalıyor.
- Aynı kaynak değerde hem `ja-Hrkt` hem `language=ja` eksikse tek `JPN_028` `aggregate_both` bildirimi üretiliyor; yalnız `language=ja` eksikse toplu `JPN_030` korunuyor.
- `JPN_027`, ortak JP tespit kapısından sonra explicit V3'te her sayısal `route_type != 3` değeri için tip başına tek bulgu üretiyor. `700..716` ve `800` de V3 sabitini karşılamıyor; Auto ve V4 hâlâ kapsam dışı. Bu davranış, önceki ankraj/çoğunluk sezgisinin düzeltmesidir.
- Audit shard'ı artık aynı indirilen ZIP'i Auto, V3 ve V4 profilleriyle koşabiliyor. Auto sonucu geriye dönük `analyzer` alanında korunuyor; explicit profil sonuçları `analyzer_profiles`, `profile-summary.json` ve `profile-rules.json` içinde tutuluyor.

SHP/STM/OPR platform farkı için eşik veya analitik davranış değiştirilmedi: yerel macOS arm64 ortamında Linux karşılaştırması kanıtlanamazdı. CI denetimi artık Ubuntu runner üzerinde explicit profil ölçümlerini yayımlayacak; bu altı feed için platform hipotezi bu koşumla doğrulanabilir veya reddedilebilir.

## Tespit kapısı — yanlış pozitif ölçümü (2026-09-15, KAPANDI)

`c266f34d` ile JP doğrulaması tespite bağlandı ve tespite iki bağımsız sinyal eklendi:
`agency_lang` Japonca **ve** `agency_timezone=Asia/Tokyo` konjonksiyonu, ayrıca `agency_name`
veya `stop_name` içinde **kana**. Kana şartı bilinçlidir: kanji Çince ve Tayvanca feed'lerle
ortaktır, kana Japoncaya özgüdür.

Sinyallerin Japonya dışında ateşleyip ateşlemediği korpusun TAMAMINDA ölçüldü. Her feed'den
yalnız `agency.txt` ve `stops.txt` üyeleri ZIP merkezî dizini üzerinden çekildi, her üyenin
CRC'si doğrulandı. **4.311 feed'in 4.311'i okundu.**

| sinyal | ateşleyen feed | ülkesi JP olmayan |
|---|---:|---:|
| `agency_name`'de kana | 179 | **0** |
| `stop_name`'de kana | 619 | **0** |
| `agency_lang` + `Asia/Tokyo` | 634 | **0** |
| **en az biri** | **635** | **0** |

🔑 **Ateşleyen 635, korpustaki Japon feed sayısının tam kendisidir.** Tespit artık **635/635**:
yanlış pozitifi de yanlış negatifi de sıfır. Daha önce kaçan beş feed (`mdb-1149`, `mdb-1301`,
`mdb-1302`, `mdb-1303`, `mdb-873`) `agency_lang`+`Asia/Tokyo` koluyla yakalanıyor.

⚠️ **Sinyaller birbirinin yerine geçmiyor, tamamlıyor.** `stop_kana` 619, `lang_tz` 634 feed'de
ateşliyor ve kümeler farklı; kapsamı birleşimleri veriyor. Metadata'sı tamamen bozuk bir Japon
feed'i (ör. `feed_lang=en`, `agency_lang=en`, `Europe/London`) yalnız kana koluyla kurtarılır —
sentetik testle doğrulandı, o feed `auto`'da tespit ediliyor ve V3'te `JPN_023`/`JPN_024`/
`JPN_025` ateşliyor. Tespiti yalnız dil/saat dilimine bağlamak, o üç kuralın yakalaması gereken
vakada susmasına yol açardı; kana kolu bu döngüselliği kırar.

⚠️ **Japonya dışı feed'ler açık profilde tamamen sessiz.** BART, TriMet ve VBB'de `is_gtfs_jp`
`false` ve `v3`/`v4` toplamları `auto` ile birebir aynı (740 · 3.162 · 26.529).

## Önceki Linux ölçümünün statüsü

GitHub Actions'taki 18. koşumun artefaktı yalnızca önceki Linux ölçümünü yeniden gösterir:
[run 34718902532](https://github.com/ttezer/gtfs-analyzer/actions/runs/34718902532), Ubuntu runner, analyzer commit `136ebc32`, `--today 20260820`. Bu, mevcut `6bf7f9a1` binary'sinin Linux koşumu değildir. `136ebc32` ile bugün arasındaki SHP/STM/OPR kod farkları incelemede elenmiştir; bu bir ölçüm değil, kod okumasına dayalı bir hipotez daraltmasıdır.

| Kural / feed | Eski Linux x86_64 kaydı | macOS arm64 kaydı | Fark |
|---|---:|---:|---:|
| `SHP_017` · `jbda-kagaminotown-kagaminotownbus` | 11 | 12 | +1 |
| `SHP_017` · `jbda-kimitsucity-Local_buses_via_Kimitsu_City` | 6 | 3 | −3 |
| `SHP_017` · `jbda-tokushima-miyoshicity-miyoshicitybus` | 2 | 1 | −1 |
| `STM_014` · `jbda-nantocity-nanbus` | 4 | 6 | +2 |
| `STM_014` · `jbda-tokushima-miyoshicity-miyoshicitybus` | 6 | 5 | −1 |
| `OPR_008` · `jbda-nantocity-nanbus` | 1 | 2 | +1 |

Bu tablo yeni bir Linux karşılaştırması değildir; Linux sütunu ile karşılaştırılan eski Linux sütunu aynı koşumdan geldiği için totolojiktir. Mevcut en dürüst sonuç şudur: kod değişikliği hipotezi zayıflatılmıştır, platform farkı en olası açıklamadır; ancak aynı güncel binary iki mimaride koşulmadığı için platform farkı doğrulanmış değildir. Eşik veya kural mantığı değiştirilmedi. Kalem, güncel binary ile Linux koşumu yapılana kadar açıktır.

Sonraki full audit koşumunda `benchmark/audit_all/platform_probe.py`, bu dört feed ve altı kural satırını güncel Linux runner, commit ve mimari bilgisiyle `platform-probe.json` olarak çıkarır. Bu artefakt gerçek kapanış kanıtı olacaktır.
