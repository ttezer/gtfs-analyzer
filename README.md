# GTFS Analyzer

GTFS (General Transit Feed Specification) dosyalarını tarayıcıda doğrulayan, tamamen istemci taraflı çalışan açık kaynak bir araç. Yüklenen zip dosyası sunucuya gönderilmez; tüm işlem WebAssembly ile kullanıcının tarayıcısında gerçekleşir.

Mevcut GTFS doğrulayıcıların çoğu yalnızca spesifikasyon uyumunu kontrol eder ve hata listesi çıkarır. GTFS Analyzer bunu çok adım öteye taşır: hangi dosyanın kaçıncı satırında ne sorun olduğunu gösterir, her sorun için adım adım düzeltme talimatı sunar ve coğrafi hataları (sapan güzergah, koordinat bozukluğu, erişilemeyen durak gibi) interaktif haritada işaretler. Her bulgu dosya ve bileşen bazlı bir kural koduyla (`ARC_`, `STP_`, `STM_`...), dört sınıftan biriyle (Spec · Interop · Quality · Analytics) ve beş önem seviyesinden biriyle (Kritik → Bilgi) etiketlenir; böylece binlerce bulgu arasında filtreleme, önceliklendirme ve otomasyon kolaylaşır. Feed'in hangi GTFS özelliklerini kullandığı (Shapes, Transfers, Fares, Headsigns vb.) otomatik tespit edilir ve rapora yansıtılır.

Spesifikasyon uyumunun ötesinde operasyonel kaliteyi de ölçer: hat bazında sefer sıklığı tutarsızlıkları, anormal hız segmentleri, izole duraklar, servis desenlerindeki boşluklar ve ağ topolojisi sorunları 360 kuralla analiz edilir. Sonuçlar iki bağımsız skorla özetlenir; düzeltme kuyruğu "önce ne düzeltilmeli?" sorusunu otomatik olarak yanıtlar ve her düzeltmenin skora katkısını gösterir.

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
| Düzeltme önerisi | ❌ | ❌ | ❌ | ✅ |
| Çıktı formatı | HTML, JSON | Web (kalıcı link) | HTML, JSON | HTML, CSV, JSON, PDF |
| Platform | Web | Web | Web, CLI, Desktop | Web *(CLI, Desktop planlanmış)* |
| **Toplam kural** | **~120** | **~80** | **~120** | **360** |

### BART GTFS Feed Örneği

BART (Bay Area Rapid Transit, San Francisco) feed'i dört validator ile test edildi.  
Feed: `BART (San Francisco).zip` — 2026-05-25 tarihinde indirilen sürüm (feed geçerlilik aralığı: 2026-01-12–2026-08-07).  
Kullanılan sürümler: MobilityData gtfs-validator v7.x · France Transport (transport.data.gouv.fr, Mayıs 2026) · GTFS Guru v0.1.0 · GTFS Analyzer v0.1.1.

| | MobilityData | France Transport | GTFS Guru | GTFS Analyzer |
|---|---:|---:|---:|---:|
| Toplam notice | 2.725 | 6 ⚠️ | 2.663 | 2.701 |
| Kritik / Error | 2 | 1 | 1 | 0 † |
| Yüksek / Warning | 2.655 | 0 | 2.655 | 1.148 |
| Orta | — | — | — | 554 |
| Düşük | — | — | — | 500 |
| Bilgi / Info | 68 | 5 | 7 | 499 |
| Tetiklenen kural tipi | 13 | 2 | 13 | **44** |
| Yayın skoru | — | — | — | **84,7 / 100** |
| Kalite skoru | — | — | — | **83,1 / 100** |

> ⚠️ France Transport, `rider_category_name` eksik alanı nedeniyle validasyonu tamamlayamadı.  
> † GTFS Analyzer standart GTFS dosyalarında kritik ihlal bulmadı. Diğer araçların hataları `rider_categories.txt` içindir; bu dosya GTFS spec'inde tanımlı değildir.

---

## Kullanım

GTFS Analyzer bir web uygulamasıdır; kurulum gerektirmez. Canlı sürümü tarayıcıda açıp GTFS zip dosyanızı yükleyin.

**→ [https://ttezer.github.io/gtfs-analyzer/](https://ttezer.github.io/gtfs-analyzer/)**

1. GTFS zip dosyanızı sürükleyip bırakın ya da dosya seçiciyle yükleyin.
2. Doğrulama otomatik başlar; ilerleme ekranda aşama aşama gösterilir.
3. Tamamlandığında Yayın ve Kalite skorları ile dört sekme görünür: **Rapor**, **Ayrıntı ve Düzeltme**, **Kategori Bazlı**, **Dışa Aktar**.

> Kendi sunucunuzda barındırmak veya geliştirme ortamı kurmak için [Geliştirici Kurulumu](#geliştirici-kurulumu) bölümüne bakın.

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

---

## Bulgu Sınırları

Büyük feed'lerde aynı kural binlerce satırda tetiklenebilir. Sınırsız bulgu listesi hem tarayıcı belleğini zorlar hem de okunabilirliği düşürür. Bu nedenle iki katmanlı bir sınır uygulanır:

| Sınır | Değer | Kapsam |
|---|---|---|
| Kural başına (varsayılan) | 500 | Tüm kurallar |
| Kural başına (yüksek) | 2.000 | `TRP_020`, `OPR_007`, `STP_016`, `STP_017` |
| Toplam (tüm kurallar) | 100.000 | Feed geneli — aşılırsa doğrulama durur |

Yüksek cap listesindeki kurallar gerçek feed'lerde doğal olarak yüksek sayılara ulaşır (örn. her sefer için bir headway kaydı). Raporda bir kuralın sınıra ulaştığı durumlarda gösterilen sayı gerçek ihlal sayısını yansıtmaz.

---

## Kural Grupları

Her kural `GRUP_NNN` formatında kodlanır. Gruplar GTFS dosya ve bileşen sınırlarını takip eder.

| Grup | GTFS Bileşeni | Açıklama |
|---|---|---|
| **ARC** | Arşiv / dosya seviyesi | ZIP açılması, dosya formatı, zorunlu dosya varlığı |
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
│   ├── rules/      # Kural tanımları ve registry (360 kural)
│   └── wasm/       # wasm-bindgen WASM çıktısı
└── ui/             # Vite + TypeScript frontend
    ├── pkg/          # wasm-pack çıktısı (üretilen, commit'lenmiş)
    ├── src/
    │   └── pages/    # Uygulama sekmeleri (domain/fix/rules/export)
    └── tests/        # Playwright testleri
```

## Lisans

MIT — ayrıntılar için [LICENSE](LICENSE) dosyasına bakın.
