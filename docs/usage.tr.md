# Sekiz Yolu Kullanmak

🇹🇷 **Türkçe** · [README'ye dön](../README.md)

GTFS Validator & Analyzer'ın 621 kurallı motoruna sekiz yoldan erişilir. Bu sayfa her yolun kurulumunu ve kullanımını anlatır.

## 1. Tarayıcı

GTFS Validator & Analyzer bir web uygulamasıdır; kurulum gerektirmez. Canlı sürümü tarayıcıda açıp GTFS zip dosyanızı yükleyin.

Motor tarayıcı yeteneğine göre otomatik seçilir: Memory64 destekleniyorsa **WASM64**,
desteklenmiyorsa **WASM32** kullanılır; sabit bir feed boyutu sınırı yoktur. Aktif motor yükleme
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

> Kendi sunucunuzda barındırmak veya geliştirme ortamı kurmak için [Geliştirici Kurulumu](../README.md#geliştirici-kurulumu) bölümüne bakın.

## 2. CLI

Aynı doğrulama çekirdeğini terminalden çalıştırabilirsiniz — toplu iş, betikleme ve Python/otomasyon entegrasyonu için.

### Kurulum

Rust kuruluysa en kısa yol:

```bash
cargo install gtfs-analyzer
gtfs-analyzer validate feed.zip
```

Rust kurmadan: [Releases](https://github.com/ttezer/gtfs-analyzer/releases) sayfasından platformunuza uygun arşivi indirin (`x86_64-linux`, `aarch64-macos`, `x86_64-macos`, `x86_64-windows`), açın ve `gtfs-analyzer` binary'sini `PATH`'inize koyun.

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
| `--compact-json` | Makine tüketicileri için kanonik kısa JSON görünümü üretir; `--samples N` ile R9 kök neden örnekleri eklenebilir |
| `--summary` | Kısa özet: durum, notice sayısı, skorlar (varsayılan; `--json` ile birlikte kullanılamaz) |
| `--rule SHP_010` | Yalnızca verilen kural için notice'lar |
| `--severity critical` | Tam olarak bu öneme sahip notice'lar (critical/high/medium/low/info) |
| `--min-severity high` | Bu önem ve daha ağırı (critical en ağır) |
| `--class spec` | Yalnızca bu kural sınıfları — `spec,interop,quality,analytics`, virgülle çoklu |
| `--disable-rule DQ_004` | Belirtilen kuralları sonuçtan, skorlardan ve R9 kuyruğundan çıkarır; virgülle çoklu |
| `--disable-rule-gtfs-jp DQ_004` | Yalnız GTFS-JP olarak tespit edilen feed'lerde bu kuralları çıkarır |
| `--fail-on critical` | Exit 1 **yalnızca** bu önem ve daha ağırı varsa |
| `--fail-on-class spec` | Exit 1 yalnızca bu sınıflarda notice varsa |
| `--pretty` | JSON'ı girintili yazar (`--json` gerektirir) |
| `--include-name-index` | `name_index`'i (durak/hat/shape arama tabloları) JSON'a dahil eder |
| `--max-notices-per-rule 20` | JSON'da kural başına en fazla N bulgu tutar; kesilen kuralların gerçek sayısı `capped_totals`'ta, R1/R5/R9 tüm bulgulardan hesaplanır (`--json` gerektirir) |
| `--samples N` | `--compact-json` ile her R9 kuralına K7 sırasındaki ilk N kök neden bulgusunu (`entity_type`, `entity_id`, `file`, `line`, `field`, `observed_value`, `expected_value`) ekler |
| `-o rapor.json` | Çıktıyı stdout yerine dosyaya yazar |
| `--lang en` | Bulgu metinlerinin dili: `en` (varsayılan) / `tr` / `ja` / `fr` / `es` (`es`: yalnız planlamacı kartları) |
| `--config config.json` | JSON config delta uygular (`ValidatorConfig::default()` üzerine) |
| `--gtfs-jp-profile auto\|v3\|v4` | GTFS-JP kural profilini açıkça seçer; config değerini geçersiz kılar |
| `--today 20260710` | Analiz "bugün"ünü sabitler (takvim kuralları için) |

**Filtreler yalnızca görüntülemeyi daraltır.** `notices` ve R2–R9 listeleri filtrelenir; **R1 yayınlanabilirlik kararı ve R5 skorları her zaman tüm feed'i** anlatır. Filtre uygulandığında JSON'a `filtered` alanı, özete `filter:` satırı eklenir.

`name_index` varsayılan olarak **çıktıya dahil edilmez**: büyük feed'lerde shape/durak koordinat tabloları JSON'un neredeyse tamamını kaplar. Gerekiyorsa `--include-name-index` ile açın.

Feed yolu yerine `-` verilirse ZIP **stdin'den** okunur: `curl -sL <url> | gtfs-analyzer validate - --json`. (ZIP merkezi dizini dosyanın sonunda olduğundan arşiv belleğe alınır, akış hâlinde işlenmez.)

> **Arayüzle sayı farkı:** Tarayıcı, kural başına bulgu örneklerini performans için sınırlar (gerçek toplamlar `capped_totals`'ta bildirilir). CLI bu sınırı **uygulamaz** — aynı feed'de daha çok notice ve sınırlanmamış R9 etki değerleri döner. Fark beklenen davranıştır; iki çıktıyı doğrudan sayı sayı karşılaştırmayın.

### JSON zarf sürümü

Her JSON çıktısı (`--json`, `--compact-json` ve fatal hata nesnesi) üst düzeyde
`schema_version`, `engine_version` ve `engine_commit` taşır. `schema_version`
(şu an `1`) yalnız mevcut bir alan kaldırıldığında, adı değiştiğinde veya anlamı
değiştiğinde artar; yeni alanlar sürüm artmadan eklenebilir. Çıktıyı okuyan programlar
bu alanı denetlemeli ve tanımadıkları alanları kabul etmelidir.

### Compact JSON ve web sınırı

`--compact-json`, tam `--json` raporundaki her notice'ı ve `name_index` tablolarını
taşımayan, makine tüketicileri için kanonik bir projeksiyondur. Üst düzeyde `status`, `schema_version`,
`engine`, `engine_version`, `engine_commit`, `validation_date`, `partial`,
`analysis`, `reports.r1`, `reports.r5`, `reports.r9`, `metrics`,
`notice_counts` ve `triggered_rule_count` alanlarını içerir. `reports.r9.items`
her kuralın başlık, önem, sınıf, mesaj, düzeltme, öncelik, etki ve skor değişimi
alanlarını ve `--lang` dilinde sade dilli bir `card` (`title`, `what`, `action`) taşır;
`--samples N` verilirse her öğeye en fazla N kök neden örneği eklenir.

`--compact-json` web arayüzünün payload'ı değildir ve kendi başına kural başına örnek
sınırı uygulamaz. Web görünümündeki notice listesini CLI full JSON ile yaklaşık eşlemek
isterseniz `--json --max-notices-per-rule 500` kullanın; gerçek R1/R5/R9 hesapları
tam notice listesinden yapılır ve kesilen kurallar `capped_totals` içinde bildirilir.

**Exit kodları:** `0` eksiksiz ve notice'sız doğrulama · `1` notice veya `PARTIAL` (kısmi kapsam) raporu · `2` fatal ya da config/dosya hatası. `PARTIAL` rapor, bozuk/eksik bir dosyayı güvenle atlayıp bağımsız kontrolleri sürdürür; JSON'da `status: "partial"`, `validation_status: "PARTIAL"` ve `partial` kapsamı görünür. `partial.skipped_checks`, önkoşul eksikliği nedeniyle çalıştırılmayan K4/K5/K6 kontrol ailelerini ve kurallarını ayrıntılı olarak listeler; `partial.skipped_stages` kaba aşama metadatası için korunur. `--fail-on*` kullanılsa da `PARTIAL` koşusu `1` döner. JSON modunda stdout yalnızca JSON'dur; hatalar stderr'e yazılır.

```bash
# CI kapısı: yalnız resmi GTFS Spec ihlalleri koşuyu düşürsün
gtfs-analyzer validate feed.zip --fail-on-class spec

# Yalnız Spec bulgularını raporla (skorlar yine tüm feed'i anlatır)
gtfs-analyzer validate feed.zip --class spec --json --pretty -o spec.json
```

```python
import json, pathlib, subprocess

for feed in sorted(pathlib.Path("feeds").glob("*.zip")):
    proc = subprocess.run(
        ["gtfs-analyzer", "validate", str(feed), "--json"],
        text=True, capture_output=True,
    )
    # exit 1 = "notice var", hata değil — check=True KULLANMAYIN
    data = json.loads(proc.stdout)
    if data["status"] == "fatal":
        print(feed.name, "FATAL", data["code"], data["message"])
        continue
    print(feed.name, data["reports"]["r5"]["pub_score"], len(data["notices"]))
```

### `rules` — kural kaydı

Doğrulama çalıştırmadan tüm kural kaydını listeler; entegre eden projenin kural sözlüğü için.

```bash
gtfs-analyzer rules --class spec --severity critical
gtfs-analyzer rules --rule STM_004 --json --pretty
```

Alanlar: `id`, `severity`, `class`, `authority_source`, `base_effort`, `blocks`, `title`, `message`, `remediation`, `card`.
`--class` / `--severity` / `--min-severity` / `--rule` filtreleri `validate` ile aynı anlamdadır.
`--lang` burada da geçerlidir (kural başlıkları, mesajları ve düzeltme metinleri).

`--compact-json` içindeki her R9 öğesi de kural başına `severity`, `rule_class`, `message` ve
`remediation` alanlarını taşır. Büyük feed'lerde isteğe bağlı `--samples N` ile aynı öğeye ilk N
kök neden bulgusu eklenebilir; mesaj ve çözüm metinleri kural şablonudur, örnek kaydın bağlamı
`samples` içindedir.

`triggered_rule_count`, R2'de görünen tüm kural kimliklerini sayar; `r9_total_count` ise yalnızca
kök neden aksiyonlarını sayar. R9 semptomları tekilleştirme için gizler ve gelecekte başlayan
feed'lerdeki sıfır puanlı `CAL_015` bilgi bildirimi bilinçli olarak aksiyon kuyruğuna alınmaz.

### Çıktı dili

Doğrulama çekirdeği bulgu metinlerini Türkçe üretir; `--lang en` / `--lang ja` / `--lang fr` bunları arayüzün kullandığı **aynı çeviri sözlükleriyle** değiştirir. Kural ID'leri, önem ve sınıf değerleri (`CRITICAL`, `SPEC`) her dilde makine-okunur sabit kalır; yalnızca `title`, `message` ve `remediation` çevrilir.

Bir kuralın çevirisi yoksa sıra şudur: istenen dil → İngilizce → Türkçe (çekirdeğin ürettiği metin). Böylece çıktı hiçbir zaman boş kalmaz.

Kural düzeyindeki çıktılar (`--compact-json` R9 öğeleri ve `rules --json`) ayrıca `card` taşır: web arayüzünün planlamacı görünümündeki sade başlık, ne olduğu (`what`) ve ne yapılacağı (`action`); `tr`, `en`, `ja`, `fr` ve `es` dillerinde. `es` yalnız kartları olan bir dildir, diğer metinleri İngilizceye düşer. Kart metni bulguya özgü değerler gerektiriyorsa `what` verilmez.

Sözlükler `ui/src/locales/{en,ja}.ts` dosyalarından `npm run locales:export` ile `crates/core/locales/*.json` içine türetilir; CLI binary'sine ve Python paketine gömülür. Locale güncellenip export çalıştırılmazsa `locale-parity.test.ts` CI'da kırmızı yanar — tek kaynak locale dosyalarıdır.

## 3. Rust kütüphanesi

Doğrulamayı kendi Rust servisinize gömmek için `gtfs-pipeline`'ı doğrudan kullanın — CLI'a, dosya sistemine ya da ağa ihtiyaç yok:

```toml
[dependencies]
gtfs-pipeline = "0.22.0"
gtfs-config   = "0.22.0"
gtfs-core     = "0.22.0"
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

## 4. CI/CD

`--fail-on` bayrağı yalnız istediğiniz sınıf/önem seviyesinde koşuyu düşürür, böylece Analytics gürültüsü pipeline'ı kırmaz:

```yaml
# GitHub Actions — yalnız resmî GTFS Spec ihlalleri koşuyu düşürsün
- name: GTFS feed doğrulama
  run: |
    curl -sL https://github.com/ttezer/gtfs-analyzer/releases/latest/download/gtfs-analyzer-x86_64-linux.tar.gz | tar xz
    ./gtfs-analyzer validate feed.zip --fail-on-class spec --min-severity critical
```

Exit kodları: `0` temiz · `1` eşiği aşan bulgu var · `2` feed okunamadı (fatal).

## 5. `gtfs-sdk` npm paketi

`gtfs-sdk`, v0.22.0 doğrulama motorunu typed JavaScript/TypeScript API olarak sunar. Feed uygulamadan çıkmadan yerel WASM ile doğrulanır:

```js
import { validateGtfs } from "gtfs-sdk";

const result = await validateGtfs(new Uint8Array(zipBytes), {
  today: "2026-08-20",
});
console.log(result.notices.length, result.reports.r5.score);
```

Dışa açılan public API `validateGtfs`, `getVersion` ve progress/cache akışı gereken uygulamalar için `createValidatorSession` içerir. Düşük seviyeli `gtfs-wasm` binding'i SDK sözleşmesinin parçası değildir. WASM64 ve threaded motor seçimi ise ilk SDK paketinde internal kalır.

Paket kaynakları `sdk/` altındadır; ayrıntılı kullanım, sonuç modeli ve config referansı [`sdk/README.md`](../sdk/README.md) içindedir. WASM binding'i build sırasında `crates/wasm` üzerinden üretilir.
Web UI worker'ı da aynı `ValidatorSession` facade'ını kullanır; seri/threaded/WASM64 seçimini yalnızca uygulama içindeki engine adapter belirler.

## 6. Python paketi

[PyPI](https://pypi.org/project/gtfs-analyzer/) üzerinden kurulur (CPython 3.9+):

```bash
pip install gtfs-analyzer
```

Python paketi aynı Rust doğrulama pipeline'ını PyO3 native modülü üzerinden
çalıştırır; ayrıca Cargo CLI kurulumu gerekmez. ZIP dosyasını yol veya bytes
olarak verebilirsiniz:

```python
from gtfs_analyzer import validate_gtfs

result = validate_gtfs("feed.zip", today="2026-08-20")
print(result["validation_status"])
print(len(result["notices"]))
```

`result`, npm SDK ve CLI JSON çıktısıyla aynı temel sonucu taşır. Parametreler,
sonuç yapısı ve hata türleri için [`python/README.md`](../python/README.md)
dosyasına bakın.

## 7–8. ChatGPT ve Claude

Kurulum ve kullanım [README'deki ChatGPT ve Claude bölümünde](../README.md).

## Topluluk bağlamaları

| Dil | Paket | Bakım |
|---|---|---|
| Kotlin / JVM | [`io.github.transittrack:gtfs-analyzer`](https://central.sonatype.com/artifact/io.github.transittrack/gtfs-analyzer) | Bu deponun dışında, kendi yazarlarınca |

Topluluk bağlamaları bu depoda test edilmez ve bu deponun yayın sürecinden çıkmaz.
Motor bulgularını buraya, bağlamaya özgü sorunları paketin kendi deposuna bildirin.
