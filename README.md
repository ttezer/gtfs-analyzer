# GTFS Analyzer

🇹🇷 **Türkçe** · 🇬🇧 [English](README.en.md) · 🇯🇵 [日本語](README.ja.md)

[![Uygulamayı Aç](https://img.shields.io/badge/Uygulamay%C4%B1%20A%C3%A7-gtfs--analyzer-2ea44f?style=flat&logo=googlechrome&logoColor=white)](https://ttezer.github.io/gtfs-analyzer/)
![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)
![WebAssembly](https://img.shields.io/badge/WebAssembly-654FF0?style=flat&logo=webassembly&logoColor=white)
![TypeScript](https://img.shields.io/badge/TypeScript-3178C6?style=flat&logo=typescript&logoColor=white)
![Vite](https://img.shields.io/badge/Vite-646CFF?style=flat&logo=vite&logoColor=white)
![Gizlilik](https://img.shields.io/badge/%25100-istemci%20tarafl%C4%B1-2ea44f?style=flat&logo=shield&logoColor=white)
![Kural sayısı](https://img.shields.io/badge/477-kural-blue?style=flat)
![Diller](https://img.shields.io/badge/diller-TR%20%C2%B7%20EN%20%C2%B7%20JA-informational?style=flat)
[![Lisans MIT](https://img.shields.io/badge/lisans-MIT-yellow?style=flat)](LICENSE)

GTFS Analyzer, GTFS dosyalarını doğrudan tarayıcıda doğrulayan ve analiz eden açık kaynak bir araçtır. Yüklenen .zip dosyası hiçbir sunucuya gönderilmez; tüm işlemler WebAssembly ile kullanıcının cihazında gerçekleştirilir.

GTFS Analyzer yalnızca dosyanın spesifikasyona uygun olup olmadığını kontrol etmez; feed'in ne kadar güvenilir, tutarlı ve kullanılabilir olduğunu da analiz eder. Hataları ilgili dosya ve satır numarasıyla birlikte gösterir, her bulgu için düzeltme adımları sunar ve coğrafi sorunları — örneğin sapan güzergâhlar, bozuk koordinatlar veya erişilemeyen duraklar — interaktif harita üzerinde işaretler.

Her bulgu; kural kodu, analiz sınıfı ve önem seviyesiyle etiketlenir. Spec · Interop · Quality · Analytics sınıfları ile Kritik → Bilgi önem seviyeleri sayesinde binlerce bulgu filtrelenebilir, önceliklendirilebilir ve sistematik biçimde ele alınabilir. Araç ayrıca feed'in kullandığı GTFS özelliklerini — Shapes, Transfers, Fares, Headsigns, Flex ve benzerlerini — otomatik olarak tespit ederek rapora dahil eder.

GTFS Analyzer, spesifikasyon doğrulamasını operasyonel kalite analiziyle genişletir. Hat bazında sefer sıklığı tutarsızlıkları, anormal hız segmentleri, izole duraklar,servis desenlerindeki boşluklar ve ağ topolojisi problemleri 477 farklı doğrulama ve analiz kuralıyla incelenir. Sonuçlar, uyumluluk ve kaliteyi ayrı ayrı değerlendiren skorlarla özetlenir. Önceliklendirilmiş düzeltme kuyruğu ise hangi sorunların önce ele alınması gerektiğini ve yapılacak düzeltmelerin skora olası etkisini gösterir.

**Kimler için?**

- **Toplu taşıma işletmecileri ve belediyeler** — Feed'i yayına almadan önce doğrulamak ve kalite sorunlarını gidermek için.
- **GTFS entegratörleri ve danışmanlar** — Teslim edilen verinin teknik ve operasyonel kalitesini belgelemek için.
- **Uygulama geliştiriciler** — Kullandıkları feed'in güvenilirliğini ve entegrasyon risklerini değerlendirmek için.
- **Araştırmacılar ve analistler** — Farklı toplu taşıma ağlarını veri kalitesi ve yapı bakımından karşılaştırmak için.

---

## Diğer Araçlarla Karşılaştırma

### Özellikler

| Özellik | MobilityData | GTFS Guru | GTFS Analyzer |
|---|:---:|:---:|:---:|
| Web arayüzü | ✅ | ✅ | ✅ |
| Veri sunucuya gitmiyor | ❌ | ✅ | ✅ |
| Spec uyum kuralları | ✅ | ✅ | ✅ |
| Kalite kuralları | ❌ | ❌ | ✅ |
| Operasyonel analitik | ❌ | ❌ | ✅ |
| Harita görselleştirme | ❌ | ❌ | Durak, güzergah, sefer, hat, pathway |
| Feed skoru | ❌ | ❌ | ✅ |
| Düzeltme önerisi | Kısmi | ❌ | ✅ |
| GTFS Flex desteği | Kısmi | ❌ | ✅ |
| Fares v2 doğrulama | ❌ | ❌ | ✅ |
| GTFS-JP profil doğrulama | ❌ | ❌ | ✅ |
| Çıktı formatı | HTML, JSON | HTML, JSON | HTML, CSV, JSON, PDF |
| Platform | Web | Web, CLI, Desktop | Web *(CLI, Desktop planlanmış)* |
| **Toplam kural** | **178** | **~120** | **477** |

### Feed Analizi Örnekleri

Aynı feed'ler üç validator ile karşılaştırıldı: MobilityData gtfs-validator v8.0.1 · GTFS Guru v0.1.0 · GTFS Analyzer v0.1.2. (GTFS Analyzer sayıları 2026-06-06 tarihli analizin anlık görüntüsüdür; tarihe bağlı kurallar nedeniyle farklı bir günde çalıştırma küçük sapmalar verebilir.)

#### BART (Bay Area Rapid Transit, San Francisco)

Feed: `BART (San Francisco).zip` — 2026-05-25'te indirilen sürüm (geçerlilik aralığı: 2026-01-12–2026-08-07) · 14 hat, 287 durak, 4.455 sefer.

| | MobilityData | GTFS Guru | GTFS Analyzer |
|---|---:|---:|---:|
| Toplam notice | 2.725 | 2.663 | 3.255 |
| Kritik / Error | 2 | 1 | 3 |
| Yüksek / Warning | 2.655 | 2.655 | 150 |
| Orta | — | — | 1.060 |
| Düşük | — | — | 1.013 |
| Bilgi / Info | 68 | 7 | 1.029 |
| Tetiklenen kural tipi | 13 | 10 | **50** |
| Yayın skoru | — | — | **87,7 / 100** |
| Kalite skoru | — | — | **80,7 / 100** |

#### TriMet (Portland, Oregon)

Feed: `trimet.zip` — 2026-06-04'te indirilen sürüm (geçerlilik aralığı: 2026-04-26–2026-08-22) · 89 hat, 6.395 durak, 48.146 sefer.

| | MobilityData | GTFS Guru | GTFS Analyzer |
|---|---:|---:|---:|
| Toplam notice | 48 | 116 | 5.084 |
| Kritik / Error | 0 | 0 | 36 |
| Yüksek / Warning | 39 | 107 | 1.655 |
| Orta | — | — | 1.382 |
| Düşük | — | — | 1.019 |
| Bilgi / Info | 9 | 9 | 992 |
| Tetiklenen kural tipi | 7 | 7 | **58** |
| Yayın skoru | — | — | **80,6 / 100** |
| Kalite skoru | — | — | **73,7 / 100** |

> ⚠️ **Fares v2:** TriMet'teki 36 kritik bulgunun tamamı GTFS Fares v2 dosyalarından gelir — 29× tekrarlanan `fare_product_id` (`fare_products.txt`) ve 7× `fare_leg_rules.txt` içinde `networks.txt`'te tanımsız `network_id`. MobilityData ve GTFS Guru bu dosyaları doğrulamadığı için aynı bozuk referansları 0 kritik olarak raporlar; tek başına GTFS Analyzer Fares v2 bütünlüğünü denetler.

#### Tokyo Toei (Tokyo Metropolitan Bureau of Transportation)

Feed: `tokyo_toei_bus.zip` — feed_version 2026-06-06 (geçerlilik aralığı: 2026-06-06–2029-06-05) · 150 hat, 5.367 durak, 54.315 sefer.

| | MobilityData | GTFS Guru | GTFS Analyzer |
|---|---:|---:|---:|
| Toplam notice | 1.637 | 4.137 | 9.335 |
| Kritik / Error | 0 | 0 | 0 |
| Yüksek / Warning | 265 | 4.128 | 1.061 |
| Orta | — | — | 3.325 |
| Düşük | — | — | 3.961 |
| Bilgi / Info | 1.372 | 9 | 988 |
| Tetiklenen kural tipi | 9 | 6 | **59** |
| Yayın skoru | — | — | **94,3 / 100** |
| Kalite skoru | — | — | **72,3 / 100** |

> 🗾 **Spec-temiz ama operasyonel olarak yoğun:** Üç araç da 0 kritik bulur — feed spec açısından temiz. Fark analitik katmanda: GTFS Analyzer'ın orta/düşük bulgularının çoğu 3 yıllık geçerlilik penceresi (2026–2029) ve yoğun şebeke/şekil desenlerinden gelen operasyonel sinyallerdir; MobilityData ve GTFS Guru bu feed'i ağırlıkla uyarı/bilgi olarak özetler.

---

## GTFS-JP Desteği

GTFS Analyzer, Japonya'nın ulusal GTFS profili **GTFS-JP**'yi (国土交通省 / MLIT standardı) otomatik olarak tanır ve standart GTFS'in isteğe bağlı bıraktığı, GTFS-JP'nin zorunlu kıldığı kuralları uygular. MLIT, sübvansiyon alan işletmecilerden GTFS-JP yayımlamasını şart koştuğu için yüzlerce küçük operatör bu profile uymak zorundadır; ancak yaygın doğrulayıcılar profile özgü zorunlulukları denetlemez.

**Otomatik tespit.** Bir feed; `*_jp.txt` dosyaları (`agency_jp.txt`, `office_jp.txt`, `routes_jp.txt`) içeriyorsa, `feed_lang` değeri `ja` ile başlıyorsa ya da `translations.txt` içinde kana (`ja-Hrkt`) okumaları taşıyorsa GTFS-JP olarak işaretlenir ve raporda **GTFS-JP** rozeti görünür. Profil kuralları yalnızca bu feed'lerde devreye girer; standart feed'lerde sessiz kalır.

**Profil kuralları (JPN grubu).**

| Kural | Denetim |
|---|---|
| **JPN_001** | Durak adlarının kana (よみがな — `translations.txt`, `ja-Hrkt`) okuması; sesli anons ve arama için GTFS-JP'de zorunludur |
| **JPN_002** | `trips.jp_office_id` değerinin `office_jp.txt`'teki bir `office_id` ile eşleşmesi (işletme ofisi referans bütünlüğü) |
| **JPN_003** | `agency_jp.txt` `agency_id` değerinin `agency.txt`'te tanımlı olması (işletici referans bütünlüğü) |

Yukarıdaki **Tokyo Toei** karşılaştırması bu profilin gerçek bir GTFS-JP feed'inde nasıl davrandığını gösterir: feed spec açısından temizdir (0 kritik) ve profil kuralları doğru referanslı veride yanlış pozitif üretmez.

---

## Kullanım

GTFS Analyzer bir web uygulamasıdır; kurulum gerektirmez. Canlı sürümü tarayıcıda açıp GTFS zip dosyanızı yükleyin.

**→ [https://ttezer.github.io/gtfs-analyzer/](https://ttezer.github.io/gtfs-analyzer/)**

1. GTFS zip dosyanızı sürükleyip bırakın ya da dosya seçiciyle yükleyin.
2. Doğrulama otomatik başlar; ilerleme ekranda aşama aşama gösterilir.
3. Tamamlandığında Yayın ve Kalite skorları ile dört sekme görünür: **Rapor**, **Ayrıntı ve Düzeltme**, **Kategori Bazlı**, **Dışa Aktar**.

> Kendi sunucunuzda barındırmak veya geliştirme ortamı kurmak için [Geliştirici Kurulumu](#geliştirici-kurulumu) bölümüne bakın.

---

## Analiz Kriterleri

Yükleme ekranındaki **Analiz Kriterleri** bölümünden doğrulama eşikleri özelleştirilebilir. Değiştirilen alanlar bir sonraki ZIP yüklemesinde uygulanır; sıfırla butonu varsayılanlara döndürür.

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
| Servis Boşluğu Eşiği | 7 gün | 3–30 | Bu günden uzun servis kesintisi işaretlenir |
| Maks. Sefer Süresi | 24 saat | 8–72 | Tek bir seferin maksimum süresi |
| Min. Sefer Süresi | 60 sn | 10–300 | Tek bir seferin minimum süresi |
| Maks. Sefer Aralığı | 240 dk | 60–720 | Bu dakikadan uzun aralık uyarı üretir |
| Sıkışma Eşiği | 2 dk | 1–10 | Bu dakikadan kısa aralık sıkışma sayılır |

---

## Skorlar

### Yayın Skoru (0–100)

Feed'in toplu taşıma uygulamaları tarafından tüketilebilirlik durumunu ölçer. Skor **100'den başlar**; bulunan her blocker sorun, kuralın ağırlığı ve düzeltme maliyetiyle orantılı bir ceza düşürür.

**Skor nasıl oluşur:**
- Yalnızca `Spec` ve `Interop` sınıfındaki `Kritik` ve `Yüksek` seviyeli sorunlar Yayın Skorunu etkiler.
- Aynı kural birden fazla kez tetiklenirse ceza **en fazla 2 katıyla** sınırlıdır; tek bir sorunun tüm skoru sıfırlaması engellenir.
- **0–40:** Feed büyük olasılıkla tüketilemez. Blocker hatalar var.
- **40–70:** Kısmi sorunlar mevcut, bazı uygulamalar reddedebilir.
- **70–90:** Kullanılabilir, dikkat gerektiren noktalar var.
- **90–100:** Yayına hazır.

### Kalite Skoru (0–100)

Spesifikasyon uyumunun ötesinde veri kalitesini ve en iyi pratiklere uyumu ölçer. Feed yayına girebilir olsa bile Kalite Skoru düşük olabilir.

**Skor nasıl oluşur:**
- `Quality` ve `Analytics` sınıfındaki sorunlar bu skoru etkiler.
- Eksik isteğe bağlı alanlar, tutarsız servis desenleri, erişilebilirlik eksiklikleri bu skora yansır.
- **0–60:** Önemli kalite sorunları, yolcu deneyimi etkileniyor olabilir.
- **60–80:** Orta kalite, iyileştirme önerilir.
- **80–100:** İyi veri kalitesi.

> **Not:** İki skor birbirinden bağımsızdır. Yayın Skoru yüksek ama Kalite Skoru düşük bir feed teknik olarak çalışır; ancak eksik erişilebilirlik bilgisi, hatalı güzergah isimleri gibi sorunlar yolcuları etkiler.

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
| **+Kalite** | Bu kural düzeltilirse Kalite Skoru kaç puan artar |
| **Bağımlı** | Bu kural giderilince kaç başka kural otomatik kapanır |
| **Çaba** | Düzeltme iş yükü: 1 = tek alan değişikliği, 5 = veri modelinde kapsamlı revizyon |

Tüm satırların +Yayın toplamı `100 − mevcut Yayın Skoru`na, +Kalite toplamı `100 − mevcut Kalite Skoru`na eşittir. Coğrafi sorunlarda harita ikonu görünür; tıklandığında sorunlu noktalar ve ilgili şekil/durak verileri interaktif haritada gösterilir.

### 3. Kategori Bazlı
Tüm kural ihlalleri grup ve sınıfa göre listelenir. Her satırda kural kodu, başlık, etkilenen kayıt sayısı, önem seviyesi ve düzeltme önerisi yer alır. Filtreleme ve sıralama desteklenir.

### 4. Dışa Aktar
Raporu HTML, CSV veya JSON olarak indirir. PDF seçeneği tarayıcının yazdırma diyaloğunu açar — "PDF olarak kaydet" ile kaydedebilirsiniz.

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

Önem seviyeleri, [GTFS Schedule Referans Dokümantasyonu](https://gtfs.org/documentation/schedule/reference/#file-requirements)'nda tanımlanan dosya ve alan zorunluluk seviyeleri (Required · Conditionally Required · Recommended · Optional) esas alınarak belirlenmiştir.

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
- **Node.js** 18+

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
│   ├── rules/      # Kural tanımları ve registry (477 kural, 37 grup)
│   └── wasm/       # wasm-bindgen WASM çıktısı
└── ui/             # Vite + TypeScript frontend
    ├── pkg/          # wasm-pack çıktısı (üretilen, commit'lenmiş)
    ├── src/
    │   └── pages/    # Uygulama sekmeleri (domain/fix/rules/export)
    └── tests/        # Playwright testleri
```

## Lisans

MIT — ayrıntılar için [LICENSE](LICENSE) dosyasına bakın.
