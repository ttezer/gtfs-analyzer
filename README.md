# GTFS Analyzer

→ [English version](README.en.md)

GTFS (General Transit Feed Specification) dosyalarını tarayıcıda doğrulayan, tamamen istemci taraflı çalışan açık kaynak bir araç. Yüklenen zip dosyası sunucuya gönderilmez; tüm işlem WebAssembly ile kullanıcının tarayıcısında gerçekleşir.

Mevcut GTFS doğrulayıcıların çoğu yalnızca spesifikasyon uyumunu kontrol eder ve hata listesi çıkarır. GTFS Analyzer bunu çok adım öteye taşır: hangi dosyanın kaçıncı satırında ne sorun olduğunu gösterir, her sorun için adım adım düzeltme talimatı sunar ve coğrafi hataları (sapan güzergah, koordinat bozukluğu, erişilemeyen durak gibi) interaktif haritada işaretler. Her bulgu dosya ve bileşen bazlı bir kural koduyla (`ARC_`, `STP_`, `STM_`...), dört sınıftan biriyle (Spec · Interop · Quality · Analytics) ve beş önem seviyesinden biriyle (Kritik → Bilgi) etiketlenir; böylece binlerce bulgu arasında filtreleme, önceliklendirme ve otomasyon kolaylaşır. Feed'in hangi GTFS özelliklerini kullandığı (Shapes, Transfers, Fares, Headsigns, Flex vb.) otomatik tespit edilir ve rapora yansıtılır.

Spesifikasyon uyumunun ötesinde operasyonel kaliteyi de ölçer: hat bazında sefer sıklığı tutarsızlıkları, anormal hız segmentleri, izole duraklar, servis desenlerindeki boşluklar ve ağ topolojisi sorunları 448 kuralla analiz edilir. Sonuçlar iki bağımsız skorla özetlenir; düzeltme kuyruğu "önce ne düzeltilmeli?" sorusunu otomatik olarak yanıtlar ve her düzeltmenin skora katkısını gösterir.

**Kimler kullanır:**
- Toplu taşıma işletmecileri ve belediyeler — feed yayına almadan önce
- GTFS entegratörleri ve danışmanlar — teslim kalitesini doğrulamak için
- Uygulama geliştiriciler — tükettikleri feed'in güvenilirliğini ölçmek için
- Araştırmacılar ve analistler — ağ kalitesini karşılaştırmak için

---

## Diğer Araçlarla Karşılaştırma

### Özellikler

| Özellik | MobilityData | France Transport | GTFS Guru | GTFS Analyzer |
|---|:---:|:---:|:---:|:---:|
| Web arayüzü | ✅ | ✅ | ✅ | ✅ |
| Veri sunucuya gitmiyor | ❌ | ❌ | ✅ | ✅ |
| Spec uyum kuralları | ✅ | ✅ | ✅ | ✅ |
| Kalite kuralları | ❌ | Kısmi | ❌ | ✅ |
| Operasyonel analitik | ❌ | ❌ | ❌ | ✅ |
| Harita görselleştirme | ❌ | Durak | ❌ | Durak, güzergah, sefer, hat, pathway |
| Feed skoru | ❌ | ❌ | ❌ | ✅ |
| Düzeltme önerisi | Kısmi | ❌ | ❌ | ✅ |
| GTFS Flex desteği | Kısmi | ❌ | ❌ | ✅ |
| Çıktı formatı | HTML, JSON | Web (kalıcı link) | HTML, JSON | HTML, CSV, JSON, PDF |
| Platform | Web | Web | Web, CLI, Desktop | Web *(CLI, Desktop planlanmış)* |
| **Toplam kural** | **178** | **~80** | **~120** | **448** |

### BART GTFS Feed Örneği

BART (Bay Area Rapid Transit, San Francisco) feed'i dört validator ile test edildi.  
Feed: `BART (San Francisco).zip` — 2026-05-25 tarihinde indirilen sürüm (feed geçerlilik aralığı: 2026-01-12–2026-08-07).  
Kullanılan sürümler: MobilityData gtfs-validator v8.0.1 · France Transport (transport.data.gouv.fr, Mayıs 2026) · GTFS Guru v0.1.0 · GTFS Analyzer v0.1.2.

| | MobilityData | France Transport | GTFS Guru | GTFS Analyzer |
|---|---:|---:|---:|---:|
| Toplam notice | 2.725 | 6 ⚠️ | 2.663 | 6.684 |
| Kritik / Error | 2 | 1 | 1 | 3 |
| Yüksek / Warning | 2.655 | 0 | 2.655 | 5.128 |
| Orta | — | — | — | 554 |
| Düşük | — | — | — | 500 |
| Bilgi / Info | 68 | 5 | 7 | 499 |
| Tetiklenen kural tipi | 13 | 2 | 13 | **45** |
| Yayın skoru | — | — | — | **79,4 / 100** |
| Kalite skoru | — | — | — | **80,1 / 100** |

> ⚠️ France Transport, `rider_category_name` eksik alanı nedeniyle validasyonu tamamlayamadı.

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
│   ├── rules/      # Kural tanımları ve registry (448 kural, 36 grup)
│   └── wasm/       # wasm-bindgen WASM çıktısı
└── ui/             # Vite + TypeScript frontend
    ├── pkg/          # wasm-pack çıktısı (üretilen, commit'lenmiş)
    ├── src/
    │   └── pages/    # Uygulama sekmeleri (domain/fix/rules/export)
    └── tests/        # Playwright testleri
```

## Lisans

MIT — ayrıntılar için [LICENSE](LICENSE) dosyasına bakın.
