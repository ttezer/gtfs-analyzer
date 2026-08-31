# GTFS Validator & Analyzer

🇹🇷 **Türkçe** · 🇬🇧 [English](README.en.md) · 🇯🇵 [日本語](README.ja.md) · 🇫🇷 [Français](README.fr.md)

[![Uygulamayı Aç](https://img.shields.io/badge/Uygulamay%C4%B1%20A%C3%A7-gtfs--analyzer-2ea44f?style=flat&logo=googlechrome&logoColor=white)](https://ttezer.github.io/gtfs-analyzer/)
[![GTFS-JP](https://img.shields.io/badge/GTFS--JP-v3%2Fv4%20destekli-c8102e?style=flat)](https://www.gtfs.jp/)
[![Kural sayısı](https://img.shields.io/badge/kural-612-blue?style=flat)](RULES.md)
![GTFS Spec kapsamı](https://img.shields.io/badge/GTFS%20Spec-97.2%25-007ec6?style=flat)
[![Korpus doğrulaması](https://img.shields.io/badge/korpus-4318%20feed%20%C3%97%2012%20ko%C5%9Fum-brightgreen?style=flat)](audit-results/)
[![crates.io](https://img.shields.io/crates/v/gtfs-analyzer?style=flat&label=crates.io)](https://crates.io/crates/gtfs-analyzer)
[![npm](https://img.shields.io/npm/v/gtfs-sdk?style=flat&label=npm)](https://www.npmjs.com/package/gtfs-sdk)
[![Lisans MIT](https://img.shields.io/badge/lisans-MIT-yellow?style=flat)](LICENSE)

**GTFS Validator & Analyzer**, GTFS dosyalarını doğrudan tarayıcıda doğrulayan açık kaynak bir **GTFS validator** ve feed kalite analiz aracıdır. Yüklenen `.zip` hiçbir sunucuya gönderilmez; doğrulama tamamen **WebAssembly** ile kullanıcının cihazında çalışır. Tarayıcı, **CLI** (`cargo install gtfs-analyzer`), **Rust kütüphanesi**, **CI/CD** ve **`gtfs-sdk` npm paketi** olmak üzere beş yoldan kullanılabilir.

**612 doğrulama kuralı** ile GTFS spesifikasyonunun ölçülebilir hükümlerinin **%97,2'sini** karşılar ve alan tablosunun **300 atomunun 300'ünde** en az bir Spec çapası taşır. Bu kuralların **417'si** son 4.318 feed'lik tam katalog koşumunda en az bir bulgu üretti; GTFS-JP ek kuralları ayrıca 585 feed'lik profil koşumunda ölçüldü. Kuralların tamamı [`RULES.md`](RULES.md) altında listelidir.

Doğruluk iddiası, MobilityData'nın resmî `gtfs-validator` aracına karşı **on iki tam katalog koşumuyla** sınanmıştır: her koşumda MobilityDatabase kataloğunun test edilebilir her GTFS Schedule feed'i — son koşumda **4.318** —, iki validatörle **aynı makinede, aynı gün** doğrulanır — MobilityData tarafında gerçek **Java** `gtfs-validator v8.0.1` çalıştırılır, rapor karşılaştırması yapılmaz. Ham sonuçların tamamı depoda: [`audit-results/`](audit-results/).

GTFS Validator & Analyzer yalnızca dosyanın spesifikasyona uygun olup olmadığını kontrol etmez; feed'in ne kadar güvenilir, tutarlı ve kullanılabilir olduğunu da analiz eder. Hataları ilgili dosya ve satır numarasıyla birlikte gösterir, her bulgu için düzeltme adımları sunar ve coğrafi sorunları — örneğin sapan güzergâhlar, bozuk koordinatlar veya erişilemeyen duraklar — interaktif harita üzerinde işaretler.

Her bulgu; kural kodu, analiz sınıfı ve önem seviyesiyle etiketlenir. Spec · Interop · Quality · Analytics sınıfları ile Kritik → Bilgi önem seviyeleri sayesinde binlerce bulgu filtrelenebilir, önceliklendirilebilir ve sistematik biçimde ele alınabilir. Araç ayrıca feed'in kullandığı GTFS özelliklerini — Shapes, Transfers, Fares, Headsigns, Flex ve benzerlerini — otomatik olarak tespit ederek rapora dahil eder.

GTFS Validator & Analyzer, spesifikasyon doğrulamasını operasyonel kalite analiziyle genişletir. Hat bazında sefer sıklığı tutarsızlıkları, anormal hız segmentleri, izole duraklar, servis desenlerindeki boşluklar ve ağ topolojisi problemleri 612 farklı doğrulama ve analiz kuralıyla incelenir. Sonuçlar, uyumluluk ve kaliteyi ayrı ayrı değerlendiren skorlarla özetlenir. Önceliklendirilmiş düzeltme kuyruğu ise hangi sorunların önce ele alınması gerektiğini ve yapılacak düzeltmelerin skora olası etkisini gösterir.

**Kimler için?**

- **Toplu taşıma işletmecileri ve belediyeler** — Feed'i yayına almadan önce doğrulamak ve kalite sorunlarını gidermek için.
- **GTFS entegratörleri ve danışmanlar** — Teslim edilen verinin teknik ve operasyonel kalitesini belgelemek için.
- **Uygulama geliştiriciler** — Kullandıkları feed'in güvenilirliğini ve entegrasyon risklerini değerlendirmek için.
- **Araştırmacılar ve analistler** — Farklı toplu taşıma ağlarını veri kalitesi ve yapı bakımından karşılaştırmak için.

---

## Diğer Araçlarla Karşılaştırma

### Özellikler

| Özellik | MobilityData | GTFS Analyzer |
|---|:---:|:---:|
| Web arayüzü | ✅ | ✅ |
| Veri sunucuya gitmiyor | ❌ | ✅ |
| Spec uyum kuralları | ✅ | ✅ |
| Kalite kuralları | ❌ | ✅ |
| Operasyonel analitik | ❌ | ✅ |
| Harita görselleştirme | ❌ | Durak, güzergah, sefer, hat, pathway |
| Feed skoru | ❌ | ✅ |
| Düzeltme önerisi | Kısmi | ✅ |
| GTFS Flex desteği | Kısmi | ✅ |
| Fares v2 doğrulama | Kısmi | ✅ |
| GTFS-JP profil doğrulama | ❌ | ✅ |
| Çıktı formatı | HTML, JSON | HTML, CSV, JSON, PDF |
| Dağıtım | Web · masaüstü kurulum (msi/dmg/deb) · CLI JAR · Docker | Web · CLI binary · `cargo install` · npm SDK |
| Belgelenmiş CI/CD entegrasyonu | README'de tarif yok (Docker/CLI ile mümkün) | ✅ `--fail-on` + exit kodu |
| npm paketi | ❌ | ✅ `gtfs-sdk` |
| crates.io paketi | — *(Java projesi)* | ✅ `gtfs-analyzer` |
| GTFS Spec kapsamı (ölçülmüş) | — | **%97,2** · 300/300 alan çapası |
| **Toplam kural** | **178** | **612** |

### Korpus Doğrulaması

Doğruluk birkaç feed'le gösterilemez. Her sürüm, **MobilityDatabase'in tüm GTFS Schedule kataloğuna** karşı koşturulur: son koşumda **4.318 feed**, 640 paralel shard. Karşı tarafta MobilityData'nın **`gtfs-validator` v8.0.1**'i, yayımlanmış raporları okunarak değil **aynı arşiv üzerinde yeniden çalıştırılarak** — böylece fark "kim ne buldu" olur, "kimin raporu ne zaman üretildi" değil.

Son koşumdan (`32587015142`, 4.275 feed'de iki taraf da temiz):

| | GTFS Analyzer | MobilityData |
|---|---|---|
| Medyan süre | **0,05 sn** | 3,00 sn |
| Medyan tepe bellek | **14 MB** | 329 MB |
| Bitiremediği feed | **1** | 10 |
| MD'nin görüp bizim göremediğimiz | **0 olgu** | — |

Ham çıktılar [`audit-results/`](audit-results/) altında — ilk yedi koşum depoda, sonrakiler `audit-<run-id>` prerelease'i olarak arşivleniyor.

### Feed Analizi Örnekleri

Aşağıdaki sayılar yukarıdaki korpus koşumundan alınmıştır: aynı arşiv, aynı gün (2026-08-20), MobilityData tarafında Java `gtfs-validator v8.0.1`.

#### BART (Bay Area Rapid Transit, San Francisco)

Feed: `mdb-53` · 14 hat, 287 durak, 4.417 sefer · 0,9 MB

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| Toplam bulgu | 2.715 | 740 |
| Kritik / Error | 2 | 2 |
| Yüksek / Warning | 2.654 | 1 |
| Orta | — | 11 |
| Düşük | — | 24 |
| Bilgi / Info | 59 | 702 |
| Tetiklenen kural tipi | 13 | **37** |
| Doğrulama süresi | 3,43 sn | **0,19 sn** |
| Yayın skoru | — | **92,6 / 100** |
| Genel skor | — | **90,9 / 100** |

> MobilityData'nın 2.654 uyarısının neredeyse tamamı tek koddan gelir. GTFS Analyzer aynı iki kritik hatayı bulur, üstüne 24 farklı kural tipinde operasyonel bulgu ekler.

#### TriMet (Portland, Oregon)

Feed: `mdb-247` · 112 hat, 6.480 durak, 70.557 sefer · 28,4 MB

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| Toplam bulgu | 51 | 3.099 |
| Kritik / Error | 0 | 0 |
| Yüksek / Warning | 38 | 12 |
| Orta | — | 97 |
| Düşük | — | 497 |
| Bilgi / Info | 13 | 2.493 |
| Tetiklenen kural tipi | 8 | **49** |
| Doğrulama süresi | 14,85 sn | **5,46 sn** |
| Yayın skoru | — | **100 / 100** |
| Genel skor | — | **90,0 / 100** |

> Spec açısından temiz bir feed: iki araç da 0 kritik bulur ve yayın skoru 100'dür. Aradaki 49'a 8'lik kural farkı, GTFS Analyzer'ın spec uyumunun ötesinde operasyonel kalite de ölçmesinden gelir.

#### Tokyo Toei (Tokyo Metropolitan Bureau of Transportation)

Feed: `mdb-3175` · 151 hat, 5.370 durak, 68.817 sefer · 8,6 MB · **GTFS-JP profili**

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| Toplam bulgu | 1.849 | 1.741 |
| Kritik / Error | 0 | 0 |
| Yüksek / Warning | 268 | 12 |
| Orta | — | 809 |
| Düşük | — | 548 |
| Bilgi / Info | 1.581 | 372 |
| Tetiklenen kural tipi | 8 | **49** |
| Doğrulama süresi | 5,94 sn | **1,75 sn** |
| Yayın skoru | — | **100 / 100** |
| Genel skor | — | **87,2 / 100** |

> GTFS-JP profili gerçek bir Japon feed'inde yanlış pozitif üretmez: feed spec açısından temizdir (0 kritik, yayın skoru 100) ve profil kuralları yalnız Japonya'ya özgü alanları denetler.

#### VBB (Berlin-Brandenburg Ulaşım Birliği)

Feed: `mdb-782` · 1.274 hat, 41.961 durak, 258.524 sefer, 14.485 shape · **~75 MB**

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| Toplam bulgu | 12.201 | 25.369 |
| Kritik / Error | 0 | 0 |
| Yüksek / Warning | 11.486 | 1.307 |
| Orta | — | 7.440 |
| Düşük | — | 8.186 |
| Bilgi / Info | 715 | 8.436 |
| Tetiklenen kural tipi | 18 | **91** |
| Doğrulama süresi | 45,16 sn | **21,07 sn** |
| Genel skor | — | **78,4 / 100** |

> 🇩🇪 **Bu feed, MobilityData'nın barındırılan web doğrulayıcısının işleyemeyeceği kadar büyüktür.** GTFS Analyzer aynı feed'i **doğrudan tarayıcıda**, dosyayı hiçbir sunucuya göndermeden doğrular. MobilityData toplamının yarıdan fazlası (`non_ascii_or_non_printable_char`) feed'in Almanca metnindeki meşru ü/ö/ä/ß karakterleridir; GTFS Analyzer geçerli Unicode harfleri işaretlemez. Çekirdek kontrollerde iki araç hizalıdır.

---

## GTFS-JP Desteği

GTFS Analyzer, Japonya'nın ulusal GTFS profili **GTFS-JP**'yi (国土交通省 / MLIT standardı) otomatik olarak tanır ve standart GTFS'in isteğe bağlı bıraktığı, GTFS-JP'nin zorunlu kıldığı kuralları uygular. MLIT, sübvansiyon alan işletmecilerden GTFS-JP yayımlamasını şart koştuğu için yüzlerce küçük operatör bu profile uymak zorundadır; ancak yaygın doğrulayıcılar profile özgü zorunlulukları denetlemez.

**Otomatik tespit.** Bir feed; GTFS-JP v3'te kullanılan (`agency_jp.txt`, `office_jp.txt`, `pattern_jp.txt`) uzantı dosyalarından birini veya eski sürüm uyumluluğu için tanınan `routes_jp.txt` dosyasını içeriyorsa, `feed_lang` değeri `ja` ile başlıyorsa ya da `translations.txt` içinde kana (`ja-Hrkt`) okumaları taşıyorsa GTFS-JP olarak işaretlenir ve raporda **GTFS-JP** rozeti görünür. `routes_jp.txt` v3 dosyası değildir; yalnızca eski feed'lerin tanınması için korunur. MLIT GTFS-JP v4 bu üç v3 uzantı dosyasını ana standardın dışına almıştır; Analyzer feed'i v3 veya v4 diye etiketlemez. Varsayılan kural kapsamı **auto** profilidir; web/CLI/WASM üzerinden açıkça `v3` veya `v4` seçilebilir. V4 profilinde v3 uzantı dosyaları referans kabul edilir ve bunlara bağlı JPN kuralları çalışmaz. Profil kuralları yalnızca GTFS-JP sinyali taşıyan feed'lerde devreye girer; standart feed'lerde sessiz kalır.

**Profil seçimi (analiz sırasında).** Web uygulamasında ZIP'i seçmeden önce **Analiz Kriterleri** panelini açın ve **GTFS-JP profil kapsamı** alanından `Auto`, `V3` veya `V4` seçin. Feed'i seçtiğiniz anda mevcut seçim kaydedilir ve analiz otomatik başlar; `Auto` varsayılandır. CLI için `--gtfs-jp-profile v3` veya `--gtfs-jp-profile v4` kullanın. SDK'da aynı seçimi `config: { gtfs_jp_profile: 'v3' }` ya da `'v4'` ile verin. Bu seçim feed'in resmî sürümünü tespit etmez; yalnızca uygulanacak doğrulama kapsamını belirler. Ayrıntılı farklar için [GTFS-JP v3/v4 uyumluluk matrisine](docs/gtfs-jp-v3-v4-matrix.md) bakın.

**Profil kuralları (JPN grubu).**

| Kural | Denetim |
|---|---|
| **JPN_001** | Durak adlarının kana (よみがな — `translations.txt`, `ja-Hrkt`) okuması; sesli anons ve arama için GTFS-JP'de zorunludur |
| **JPN_002** | `jp_office_id` (`trips.txt` **veya** `routes.txt`) değerinin `office_jp.txt`'teki bir `office_id` ile eşleşmesi (işletme ofisi referans bütünlüğü) |
| **JPN_003** | `agency_jp.txt` `agency_id` değerinin `agency.txt`'te tanımlı olması (işletici referans bütünlüğü) |
| **JPN_004** | `translations.txt`'in mevcudiyeti — GTFS-JP'de (özellikle kana okumaları için) zorunludur |
| **JPN_005** | `office_jp.txt`'te `office_name` zorunlu alanının dolu olması |
| **JPN_006** | `fare_attributes.txt` zorunluluğu ve farklı ücret profillerinde `fare_rules.txt` koşulu |
| **JPN_007** | `feed_info.txt`'in mevcudiyeti — GTFS-JP'de zorunludur |
| **JPN_008** | Hat adının (`route_long_name`) kana (`ja-Hrkt`) okuması |
| **JPN_009** | `trip_headsign` kana (`ja-Hrkt`) okuması |
| **JPN_010** | İşletici adının (`agency_name`) kana (`ja-Hrkt`) okuması |
| **JPN_011** | GTFS-JP feed'inde tek işletici olsa bile `agency_id` zorunluluğu |
| **JPN_012** | `agency_jp.agency_id` eksikliği |
| **JPN_013** | Varsa `agency_zip_number` değerinin 7 ASCII rakam olması |
| **JPN_014** | `office_jp.office_id` eksikliği ve tekrarları |
| **JPN_015** | Eski `routes_jp.route_id` uyumluluk kontrolü; v3 dosyası değildir |
| **JPN_016** | `pattern_jp.route_update_date`; legacy `routes_jp.route_update_date` geçerli tarih biçimi |
| **JPN_017** | `pattern_jp.jp_pattern_id` eksikliği ve tekrarları |
| **JPN_018** | Mevcut `pattern_jp.txt` içindeki kopuk `trips.jp_pattern_id` referansı |
| **JPN_019** | GTFS-JP `ja-Hrkt` satırlarında geçersiz kayıt/alan/alt kayıt |
| **JPN_020** | `office_url` ve `office_phone` biçim kalite kontrolü |
| **JPN_021** | Kana çevirilerinde boş, çakışan veya tutarsız kayıtlar |
| **JPN_022** | GTFS-JP v4'te `agency_lang`, `feed_start_date`, `feed_end_date` ve `feed_version` zorunlu alanlarının eksikliği |

Yukarıdaki **Tokyo Toei** karşılaştırması bu profilin gerçek bir GTFS-JP feed'inde nasıl davrandığını gösterir: feed spec açısından temizdir (0 kritik) ve profil kuralları doğru referanslı veride yanlış pozitif üretmez.

---

## Kullanım

GTFS Validator & Analyzer bir web uygulamasıdır; kurulum gerektirmez. Canlı sürümü tarayıcıda açıp GTFS zip dosyanızı yükleyin.

Motor tarayıcı yeteneğine göre otomatik seçilir: Memory64 destekleniyorsa 4 GB üzerindeki
büyük feed'ler için **WASM64**, desteklenmiyorsa **WASM32** kullanılır. Aktif motor yükleme
ekranında gösterilir. Hata ayıklamak için `?wasm32=1`, `?wasm64=1` veya `?serial=1` kullanılabilir.

**→ [https://ttezer.github.io/gtfs-analyzer/](https://ttezer.github.io/gtfs-analyzer/)**

1. GTFS zip dosyanızı sürükleyip bırakın ya da dosya seçiciyle yükleyin.
2. Doğrulama otomatik başlar; ilerleme ekranda aşama aşama gösterilir.
3. Tamamlandığında Yayın ve Genel skorları ile ayrıntılı rapor sekmeleri görünür.
4. Önceki bir analizi karşılaştırmak için **Karşılaştır** sekmesinden eski Golden JSON'u yükleyin. Düzeltilen, yeni, azalan ve artan kurallar; skorlar, feed tarihleri ve normalize edilmiş notice yoğunlukları birlikte gösterilir.
5. Paylaşılabilir bir çıktı için **Dışa Aktar → Yönetici PDF Raporu** yolunu açın, rapor dilini seçin ve önizlemedeki **Yazdır / PDF Kaydet** düğmesini kullanın.

### Yönetici PDF Raporu

**Yönetici PDF Raporu**, ayrıntılı doğrulama sonuçlarını karar vericiler ve feed üreticileri için okunabilir, renkli ve A4 uyumlu bir belgeye dönüştürür. Rapor yalnızca **GTFS Analyzer** sonuçlarından oluşturulur; başka bir validator sonucu veya harici karşılaştırma içermez.

Rapor şunları kapsar:

- yayınlanabilirlik durumu, Yayın Skoru, Genel Skor ve Spec · Interop · Quality · Analytics bileşenleri;
- durak, hat, sefer, shape, servis günü ve tarih aralıklarından oluşan feed profili;
- R1 yayın engelleri ile R9 etki/efor sıralamasını birleştiren, kural bazında tekilleştirilmiş **P0 / P1 / P2** aksiyonları;
- her öncelikli bulgu için kanıt, etkisi, önerilen düzeltme, etkilenen gerçek örnek sayısı ve olası skor kazanımı;
- feed'e özgü yapısal içgörüler, aşamalı iyileştirme planı, önem/sınıf dağılımları ve teknik ek.

Arayüzde performans için sınırlandırılmış bulgu örnekleri bulunsa bile rapor, mevcut olduğunda `capped_totals` içindeki **gerçek toplu sayıları** kullanır. Belge Türkçe, İngilizce veya Japonca üretilebilir; rapor dili arayüz dilinden bağımsız seçilir. Oluşturma ve yazdırma işlemi tamamen tarayıcıda gerçekleşir, GTFS verisi sunucuya gönderilmez ve harici API kullanılmaz.

> Rapor skorları yüklenen GTFS feed'ini değerlendirir; GTFS Analyzer uygulamasının performansını veya doğruluğunu puanlamaz.

> Kendi sunucunuzda barındırmak veya geliştirme ortamı kurmak için [Geliştirici Kurulumu](#geliştirici-kurulumu) bölümüne bakın.

---

## Beş Kullanım Yolu

Aynı doğrulama çekirdeği (`gtfs_pipeline::validate_bytes`) beş şekilde çalışır — hepsi aynı 612 kuralı, aynı sonucu üretir:

| yol | ne için | veri nereye gider |
|---|---|---|
| **Tarayıcı** ([uygulama](https://ttezer.github.io/gtfs-analyzer/)) | tek feed'i açıp haritayla incelemek | **hiçbir yere** — WebAssembly ile cihazda |
| **CLI** (`cargo install gtfs-analyzer` ya da hazır binary) | toplu doğrulama, betikleme, Python entegrasyonu | hiçbir yere — yerel binary |
| **Rust kütüphanesi** ([`gtfs-pipeline`](https://crates.io/crates/gtfs-pipeline)) | doğrulamayı kendi Rust servisinize gömmek | hiçbir yere — kendi süreciniz |
| **CI/CD** (exit kodu + `--fail-on`) | feed yayına çıkmadan önce pipeline kapısı | hiçbir yere — kendi runner'ınız |
| **[`gtfs-sdk`](https://www.npmjs.com/package/gtfs-sdk) npm paketi** | kendi web veya Node uygulamanıza gömmek | hiçbir yere — yerel WASM |

Hiçbirinde feed sunucuya yüklenmez. Bu, barındırılan doğrulayıcılardan temel farktır: ticari sözleşme gereği dışarı çıkamayan veriyi de doğrulayabilirsiniz.

### CI/CD entegrasyonu

`--fail-on` bayrağı yalnız istediğiniz sınıf/önem seviyesinde koşuyu düşürür, böylece Analytics gürültüsü pipeline'ı kırmaz:

```yaml
# GitHub Actions — yalnız resmî GTFS Spec ihlalleri koşuyu düşürsün
- name: GTFS feed doğrulama
  run: |
    curl -sL https://github.com/ttezer/gtfs-analyzer/releases/latest/download/gtfs-analyzer-x86_64-linux.tar.gz | tar xz
    ./gtfs-analyzer validate feed.zip --fail-on-class spec --min-severity critical
```

Exit kodları: `0` temiz · `1` eşiği aşan bulgu var · `2` feed okunamadı (fatal).

### Rust kütüphanesi

Doğrulamayı kendi Rust servisinize gömmek için `gtfs-pipeline`'ı doğrudan kullanın — CLI'a, dosya sistemine ya da ağa ihtiyaç yok:

```toml
[dependencies]
gtfs-pipeline = "0.11.1"
gtfs-config   = "0.11.1"
gtfs-core     = "0.11.1"
```

```rust
use gtfs_config::ValidatorConfig;
use gtfs_core::ValidateResult;
use gtfs_pipeline::validate_bytes;

let zip = std::fs::read("feed.zip")?;
let config = ValidatorConfig::default();

match validate_bytes(&zip, &config, 20_260_820) {
    ValidateResult::Ok(result) => {
        println!("bulgu: {}", result.notices.len());
        println!("yayın skoru: {}", result.reports.r5.pub_score);
    }
    ValidateResult::Fatal(err) => eprintln!("fatal: {}", err.message),
}
```

`validate_bytes` baytları alır ve tüm raporları (`r1`–`r9`), skorları ve bulguları taşıyan bir sonuç döndürür. Eşikleri değiştirmek için `ValidatorConfig` alanlarını ayarlayın ya da `merge_delta` ile JSON bir delta uygulayın.

⚠️ Kütüphane crate'leri analyzer'ın **iç yapısıdır**; binary crates.io'dan derlenebilsin diye yayımlanmışlardır ve **API kararlılığı garantisi taşımazlar**. Kararlı bir yüzey istiyorsanız CLI'ın JSON çıktısı ya da `gtfs-sdk` daha güvenlidir.

### `gtfs-sdk` npm paketi

`gtfs-sdk`, v0.11.1 doğrulama motorunu typed JavaScript/TypeScript API olarak sunar. Feed uygulamadan çıkmadan yerel WASM ile doğrulanır:

```js
import { validateGtfs } from "gtfs-sdk";

const result = await validateGtfs(new Uint8Array(zipBytes), {
  today: "2026-08-20",
});
console.log(result.notices.length, result.reports.r5.score);
```

Dışa açılan public API `validateGtfs`, `getVersion` ve progress/cache akışı gereken uygulamalar için `createValidatorSession` içerir. Düşük seviyeli `gtfs-wasm` binding'i SDK sözleşmesinin parçası değildir. WASM64 ve threaded motor seçimi ise ilk SDK paketinde internal kalır.

Paket kaynakları `sdk/` altındadır; ayrıntılı kullanım, sonuç modeli ve config referansı [`sdk/README.md`](sdk/README.md) içindedir. WASM binding'i build sırasında `crates/wasm` üzerinden üretilir.
Web UI worker'ı da aynı `ValidatorSession` facade'ını kullanır; seri/threaded/WASM64 seçimini yalnızca uygulama içindeki engine adapter belirler.

## CLI (Terminal)

Aynı doğrulama çekirdeğini terminalden çalıştırabilirsiniz — toplu iş, betikleme ve Python/otomasyon entegrasyonu için.

### Kurulum

Rust kuruluysa en kısa yol:

```bash
cargo install gtfs-analyzer
gtfs-analyzer validate feed.zip
```

Rust kurmadan: [Releases](https://github.com/ttezer/gtfs-analyzer/releases) sayfasından platformunuza uygun arşivi indirin (`x86_64-linux`, `aarch64-macos`, `x86_64-windows`), açın ve `gtfs-analyzer` binary'sini `PATH`'inize koyun.

```bash
# Linux / macOS — en son sürüm
curl -sL https://github.com/ttezer/gtfs-analyzer/releases/latest/download/gtfs-analyzer-x86_64-linux.tar.gz | tar xz
./gtfs-analyzer --version
# Sürümle birlikte deterministik provenance bilgisi
./gtfs-analyzer --version --verbose
```

Kaynaktan derlemek için:

```bash
cargo build --release -p gtfs-analyzer
target/release/gtfs-analyzer validate feed.zip --json

# ya da doğrudan
cargo run -p gtfs-analyzer -- validate feed.zip --json
```

### `validate` — feed doğrulama

| Bayrak | Açıklama |
|---|---|
| `--json` | Sonucun tamamını JSON olarak yazar |
| `--summary` | Kısa özet: durum, notice sayısı, skorlar (varsayılan; `--json` ile birlikte kullanılamaz) |
| `--rule SHP_010` | Yalnızca verilen kural için notice'lar |
| `--severity critical` | Tam olarak bu öneme sahip notice'lar (critical/high/medium/low/info) |
| `--min-severity high` | Bu önem ve daha ağırı (critical en ağır) |
| `--class spec` | Yalnızca bu kural sınıfları — `spec,interop,quality,analytics`, virgülle çoklu |
| `--fail-on critical` | Exit 1 **yalnızca** bu önem ve daha ağırı varsa |
| `--fail-on-class spec` | Exit 1 yalnızca bu sınıflarda notice varsa |
| `--pretty` | JSON'ı girintili yazar (`--json` gerektirir) |
| `--include-name-index` | `name_index`'i (durak/hat/shape arama tabloları) JSON'a dahil eder |
| `-o rapor.json` | Çıktıyı stdout yerine dosyaya yazar |
| `--lang en` | Bulgu metinlerinin dili: `en` (varsayılan) / `tr` / `ja` / `fr` |
| `--config config.json` | JSON config delta uygular (`ValidatorConfig::default()` üzerine) |
| `--gtfs-jp-profile auto\|v3\|v4` | GTFS-JP kural profilini açıkça seçer; config değerini geçersiz kılar |
| `--today 20260710` | Analiz "bugün"ünü sabitler (takvim kuralları için) |

**Filtreler yalnızca görüntülemeyi daraltır.** `notices` ve R2–R9 listeleri filtrelenir; **R1 yayınlanabilirlik kararı ve R5 skorları her zaman tüm feed'i** anlatır. Filtre uygulandığında JSON'a `filtered` alanı, özete `filter:` satırı eklenir.

`name_index` varsayılan olarak **çıktıya dahil edilmez**: büyük feed'lerde shape/durak koordinat tabloları JSON'un neredeyse tamamını kaplar. Gerekiyorsa `--include-name-index` ile açın.

Feed yolu yerine `-` verilirse ZIP **stdin'den** okunur: `curl -sL <url> | gtfs-analyzer validate - --json`. (ZIP merkezi dizini dosyanın sonunda olduğundan arşiv belleğe alınır, akış hâlinde işlenmez.)

> **Arayüzle sayı farkı:** Tarayıcı, kural başına bulgu örneklerini performans için sınırlar (gerçek toplamlar `capped_totals`'ta bildirilir). CLI bu sınırı **uygulamaz** — aynı feed'de daha çok notice ve sınırlanmamış R9 etki değerleri döner. Fark beklenen davranıştır; iki çıktıyı doğrudan sayı sayı karşılaştırmayın.

**Exit kodları:** `0` eksiksiz ve notice'sız doğrulama · `1` notice veya `PARTIAL` (kısmi kapsam) raporu · `2` fatal ya da config/dosya hatası. `PARTIAL` rapor, bozuk/eksik bir dosyayı güvenle atlayıp bağımsız kontrolleri sürdürür; JSON'da `status: "partial"`, `validation_status: "PARTIAL"` ve `partial` kapsamı görünür. `partial.skipped_checks`, önkoşul eksikliği nedeniyle çalıştırılmayan K4/K5/K6 kontrol ailelerini ve kurallarını ayrıntılı olarak listeler; `partial.skipped_stages` kaba aşama metadatası için korunur. `--fail-on*` kullanılsa da `PARTIAL` koşusu `1` döner. JSON modunda stdout yalnızca JSON'dur; hatalar stderr'e yazılır.

```bash
# CI kapısı: yalnız resmi GTFS Spec ihlalleri koşuyu düşürsün
gtfs-analyzer validate feed.zip --fail-on-class spec

# Yalnız Spec bulgularını raporla (skorlar yine tüm feed'i anlatır)
gtfs-analyzer validate feed.zip --class spec --json --pretty -o spec.json
```

```python
import json, subprocess

proc = subprocess.run(
    ["target/release/gtfs-analyzer", "validate", "feed.zip", "--json"],
    text=True, capture_output=True,
)
# exit 1 = "notice var", hata değil — check=True KULLANMAYIN
data = json.loads(proc.stdout)
if data["status"] == "fatal":
    raise SystemExit(f'{data["code"]}: {data["message"]}')
for n in data["notices"]:
    print(n["rule_id"], n["severity"], n["rule_class"])
```

### `rules` — kural kaydı

Doğrulama çalıştırmadan tüm kural kaydını listeler; entegre eden projenin kural sözlüğü için.

```bash
gtfs-analyzer rules --class spec --severity critical
gtfs-analyzer rules --rule STM_004 --json --pretty
```

Alanlar: `id`, `severity`, `class`, `authority_source`, `base_effort`, `blocks`, `title`.
`--class` / `--severity` / `--min-severity` / `--rule` filtreleri `validate` ile aynı anlamdadır.
`--lang` burada da geçerlidir (kural başlıkları).

### Çıktı dili

Doğrulama çekirdeği bulgu metinlerini Türkçe üretir; `--lang en` / `--lang ja` / `--lang fr` bunları arayüzün kullandığı **aynı çeviri sözlükleriyle** değiştirir. Kural ID'leri, önem ve sınıf değerleri (`CRITICAL`, `SPEC`) her dilde makine-okunur sabit kalır; yalnızca `title`, `message` ve `remediation` çevrilir.

Bir kuralın çevirisi yoksa sıra şudur: istenen dil → İngilizce → Türkçe (çekirdeğin ürettiği metin). Böylece çıktı hiçbir zaman boş kalmaz.

Sözlükler `ui/src/locales/{en,ja}.ts` dosyalarından `npm run locales:export` ile `crates/cli/locales/*.json` içine türetilir ve CLI binary'sine gömülür. Locale güncellenip export çalıştırılmazsa `locale-parity.test.ts` CI'da kırmızı yanar — tek kaynak locale dosyalarıdır.

---

## Analiz Kriterleri

Yükleme ekranındaki **Analiz Kriterleri** bölümünden doğrulama eşikleri özelleştirilebilir. Değiştirilen alanlar bir sonraki ZIP yüklemesinde uygulanır; sıfırla butonu varsayılanlara döndürür.

### Kural Sınıfları ve Otorite Kaynağı

Her kural dört sınıftan birine ayrılır. Sınıf, bulgunun **otorite kaynağını** (meşruiyet dayanağını) yansıtır; kullanıcı "bu gerçek bir GTFS Spec hatası mı, yoksa uyumluluk/kalite/analitik sinyal mi" ayrımını raporda net görür:

- **Spec** — yalnızca resmi **GTFS Schedule Reference** tarafından açıkça zorunlu/yasak/geçersiz tanımlanan durumlar (required / conditionally required / conditionally forbidden alanlar, enum-değer, foreign-key, uniqueness, format kısıtları). Başka hiçbir kaynak `Spec` üretmez.
- **Interop** — MobilityData, Google Transit veya bölgesel profil (ör. GTFS-JP) gibi tüketici/validator davranışlarıyla uyumluluk sinyalleri.
- **Quality** — GTFS best-practice, veri kalitesi, okunabilirlik, tutarlılık ve üretim kalitesi kontrolleri.
- **Analytics** — istatistiksel, operasyonel, performans veya analiz amaçlı sinyaller.

Her kuralın ayrıca makine-okunur bir **otorite kaynağı** (`authority_source`) alanı vardır (`GTFS_SPEC`, `MOBILITYDATA_PARITY`, `REGIONAL_PROFILE`, `PROJECT_QUALITY` vb.). Değişmez kural: **`Spec` sınıfı yalnızca `authority_source = GTFS_SPEC` ile meşrudur**; MobilityData/Guru/Google paritesi, best-practice veya proje-özel sezgi tek başına Spec kanıtı değildir.

### İsteğe Bağlı Profiller ve Kaynak URL

Config delta içinde `stop_name_best_practices=true` verilirse dil-bağımlı `STP_040` ve `STP_041` kontrolleri etkinleşir; yanlış pozitif riski nedeniyle varsayılan kapalıdır. URL tabanlı entegrasyonlar `source_url` metadata'sı sağlayabilir; `ARC_028` kalıcı yayın adresinin `.zip` dosya adı taşımasını denetler. Dosya yükleme modunda bu kontrol sessizdir. Core motor feed içindeki URL'lere ağ isteği yapmaz; 404 kontrolü ayrı ve açıkça opt-in bir online adapter gerektirir.

### Shape mesafe alanlarının birlikte kullanımı

`stop_times.txt` içinde `shape_dist_traveled` kullanan bir trip'in referansladığı `shapes.txt` noktalarının bir kısmında aynı alan eksikse `SHP_030` (Quality · Orta) üretilir. Bu iki alan GTFS'te ayrı ayrı opsiyoneldir; kural bir Spec yayın engeli değil, tüketicilerin durakları shape geometrisiyle güvenilir eşleştirememe riskini shape başına toplar. Etkilenen trip sayısı ve örnek kimlikler notice ayrıntısında gösterilir.

Tek noktadan oluşan ve gerçekten bir trip tarafından kullanılan shape `SHP_006` ile Düşük · Quality olarak raporlanır; ayrıntıda `shape_id` ve `shape_point_count=1` bulunur. İki noktalı düz segment geçerlidir. Kullanılmayan tek noktalı shape yalnız `SHP_018` ile raporlanır. Bu, MobilityData `single_shape_point` sinyaline bilinçli bir near-parity eşlemesidir; Analyzer yalnız kullanılan shape'i SHP_006 ile raporlar.

### Uzak durak hız paritesi

MobilityData'nın güncel rules sayfası `fast_travel_between_far_stops` için tutarsızdır: ana WARNING tablosunda kural aktif görünürken notice-detail metadata'sı `Deprecated since undefined` gösterir ve deprecated tablosunda kural yoktur. #115 audit'inde 20 pozitif feed örneği incelendi; karar deprecation varsayımına değil, 10 km üzeri kümülatif mesafe, ardışık olmayan stop çiftleri ve zaman cascade'lerini birleştiren sinyalin karma/noisy olmasına dayanır. Bu notice'ın `STM_012`/`STM_014` ile eşlenmesi reddedildi; yeni kural eklenmedi ve fark bilinçli Analytics coverage gap olarak tutulur.

### Durak URL özgüllüğü

`STP_034` ve `STP_035`, `stop_url` değerini acente ve hat URL'leriyle güvenli bir sözdizimsel anahtarla karşılaştırır ve düşük öncelikli Quality bulguları üretir. Şema/host harf büyüklüğü, kök `/` ve HTTP 80/HTTPS 443 varsayılan port farkları eşdeğer sayılır; query, fragment, path sonundaki `/` ve percent-encoding farkları korunur. Aynı normalize URL'yi kullanan duraklar tek aggregate bulguda toplanır; ayrıntıda etkilenen durak sayısı ve örnek kimlikler bulunur.

### Hız Eşikleri

| Parametre | Varsayılan | Aralık | Açıklama |
|---|---:|---|---|
| Maks. Otobüs Hızı | 120 km/h | 60–200 | Otobüs seferleri için maksimum izin verilen hız |
| Maks. Tramvay Hızı | 100 km/h | 40–160 | Tramvay seferleri için maksimum izin verilen hız |
| Maks. Metro Hızı | 150 km/h | 80–250 | Metro seferleri için maksimum izin verilen hız |
| Maks. Demiryolu Hızı | 300 km/h | 100–400 | Demiryolu seferleri için maksimum izin verilen hız |
| Maks. Feribot Hızı | 80 km/h | 20–150 | Feribot seferleri için maksimum izin verilen hız |
| Maks. Teleferik Hızı | 30 km/h | 10–60 | Teleferik/füniküler için maksimum izin verilen hız |

### Coğrafi ve Aktarma Eşikleri

| Parametre | Varsayılan | Aralık | Açıklama |
|---|---:|---|---|
| Min. Aktarma Süresi | 180 sn | 30–1800 | Transferler için minimum bağlantı süresi |
| Maks. Aktarma Mesafesi | 500 m | 50–2000 | Transfer geçerli sayılmak için maksimum mesafe |
| Maks. Güzergah Sıçraması | 10 km | 1–50 | Art arda güzergah noktaları arasındaki maksimum mesafe |
| Çok Yakın Durak Eşiği | 5 m | 1–20 | Bu mesafeden yakın duraklar tekrar sayılır |
| Durağın Güzergaha Uzaklığı | 100 m | 20–500 | Durağın güzergahından en fazla bu kadar uzakta olabilir |
| Üst İstasyona Uzaklık Eşiği | 100 m | 10–1000 | Durak, üst istasyonundan en fazla bu kadar uzakta olabilir |

### Servis ve Operasyonel Eşikler

| Parametre | Varsayılan | Aralık | Açıklama |
|---|---:|---|---|
| Son Kullanma Uyarısı | 30 gün | 1–60 | Feed bu kadar günden az kalmışsa uyarı üretilir |
| Feed Bilgisi Son Kullanma Uyarısı | 7 gün | 1–60 | `FIN_019` için `feed_info.feed_end_date` bu pencere içinde bitiyorsa uyarı üretilir; varsayılan 7 gündür ve 30 günlük MobilityData paritesi `feed_info_expiry_warning_days=30` ile elde edilir |
| Servis Boşluğu Eşiği | 7 gün | 3–30 | Bu günden uzun servis kesintisi işaretlenir |
| Maks. Sefer Süresi | 24 saat | 8–72 | Tek bir seferin maksimum süresi |
| Min. Sefer Süresi | 60 sn | 10–300 | Tek bir seferin minimum süresi |
| Maks. Sefer Aralığı | 240 dk | 60–720 | Bu dakikadan uzun aralık uyarı üretir |
| Sıkışma Eşiği | 2 dk | 1–10 | Bu dakikadan kısa aralık sıkışma sayılır |

---

## Skorlar

### Yayın Skoru (0–100)

Feed'in resmi GTFS Schedule Reference'a göre yayınlanabilirlik durumunu ölçer. Skor **100'den başlar**; yayını engelleyen her sorun, kuralın ağırlığı ve düzeltme maliyetiyle orantılı bir ceza düşürür.

**Skor nasıl oluşur:**
- Yalnızca `Spec` sınıfındaki `Kritik` seviyeli sorunlar (resmi GTFS spec kapısı) Yayın Skorunu etkiler. `Interop` uyumluluk sinyalleri ayrı raporlanır (Interop Skoru / R8).
- Aynı kural birden fazla kez tetiklenirse ceza **en fazla 2 katıyla** sınırlıdır; tek bir sorunun tüm skoru sıfırlaması engellenir.
- **0–40:** Feed büyük olasılıkla tüketilemez. Blocker hatalar var.
- **40–70:** Kısmi sorunlar mevcut, bazı uygulamalar reddedebilir.
- **70–90:** Kullanılabilir, dikkat gerektiren noktalar var.
- **90–100:** Yayına hazır.

### Genel Skor (0–100)

Spec, Interop, Quality ve Analytics sınıflarının ağırlıklı ortalamasıdır (Spec×40% + Interop×30% + Quality×20% + Analytics×10%). Spesifikasyon uyumunun ötesinde operasyonel veri kalitesini de yansıtır.

**Skor nasıl oluşur:**
- Dört sınıfın tümündeki sorunlar bu skoru etkiler.
- `Spec` ve `Interop` sınıfları daha ağır; `Quality` ve `Analytics` veri kalitesi ve servis deseni boyutlarını temsil eder.
- **0–60:** Önemli kalite sorunları, yolcu deneyimi etkileniyor olabilir.
- **60–80:** Orta kalite, iyileştirme önerilir.
- **80–100:** İyi veri kalitesi.

> **Not:** Yayın Skoru ve Genel Skor farklı amaçlarla ve farklı formüllerle hesaplanır. Yayın Skoru yüksek ama Genel Skor düşük bir feed teknik olarak çalışır; ancak eksik erişilebilirlik bilgisi, hatalı güzergah isimleri gibi sorunlar yolcuları etkiler.

---

## Rapor Sekmeleri

### 1. Rapor
Genel özet: iki skor, feed metrikleri (hat sayısı, sefer sayısı, tarih aralığı vb.) ve sorun dağılım grafiği.

### 2. Ayrıntı ve Düzeltme
Bulunan sorunlar öncelik puanına göre sıralanmış bir düzeltme kuyruğu olarak sunulur. Her satır şu bilgileri içerir:

| Sütun | Açıklama |
|---|---|
| **Skor** | Öncelik puanı — `Ciddiyet × (1 + Bağımlı) × log₂(1 + Etkilenen) / Çaba` formülüyle hesaplanır; yüksek = önce düzelt |
| **+Yayın** | Bu kural düzeltilirse Yayın Skoru kaç puan artar |
| **+Skor** | Bu kural düzeltilirse Genel Skor kaç puan artar |
| **Bağımlı** | Bu kural giderilince kaç başka kural otomatik kapanır |
| **Çaba** | Düzeltme iş yükü: 1 = tek alan değişikliği, 2 = sınırlı çapraz-dosya, 3 = yapısal / veri modeli revizyonu |

Tüm satırların +Yayın toplamı `100 − mevcut Yayın Skoru`na, +Skor toplamı `100 − mevcut Genel Skor`a eşittir. Coğrafi sorunlarda harita ikonu görünür; tıklandığında sorunlu noktalar ve ilgili şekil/durak verileri interaktif haritada gösterilir. **Kural kodu**na tıklandığında ilgili GTFS spesifikasyon bölümü yeni sekmede açılır — bulgunun en çok etkilediği dosyanın referans sayfası (GTFS-JP kurallarında gtfs.jp).

### 3. Kategori Bazlı
Tüm kural ihlalleri grup ve sınıfa göre listelenir. Her satırda kural kodu, başlık, etkilenen kayıt sayısı, önem seviyesi ve düzeltme önerisi yer alır. Filtreleme ve sıralama desteklenir.

### 4. Dışa Aktar
Raporu HTML, CSV veya JSON olarak indirir. PDF seçeneği tarayıcının yazdırma diyaloğunu açar — "PDF olarak kaydet" ile kaydedebilirsiniz.

---

## İnteraktif GTFS Dosya Haritası

GTFS Validator & Analyzer, GTFS veri yapısını analiz edilen feed'in gerçek doğrulama bulgularıyla birleştiren interaktif bir Dosya Haritası içerir.

Bu görünüm statik bir şema değildir. Feed'de bulunan dosyaları, eksikleri, bulguları ve doğrulanmış dosya ilişkilerini analiz sonucuna göre gösterir.

### Özellikler

- Yedi çekirdek GTFS dosyasını **Takvim** ve **Ana Servis** gruplarında gösterir
- Çekirdek dışındaki standart dosyaları yalnızca analyzer bulgusu varsa gösterir
- Feed'de bulunan spec dışı dosyaları ayrı bir grupta listeler
- `route_id`, `trip_id`, `stop_id`, `service_id` ve `shape_id` gibi doğrulanmış GTFS ilişkilerini görselleştirir
- Dosyaları en yüksek bulgu önemine göre renklendirir
- Eksik, temiz ve sorunlu dosyaları birbirinden ayırır
- Satır sayısı, dosya boyutu, bulgu sayısı ve önem dağılımını gösterir
- Bulguları kurala göre ve **Kritik → Yüksek → Orta → Düşük → Bilgi** sırasıyla listeler
- Seçilen dosyanın tüm bulgularını filtrelenmiş Ayrıntı ve Düzeltme ekranında açar
- Dosya varlığı ve önem filtreleri sunar
- Yakınlaştırma, ekrana sığdırma, koyu tema ve mobil görünümü destekler

Bir dosya seçildiğinde yalnızca doğrulanmış ve ilgili GTFS bağlantıları açılır. Spec dışı dosyalar görünür tutulur ancak doğrulanmamış ilişkiler çizilmez.

Analiz ve görselleştirme tamamen tarayıcı içinde çalışır. GTFS dosyaları herhangi bir sunucuya yüklenmez.

![GTFS Dosya Haritası](docs/images/gtfs-file-map.png)

---

## Çalıştırmalar Arası Karşılaştırma

GTFS Validator & Analyzer, aynı feed'in iki analizini (önce/sonra) karşılaştırarak bir düzeltme turunun neyi iyileştirdiğini, neyi bozduğunu gösterir. Önceki analizden indirdiğiniz **Golden JSON**'u **Karşılaştır** sekmesinden yükleyin; karşılaştırma mevcut çalışmaya göre yapılır.

### Özellikler

- Yayın, Genel ve alt skorların (Spec, Interop, Quality, Analytics) önce/sonra değişimini gösterir
- Her kuralı **Düzeltilen, Azalan, Artan, Yeni, Aynı** olarak sınıflandırır; filtre ve arama sunar
- Önem (Kritik → Bilgi) ve sınıf (Spec/Interop/Quality/Analytics) dağılımındaki değişimi gösterir
- Feed yapısı değişimini (sefer, durak, `stop_times` ve `calendar_dates` satır sayıları) ve feed/servis tarih aralıklarını karşılaştırır
- Notice yoğunluğunu **1.000 sefer** ve **100.000 stop_time** başına normalize eder — böylece farklı boyuttaki feed'ler kıyaslanabilir
- İki çalışma feed adı, tarih aralığı veya yapılandırma (config) bakımından farklıysa uyarır; böylece yanıltıcı bir fark yanlış okunmaz
- Karşılaştırmayı CSV olarak dışa aktarır
- Eski Golden şemalarını (v1–v3) da okur

Karşılaştırma tamamen tarayıcı içinde çalışır. Golden JSON tarayıcıda çözümlenir; hiçbir veri sunucuya yüklenmez.

---

## Kural Sınıfları

| Sınıf | Ne ölçer | Hangi skoru etkiler |
|---|---|---|
| **Spec** | GTFS spesifikasyonuna aykırılık — zorunlu alan eksikliği, geçersiz değer, referans bütünlüğü hatası | Yayın |
| **Interop** | Spese uygun ama yaygın tüketicilerin (Google Maps, Apple Maps vb.) reddettiği veya yanlış yorumladığı durumlar | Yayın |
| **Quality** | İsteğe bağlı ama beklenen alanların eksikliği, tutarsızlıklar, en iyi pratikten sapmalar | Kalite |
| **Analytics** | Servis deseni analizi — sıkışıklık, seyrek sefer, süresi dolmuş servis | Kalite |

---

## Önem Seviyeleri

| Seviye | Anlamı |
|---|---|
| **Kritik** | Feed kullanılamaz hale getirir veya veri kaybına yol açar |
| **Yüksek** | Önemli işlevsellik sorunu, düzeltilmesi güçlü önerilir |
| **Orta** | Dikkat gerektiren tutarsızlık |
| **Düşük** | Küçük sapma, en iyi pratikten uzaklaşma |
| **Bilgi** | Bilgilendirme amaçlı, eylem gerekmeyebilir |

Önem seviyesi, [GTFS Schedule Referans Dokümantasyonu](https://gtfs.org/documentation/schedule/reference/#file-requirements)'ndaki requirement level (Required · Conditionally Required · Recommended · Optional) ile ihlalin semantic impact değerlendirmesinin birlikte sonucudur.

### Spec severity rubric'i

Spec kurallarında önem, requirement level + semantic impact birleşiminden; MobilityData'nın `ERROR/WARNING/INFO` etiketlerinden değil, ihlalin
GTFS verisini tüketilebilirlik üzerindeki etkisinden türetilir:

- **Kritik:** Required dosya/alan, primary-key veya foreign-key bütünlüğü ya da çekirdek tip/range ihlali; feed'in güvenilir biçimde tüketilmesini engeller ve `Spec + Kritik` yayın kapısıdır.
- **Yüksek:** Feed parse edilebilir kalsa bile sefer, ücret, erişilebilirlik veya Flex/pathway semantiğini maddi biçimde değiştiren doğrudan normatif ihlal.
- **Orta:** Etkisi sınırlı bir dosya, alan veya koşullu semantik ihlali; ana veri modeli okunabilir kalır.
- **Düşük:** Dar etkili, metadata/opsiyonel alan ölçeğinde normatif sapma; yayın kapısını etkilemez.
- **Bilgi:** Normatif Spec ihlali için kullanılmaz; yalnız ölçüm veya bağlam sinyalidir.

Bu değişmez nedeniyle `Spec` sınıfında `Bilgi` kural bulunamaz. 2026-08-09 audit'inde 307
Spec kuralı bu rubric ile yeniden incelendi; iki raw servis-günü kuralı (`STM_048` ve
`STM_049`) Bilgi'den Yüksek'e alındı. Ayrıntılı ID envanteri [`docs/audits/spec-severity-rubric-2026-08-09.md`](docs/audits/spec-severity-rubric-2026-08-09.md)'dedir.

GTFS-JP feed'leri için **JPN** grubu kuralları, resmî [GTFS-JP spesifikasyonu](https://www.gtfs.jp/) (gtfs.jp) esas alınarak belirlenir.

---

## Bulgu Sınırları

Büyük feed'lerde aynı kural binlerce satırda tetiklenebilir. Sınırsız bulgu listesi hem tarayıcı belleğini zorlar hem de okunabilirliği düşürür. Bu nedenle iki katmanlı bir sınır uygulanır:

| Sınır | Değer | Kapsam |
|---|---|---|
| Kural başına (varsayılan) | 500 | Tüm kurallar |
| Kural başına (yüksek) | 2.000 | `TRP_020`, `OPR_007`, `STP_016`, `STP_017` |
| Toplam (tüm kurallar) | 100.000 | Feed geneli — aşılırsa doğrulama durur |

Yüksek cap listesindeki kurallar gerçek feed'lerde doğal olarak yüksek sayılara ulaşır (örn. her sefer için bir headway kaydı). Sınıra çarpan kurallarda gerçek ihlal sayısı Düzeltme Kuyruğu'nun **Toplam** sütununda görünür; Tüm Bulgular sayfasında kural filtresi seçildiğinde ise sarı bir uyarı satırı gösterilir.

---

## Kural Grupları

Her kural `GRUP_NNN` formatında kodlanır. Gruplar GTFS dosya ve bileşen sınırlarını takip eder.

| Grup | GTFS Bileşeni | Açıklama |
|---|---|---|
| **ARC** | Arşiv / dosya seviyesi | ZIP açılması, dosya formatı, zorunlu dosya varlığı, karakter kodlaması |
| **AGN** | `agency.txt` | Acente bilgileri ve çoklu acente tutarlılığı |
| **CAL** | `calendar.txt` | Servis takvimleri ve haftalık gün desenleri |
| **CLD** | `calendar_dates.txt` | Servis istisna günleri ve tarih geçerliliği |
| **STP** | `stops.txt` | Durak konumları, hiyerarşi ve erişilebilirlik bilgileri |
| **RTS** | `routes.txt` | Hat tanımları, hat tipi, renk ve isimlendirme |
| **TRP** | `trips.txt` | Sefer tanımları, blok ve şekil ilişkileri |
| **STM** | `stop_times.txt` | Durak zamanlamaları, hız, sıra ve zamanlama tutarlılığı |
| **SHP** | `shapes.txt` | Güzergah şekilleri, nokta sırası ve durak hizalaması |
| **FRQ** | `frequencies.txt` | Frekans tabanlı seferler ve headway değerleri |
| **TRF** | `transfers.txt` | Aktarma tanımları, türleri ve süre geçerliliği |
| **FAR** | `fare_attributes.txt` | Ücret tanımları, para birimi ve ödeme yöntemi |
| **FRL** | `fare_rules.txt` | Hat ve bölge bazlı ücret kuralları |
| **FIN** | `feed_info.txt` | Feed yayıncı bilgisi, dil, geçerlilik tarihleri |
| **PTH** | `pathways.txt` | İstasyon içi yol ağı ve erişilebilirlik bağlantıları |
| **LVL** | `levels.txt` | İstasyon katları ve asansör/merdiven ilişkileri |
| **TRN** | `translations.txt` | Alan çevirileri ve dil tutarlılığı |
| **ATR** | `attributions.txt` | Veri kaynağı ve atıf bilgileri |
| **XFL** | Çapraz dosya | Dosyalar arası referans bütünlüğü ve tutarlılık |
| **GEO** | Coğrafi analiz | Koordinat tutarlılığı, outlier tespiti, kümeleme |
| **OPR** | Operasyonel analiz | Seferler arası bekleme süresi, hat yoğunluğu, durak tekrarı |
| **VAT** | Ağ topolojisi | İzole duraklar, bağlantısız güzergahlar, ağ erişilebilirliği |
| **DQ** | Feed geneli kalite | Genel veri kalitesi metrikleri ve eşik kontrolleri |
| **RCT** | `rider_categories.txt` | Yolcu kategorileri, yaş aralıkları ve varsayılan kategori (Fares v2) |
| **FMD** | `fare_media.txt` | Ödeme araçları: fiziksel kart, mobil uygulama, EMV vb. (Fares v2) |
| **FPD** | `fare_products.txt` | Ücret ürünleri, tutar, para birimi ve medya/kategori ilişkileri (Fares v2) |
| **FLG** | `fare_leg_rules.txt` | Yolculuk bacağı bazında ücret kuralları ve öncelik (Fares v2) |
| **FLJ** | `fare_leg_join_rules.txt` | Aktarmayla birleşen bacakları tek etkin ücret bacağı sayan eşleşme kuralları (Fares v2) |
| **FTR** | `fare_transfer_rules.txt` | Aktarma ücret kuralları ve süre limitleri (Fares v2) |
| **ARS** | `areas.txt` | Coğrafi alan tanımları (Fares v2) |
| **SAR** | `stop_areas.txt` | Durak–alan eşleştirmeleri (Fares v2) |
| **NET** | `networks.txt` | Ağ tanımları (Fares v2) |
| **TFR** | `timeframes.txt` | Zaman dilimi grupları ve servis takvimi ilişkileri (Fares v2) |
| **BKR** | `booking_rules.txt` | Talep odaklı rezervasyon kuralları, önceden bildirim süreleri ve rezervasyon türleri (GTFS Flex) |
| **PDW** | Esnek pencere kuralları | `stop_times.txt` içindeki talep odaklı alım/bırakma zaman penceresi tutarlılığı (GTFS Flex) |
| **LOC** | `locations.geojson` | Coğrafi esnek hizmet bölgelerinin geometri ve format doğrulaması (GTFS Flex) |
| **GGL** | Google Transit özgün | Google Maps ve Google Transit'in ek olarak zorunlu kıldığı ya da kısıtladığı kurallar |
| **JPN** | GTFS-JP profili | Japonya ulusal GTFS-JP profili kuralları — kana okuması, `office_jp.txt`/`agency_jp.txt` referans bütünlüğü (yalnız GTFS-JP feed'lerinde) |

---

## Geliştirici Kurulumu

### Gereksinimler

- **Rust** — GNU toolchain (`stable-x86_64-pc-windows-gnu`), MinGW gcc
- **wasm-pack** — WASM derleme aracı
- **Node.js** — bakımdaki bir LTS sürümü (kesin aralık: `ui/package.json` > `engines`)

> **Windows notu:** MSVC toolchain yerine GNU toolchain gereklidir. WASM build'i sırasında `wasm-opt` binary'si indirilmekte olup bu adım MSVC linker ile uyumsuz çalışmaktadır. MinGW `gcc` linker'ın PATH'te bulunması gerekir.

```powershell
# Rust GNU toolchain (bir kez)
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup override set stable-x86_64-pc-windows-gnu
```

### Build

```powershell
# 1. Bağımlılıkları kur
cd ui
npm install

# 2. WASM derle
npm run wasm

# 3. UI derle
npm run build
# Çıktı: ui/dist/
```

### Geliştirme Sunucusu

```powershell
cd ui
npm install
npm run dev
```

### Testler

```powershell
# Rust birim ve entegrasyon testleri
cargo test

# Tüm workspace crate, test ve example target'ları için warnings blocking lint
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Playwright smoke testleri
cd ui
npx playwright test
```

## Proje Yapısı

```
gtfs-validator/
├── crates/
│   ├── config/     # Yapılandırma tipleri
│   ├── core/       # Ortak veri yapıları ve sonuç modeli
│   ├── pipeline/   # Doğrulama pipeline'ı (k1–k7 aşamaları)
│   ├── rules/      # Kural tanımları ve registry (612 kural, 38 grup)
│   └── wasm/       # wasm-bindgen WASM çıktısı
├── spec-audit/     # Spec'ten üretilen alan tablosu (WP-2 çapa kapısı)
└── ui/             # Vite + TypeScript frontend
    ├── pkg/          # wasm-pack çıktısı (üretilen, commit'lenmiş)
    ├── src/
    │   └── pages/    # Uygulama sekmeleri (domain/fix/rules/export)
    └── tests/        # Playwright testleri
```

## Lisans

MIT — ayrıntılar için [LICENSE](LICENSE) dosyasına bakın.
