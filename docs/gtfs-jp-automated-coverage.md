# GTFS-JP V3/V4 — %100 otomatik kapsama sözleşmesi

Bu belge UI'daki `GTFS-JP V3/V4 · %100 otomatik kapsama (insan yorumu
gerektiren kurallar hariç)` rozetinin ürün sözleşmesidir. Yüzde feed'in
doğrulama sonucunu, dosya okunabilirliğini veya R1 kapsamını göstermez; seçilen
profildeki **makineyle doğrulanabilen MLIT hükümlerinin** ürün envanterine
alındığını gösterir.

## Rozetin açılma koşulları

- Bağımsız GTFS-JP detection gate'i açık olmalıdır. Detection, JPN bulgularından
  veya profil seçiminden geri beslenmez.
- Kullanıcı açıkça `v3` veya `v4` profilini seçmiş olmalıdır. `auto`, sürüm
  iddiası taşımadığı için bu rozeti göstermez.
- Rozet, CI ile doğrulanan makine-okunur MLIT provision envanterinden üretilir; feed'e ait `r1.coverage_complete` alanı
  bu karara dahil değildir. R1 kapsamı eksikse yalnız yayınlanabilirlik kartı
  etkilenir.
- Gerçek法人番号 kurum eşleşmesi, kana/okuma doğruluğu, dış veriyle ücret
  yorumu ve insan kararı gerektiren konular yüzdeye dahil değildir.

## Makineyle doğrulanan envanter

| Kapsam | V3 | V4 | Runtime kanıtı |
|---|---|---|---|
| Ortak GTFS-JP tespit, profil kapısı ve ad alanı koruması | `JPN_033` ortak `*_jp` / `jp_*` alt kümesi | `JPN_033` V4 `*jp` / `jp*` kümesi | `k4_cross_ref::check_gtfs_jp` ve `JPN_033` fixture'ları |
| V3 uzantı dosyaları ve referansları | `JPN_002/003/005/012–018/020` | V4 ana profilinde çalışmaz | `gtfs-jp-v3-v4-matrix.md`, JP takip fixture'ları |
| Zorunlu/önerilen ad ve çeviri okumaları | `JPN_001/004/008–011/019/021/028/030` | `JPN_001/004/008–011/019/021/029` | translations ve kana testleri |
| V4 sabitleri ve ana alanları | `JPN_023–026`, ortak `JPN_022` dalları | `JPN_022–026` | profil ve emit-proof testleri |
| V3 `agency_id` ve otobüs profili | `JPN_027`, `JPN_032` | — | strict V3 test matrisi |
| Ücret bölgesi kapsamı | `JPN_031` | `JPN_031` | geçerli zone kapsamı ve bozuk referans regresyonları |
| V4'ün genel GTFS hükümleri | koşula bağlı ortak GTFS kuralları | `AGN_011`, `STP`, `STM`, `TRP`, `FAR` ve ilgili kural aileleri | `PROVISION_TRIAGE.md`, provision evidence ve spec-conformance testleri |

JPN kural kaydı 33 karttan oluşur. Envanter, her MLIT hükmünü güçlü, yumuşak veya insan incelemesi olarak sınıflandırır; ayrıntılı alan, sürüm, sınıf ve test
eşleşmeleri [GTFS-JP V3/V4 uyumluluk matrisinde](gtfs-jp-v3-v4-matrix.md)
bulunur. Genel GTFS kurallarının MLIT hükmünü taşıdığı durumlar matrisin
ilgili satırında ayrıca gösterilir; aynı hüküm için ikinci bir JPN kuralı
üretilmez.

## Makine-okunur MLIT hüküm envanteri

Kaynak gerçekliği ve yüzde hesabının paydası [`spec-audit/gtfs_jp_provisions.tsv`](../spec-audit/gtfs_jp_provisions.tsv) dosyasıdır. CI bu tabloyu registry ile karşılaştırır; güçlü makine hükümlerinin tamamı bir Analyzer kuralına bağlanmadıkça kapı kapanır.

**Eşleşmemiş güçlü makine hükmü: 0**

Envanterde **33 makine kuralı eşleşmesi** bulunur (30 güçlü hüküm, 3 yumuşak
öneri). Yumuşak öneriler de otomatik çalışır; uyumluluk yüzdesinin zorunlu
paydasına yalnız güçlü hükümler girer.

| Provision ID | Profil | Güç | Otomasyon | Rule ID | MLIT / temel kaynak |
|---|---|---|---|---|---|
| `V3-AGENCY-ID` | v3 | strong | rule | `JPN_032` | MLIT V3 s.11 |
| `V3-AGENCY-JP-ID` | v3 | strong | rule | `JPN_012` | MLIT V3 agency_jp |
| `V3-AGENCY-JP-REFERENCE` | v3 | strong | rule | `JPN_003` | MLIT V3 agency_jp |
| `V3-AGENCY-ZIP` | v3 | soft | rule | `JPN_013` | MLIT V3 agency_jp |
| `V3-OFFICE-ID` | v3 | strong | rule | `JPN_014` | MLIT V3 office_jp |
| `V3-OFFICE-NAME` | v3 | strong | rule | `JPN_005` | MLIT V3 office_jp |
| `V3-OFFICE-CONTACT` | v3 | soft | rule | `JPN_020` | MLIT V3 office_jp |
| `V3-OFFICE-REFERENCE` | v3 | strong | rule | `JPN_002` | MLIT V3 office_jp |
| `V3-ROUTES-JP-ROUTE` | v3 | strong | rule | `JPN_015` | MLIT V3 routes_jp |
| `V3-ROUTES-JP-DATE` | v3 | soft | rule | `JPN_016` | MLIT V3 routes_jp |
| `V3-PATTERN-ID` | v3 | strong | rule | `JPN_017` | MLIT V3 pattern_jp |
| `V3-PATTERN-REF` | v3 | strong | rule | `JPN_018` | MLIT V3 trips/pattern_jp |
| `V3-TRANSLATIONS-FILE` | v3 | strong | rule | `JPN_004` | MLIT V3 translations |
| `V3-TRANSLATIONS-KANA` | v3 | strong | rule | `JPN_001`, `JPN_008`, `JPN_009`, `JPN_010`, `JPN_028` | MLIT V3 translations s.33 |
| `V3-TRANSLATIONS-JA` | v3 | strong | rule | `JPN_030` | MLIT V3 translations s.33 |
| `V3-TRANSLATIONS-RECORD` | v3 | strong | rule | `JPN_019`, `JPN_021` | MLIT V3 translations |
| `V3-FEED-INFO` | v3 | strong | rule | `JPN_007` | MLIT V3 feed_info |
| `V3-AGENCY-REQUIRED` | v3 | strong | rule | `JPN_011` | MLIT V3 agency/routes |
| `V3-ROUTE-TYPE` | v3 | strong | rule | `JPN_027` | MLIT V3 routes.txt |
| `V3-ZONE-ID` | v3 | strong | rule | `JPN_031` | MLIT V3 fare/stop kapsamı |
| `SHARED-NAMESPACE` | v3,v4 | strong | rule | `JPN_033` | MLIT V3 s.10; V3→V4 farkı |
| `V4-FEED-LANG` | v4 | strong | rule | `JPN_023` | MLIT V4 s.29 |
| `V4-AGENCY-LANG` | v4 | strong | rule | `JPN_024` | MLIT V4 s.35 |
| `V4-AGENCY-TIMEZONE` | v4 | strong | rule | `JPN_025` | MLIT V4 s.35 |
| `V4-FARE-CURRENCY` | v4 | strong | rule | `JPN_026` | MLIT V4 s.67 |
| `V4-FARE-FILE` | v4 | strong | rule | `JPN_006` | MLIT V4 s.67 |
| `V4-CORE-FIELDS` | v4 | strong | rule | `JPN_022` | MLIT V4 s.29,35,38,120 |
| `V4-TRANSLATIONS-FILE` | v4 | strong | rule | `JPN_004` | MLIT V4 translations |
| `V4-TRANSLATIONS-KANA` | v4 | strong | rule | `JPN_001`, `JPN_008`, `JPN_009`, `JPN_010`, `JPN_019`, `JPN_021`, `JPN_029` | MLIT V4 s.75-77 |
| `V4-FARE-AGENCY` | v4 | strong | rule | `AGN_011` | MLIT V4 farkı; GTFS Reference |
| `V4-LOCATION-HIERARCHY` | v4 | strong | rule | `STP_009`, `STP_010`, `STP_011`, `STP_012`, `STP_021`, `STP_032`, `STP_036` | MLIT V4 s.38,120; GTFS Reference |
| `V4-FLEX-STOP-TIMES` | v4 | strong | rule | `STM_037`, `STM_038`, `STM_039`, `STM_040`, `STM_041`, `STM_051`, `STM_052`, `STM_054`, `STM_055`, `STM_058` | MLIT V4 farkı; GTFS Reference |
| `V4-CONTINUOUS-SHAPE` | v4 | strong | rule | `TRP_019` | MLIT V4 farkı; GTFS Reference |
| `V4-TRANSFER-RECOMMENDATION` | v4 | soft | excluded_recommendation | — | MLIT V4 farkı |
| `V4-JP-EXTENSION-MASTER` | v4 | soft | excluded_recommendation | — | MLIT V4 farkı |
| `HUMAN-CORPORATE-IDENTITY` | v3,v4 | manual | manual | — | MLIT V3/V4法人番号 |
| `HUMAN-KANA-PRONUNCIATION` | v3,v4 | manual | manual | — | MLIT V3/V4 translations |
| `HUMAN-FARE-EXCEPTION` | v4 | manual | manual | — | MLIT V4 ücret istisnası |
| `HUMAN-STOP-ACCESS` | v4 | manual | manual | — | MLIT V4 stop access |

## Kapsam dışı insan ve dış doğrulama kararları

Otomatik validator bir `agency_id` değerinin gerçekten doğru işleticiye ait
olduğunu, kana çevirisinin insan açısından doğru okunuşu verdiğini veya karmaşık
ücret istisnalarının dış tarife belgesiyle uyumunu kanıtlayamaz. `stop_access`,
operatör niyeti ve benzeri bağlam isteyen değerlendirmeler de bu rozetin
paydasında değildir; bunlar ayrı insan-okur inceleme maddeleridir.

## Kanıt ve yeniden değerlendirme

Kapsam değiştiğinde bu envanter ve kural kartları birlikte güncellenir; genel
GTFS kanıt defterleri (`PROVISION_TRIAGE.md`, `provision_evidence.tsv`) kendi
alanları için ayrı tutulur. Yeni bir güçlü makine hükmü eklenirse önce bu
tabloya yazılır ve eşleşmemiş sayısı sıfıra indirilmeden rozet yayımlanmaz. Rust/native, WASM ve SDK aynı
K2/K4 kayıtlarını kullanmalı; WASM'in iki K2 yolu da bilinmeyen özel dosya
başlıklarını GTFS-JP ad alanı kontrolüne taşır. CI'daki card-consistency,
provision-audit, Rust/WASM ve UI testleri bu sözleşmenin uygulanabilirliğini
kontrol eder.
