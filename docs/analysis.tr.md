# Analiz Kriterleri, Skorlar ve Bulgu Sınırları

🇹🇷 **Türkçe** · [README'ye dön](../README.md)

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

Tek noktadan oluşan ve gerçekten bir trip tarafından kullanılan shape `SHP_006` ile Düşük · Quality olarak raporlanır; ayrıntıda `shape_id` ve `shape_point_count=1` bulunur. İki noktalı düz segment geçerlidir. Kullanılmayan tek noktalı shape yalnız `SHP_018` ile raporlanır.

### Durak URL özgüllüğü

`STP_034` ve `STP_035`, `stop_url` değerini acente ve hat URL'leriyle güvenli bir sözdizimsel anahtarla karşılaştırır ve düşük öncelikli Quality bulguları üretir. Şema/host harf büyüklüğü, kök `/` ve HTTP 80/HTTPS 443 varsayılan port farkları eşdeğer sayılır; query, fragment, path sonundaki `/` ve percent-encoding farkları korunur. Aynı normalize URL'yi kullanan duraklar tek aggregate bulguda toplanır; ayrıntıda etkilenen durak sayısı ve örnek kimlikler bulunur.

### Eşikler

Kuralların bir kısmı ayarlanabilir eşiklerle çalışır: hat tipine göre en yüksek hız (otobüs 120 km/h, tramvay 100, metro 150, demiryolu 300, feribot 80, teleferik 30), aktarma süresi ve mesafesi, güzergâh sıçraması, çok yakın durak (5 m), durağın güzergâha ve üst istasyona uzaklığı, servis boşluğu, sefer süresi, sefer aralığı ve feed'in son kullanma uyarısı.

Eşikler web uygulamasında **Analiz Kriterleri** panelinden, CLI'da `--config config.json` ile, SDK ve Python paketinde `config` nesnesiyle değiştirilir. Tüm anahtarlar, varsayılanlar ve birimler: [yapılandırma başvurusu](../sdk/README.md#configuration-reference).

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

## Bulgu Sınırları

Büyük feed'lerde aynı kural binlerce satırda tetiklenebilir. Sınırsız bulgu listesi hem tarayıcı belleğini zorlar hem de okunabilirliği düşürür. Bu nedenle iki katmanlı bir sınır uygulanır:

| Sınır | Değer | Kapsam |
|---|---|---|
| Kural başına (varsayılan) | 500 | Tüm kurallar |
| Kural başına (yüksek) | 2.000 | `TRP_020`, `OPR_007`, `STP_016`, `STP_017` |
| Toplam (tüm kurallar) | — | Feed-geneli 100.000 durdurma sınırı yok |

Bu 500/2.000 sınırları yalnızca tarayıcı/WASM görünümündeki performans örnekleridir; feed-geneli
100.000'e ulaşınca doğrulamayı durduran bir kod yolu yoktur. CLI varsayılan olarak sınırsız
notice üretir; `--json --max-notices-per-rule N` yalnız JSON çıktısında görünür bulguları sınırlar,
R1/R5/R9 hesaplarını sınırlamaz.
