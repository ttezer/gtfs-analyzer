# Tam MobilityData kataloğu koşumu — devretme belgesi

> 🔴 **BU BELGE KOŞUM ÖNCESİ BİR PLANDIR VE SAYILARI GEÇERSİZDİR.** İki gerçek koşum yapıldı;
> aşağıdaki eleme zinciri (3.358 katalog → 1.479 koşulabilir) o zamanki `notgit/corpus/catalog.csv`
> anlık görüntüsünden türetilmişti ve gerçek katalogla tutmadı. Belge, NE PLANLANDIĞININ kaydı
> olarak duruyor; **güncel sayı için buraya değil koşum artefaktlarına bakın.**
>
> | | run-31934698855 (08-16) | run-31981225727 (08-17) |
> |---|---:|---:|
> | katalog Schedule satırı | 4.463 | — |
> | denenen feed | 4.259 | **4.258** (`mdb-2904` talimatla dışlandı) |
> | inen | 4.226 | 4.226 |
> | iki validatör de temiz | 4.215 | **4.214** |
> | süre kapsaması | **0 / 4.259** 🔴 | Analyzer 4.224/4.224 · MD 4.216/4.216 ✅ |
> | `md_mapped_missing` | 4.706 | **43** |
> | `md_unmapped` | 1.008 | 57 |
> | `adjudicated_divergence` | — | 9.595 |
> | `context_unresolved` | — | 7 |
> | medyan süre (Analyzer / MD) | ölçülmedi | **0,05 sn / 3,06 sn** |
> | p95 süre | ölçülmedi | 3,65 sn / 12,72 sn |
> | aynı SHA-256 grubu | ölçülmedi | **101 grup / 206 feed** |
>
> 🔑 **İki koşum arasında ÜRÜN değişti, KATALOG neredeyse değişmedi:** ortak 4.258 feed'in
> **4.185'i bayt bayt aynı**, yalnız 72'sinin içeriği değişti. MobilityData bir kontrol grubudur
> (kodu v8.0.1'de sabit, yalnız tarihi kaydı): onun toplamı düz kaldı (120,4M → 120,4M), bizimki
> 45,5M → 46,9M çıktı. Fark ürün değişikliklerimizdir, katalog kayması değil.
>
> ⚠️ **4.706 → 43 düşüşünde ÜRÜN neredeyse hiç pay sahibi değildir.** Aynı veriye karşı yeniden
> türetildiğinde de aynı sonuç çıkıyor: fark, aracın bağlam çözebilmesi ve karar defterlerini
> okuyabilmesidir (#146, #148). "Körlüğümüz" diye raporlanan şeyin %74'ü zaten yargılanmıştı.
>
> Artefaktlar: `audit-results/full-mdb-schedule-20260816/run-31934698855/` (main) ve
> `audit-results/full-mdb-schedule-20260817/run-31981225727/`
> (`agent/validator-audit-all-mdb-20260817`, `437ec7f0` — dal bilerek tutuluyor).
> Kural bazlı hükümler `fp_adjudication.tsv`'de, `run_id` kolonuyla ayrılmış.

Bu iş başka bir asistana veriliyor. Belge, koşumu yapacak olanın bilmesi gereken ölçülmüş
gerçekleri ve geri istenen çıktıyı tanımlar. Sayıların hepsi bu repoda ölçüldü; tahmin yok.

## 0. Katalog gerçekte ne kadar — ELEME ZİNCİRİ ÖLÇÜLDÜ

`notgit/corpus/catalog.csv` (MobilityData `sources.csv` anlık görüntüsü). Her adım
sayıldı, tahmin yok:

| adım | kalan | eleme |
|---|---:|---:|
| katalog toplamı | **3.358** | — |
| `data_type == "gtfs"` | **2.386** | −972 **GTFS-RT** |
| `status ∈ {active, ""}` | **1.662** | −724 (`deprecated` 622 · `inactive` 191 · `development` 7) |
| kimlik doğrulama yok | **1.479** | −183 |
| `urls.latest` var, `redirect.id` yok | **1.479** | −0 |

🔴 **KOŞULABİLİR FEED 1.479'DUR, 2.386 DEĞİL.** İlk tahminim 2.386'ydı ve yanlıştı: statik
olmak yetmiyor, feed'in **yaşayan** ve **açık** olması da gerekiyor. En büyük tek kesinti
**622 `deprecated`** — MobilityData'nın emekliye ayırdığı kayıtlar; koşmak istatistiği ölü
veriyle kirletir.

Boyut: mevcut 242 feed 2,2 GB → 1.479 feed doğrusal ölçekle **~13 GB**.
⚠️ Ölü URL oranı bizim örneklemimizde **14/242 (%5,8)** ölçüldü; elemeden GEÇEN feed'lerde
bile indirme başarısızlığı bekleyin.

### 0.1 Eleme filtresi — kopyalanabilir

`corpus_batch.py::eligible()` bu filtreyi zaten uyguluyor; ayrı bir araç yazılacaksa
birebir aynısı kullanılmalı:

```python
def eligible(row):
    return (
        row.get("data_type") == "gtfs"                              # 972 RT satırını eler
        and row.get("status") in ("active", "")                     # deprecated/inactive/development
        and row.get("urls.authentication_type", "") in ("", "0")    # #55: auth'lu feed indirilemez
        and row.get("urls.latest")
        and not row.get("redirect.id")                              # başka kayda yönlenenler
    )
```

🔴 **GTFS-RT satırları TEK TEK DENENMEMELİ.** Bu doğrulayıcıda realtime desteği YOKTUR —
kod tabanında `protobuf` / `realtime` / `TripUpdate` için **sıfır** eşleşme var. 972 RT
kaydını indirip doğrulamaya vermek 972 anlamsız hata üretir ve hiçbirini rapora yazmak
doğru olmaz. `data_type` sütunu bu ayrımı zaten veriyor; başka bir sezgiye gerek yok.

⚠️ RT feed'lerinin bir kısmının `static_reference` sütununda statik karşılığı yazar. O
karşılık **zaten `gtfs` satırı olarak katalogda vardır** — RT satırından türetilirse aynı
feed İKİ KEZ sayılır (bkz. §3.4 aynalar).

## 1. 🔴 EN KRİTİK MADDE: bu daha büyük bir örneklem DEĞİL, BAŞKA bir popülasyon

`spec-audit/corpus-evidence/rule_stats.csv` **242 feed'lik KATMANLI** bir örneklemdir —
ülke başına tavan uygulanarak, çeşitlilik için seçilmiştir (`corpus_batch.py cmd_sample`:
*"amaç HACİM değil ÇEŞİTLİLİK"*). Kör rastgele seçim kataloğun kendi ABD/Avrupa
ağırlığını verir.

Dolayısıyla **1.479 feed'lik bir koşum o baseline ile "kodda ne değişti" diye
KIYASLANAMAZ.** Değişen şey kod değil popülasyondur. Bu, bu turda yaşanan `--today`
hatasının başka kılıktaki hâlidir: iki koşum ancak TEK bir değişkende farklıysa
kıyaslanabilir.

**Doğru kullanım:** tam katalog koşumu YENİ BİR ÖLÇÜMDÜR. Kendi baseline'ı olur; eskisinin
yerini ALMAZ, yanına konur. Diff istenecekse ya (a) aynı 242 feed yeniden koşulur, ya da
(b) tam katalog iki kez koşulur (önce/sonra).

## 2. Geri istenen çıktı — DÖRT DOSYA, ham bulgu YOK

`python3 spec-audit/corpus_report.py` tam olarak bunu üretir (`notgit/corpus/report/`):

| dosya | neden |
|---|---|
| `provenance.json` | binary commit · `--today` · feed sayısı · hariç tutulanlar · **ağaç kirli miydi** |
| `rule_stats.csv` | kural bazında toplam, diff'lenebilir şema |
| `per_feed_rules.tsv` | feed × kural sayımı + `status`/`coverage_complete`/skor |
| `samples.jsonl` | kural başına ≤3 bulgu, `observed` 80 / `message` 160 karaktere kırpılmış |

🔴 **Tam JSON çıktıları GÖNDERİLMESİN.** Bizim 242 feed'lik korpus 11,6M bulgu üretiyor;
tam katalog bunun ~10 katı olur. Okunamaz, taşınamaz, faydası yok.
🔴 **Zip'ler de saklanmak zorunda değil.** Teslim edilen şey rapordur, arşiv değil.

## 3. Koşum disiplini — her maddesi bu turda hataya mal oldu

1. **`--today` SABİT ve KAYITLI olmalı.** Mevcut baseline `20260717` ile üretildi. Farklı
   günle kıyas kodu değil TAKVİMİ ölçer: bu turda 35 kural sahte "değişti" gösterdi,
   başı `CAL_017` **+%135490**. Takvime göreli kurallar (`CAL_*`, `TRP_026`, feed
   geçerlilik penceresi) tamamen tarihe bağlıdır.
2. **TEK binary.** Koşum ortasında derleme yapılmaz. `provenance.json` commit'i yazar ve
   ağaç kirliyse işaretler — commit'siz kaynaktan koşum hiçbir commit'e karşılık gelmez.
3. **`mdb-2904` KOŞULMAZ** (kullanıcı talimatı). 85 MB, tek başına 5.374.292 bulgu.
   `corpus_report.py` içinde `EXCLUDED_FEEDS` kapısı var.
4. **Aynalar/kopyalar elenmeli.** Bizim 242'lik kümede `PID_GTFS` (45,63 MB) ve `mdb-767`
   (45,88 MB) ikisi de 1494 pathway taşıyor — aynı feed'in iki kaydı. Aynaları iki kez
   saymak HER yüzdeyi bozar. `sha256` ya da (satır sayısı + boyut) ile dedupe edin.
   ⚠️ İkinci bir çift-sayım kaynağı: RT satırlarının `static_reference` sütunu. O statik
   feed zaten kendi `gtfs` satırıyla katalogda; RT'den türetmek aynı feed'i tekrar ekler.
5. **Zaman aşımı koyun ve KAYDEDİN.** Bir feed 30 dakikada bitmeyebilir; atlanan feed
   sessizce kaybolmasın, `status` sütununa yazılsın.
6. **Devam ettirilebilir olsun.** `corpus_report.py` `per_feed_rules.tsv`'yi okuyup
   biteni atlar; koşum kesilirse dosyayı silmeyin.

## 4. Bu koşumun ne kazandıracağı — ve kazandırmayacağı

**Kazandırır — yanlış pozitif keşfi.** Ölçülmemiş asıl eksen budur. Faz 3'te 287 tetiklenen
kuraldan yalnız **28'i** (bulgusunun %90+'ı tek feed'de toplananlar) örneklenebildi.
Daha çok üretici, bir kuralın tek bir alışkanlığa kilitlenmesini yakalama şansını artırır.

**Kazandırmaz — ve bunlar ÖLÇÜLDÜ, tahmin değil:**
- `SELECTION_BIAS` kovası (66 Kritik∧Spec kural) **kapalı kalır.** İhlali feed'i
  yayımlanamaz yapan kurallardır; `agency_name`'i eksik feed katalogda bulunmaz.
  Bunların kanıtı mutasyondur, hacim değil (`EVIDENCE_BASE §5.7.1`).
- `fare_leg_join_rules.txt`: 242 feed'de **0**, ve MD'nin kendi Adoption Tracker'ı bu
  dosyayı izlemiyor — sahada benimsenmemiş.
- `stops.stop_access`: 242 feed'de **0** (altı pathway feed'i tek tek tarandı).

## 5. Geri geldiğinde bizim yapacağımız

1. `provenance.json` okunur — `--today`, commit, kirli-ağaç bayrağı. Uymuyorsa koşum
   KANIT SAYILMAZ.
2. Aynalar elenmiş mi, feed sayısı beyanla tutuyor mu denetlenir.
3. `rule_stats.csv` üzerinde **yoğunlaşma triyajı** koşulur (Faz 3'ün yöntemi): bulgusunun
   %90+'ı tek feed'de olan kurallar çıkarılır, her birinin `samples.jsonl`'daki ilk
   örnekleri kartına karşı okunur. Kararlar `spec-audit/fp_adjudication.tsv`'ye yazılır.
4. Yeni açılan `THIN_FEATURE`/`NO_FEATURE` kovaları `silent_rules.tsv`'de güncellenir.
