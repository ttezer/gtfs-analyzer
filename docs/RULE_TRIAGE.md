# Kural Farkı Triyaj Listesi

> Bir feed'i kendi validator'ımız ile **MobilityData (MD)** veya **GTFS Guru** ile karşılaştırınca çıkan
> her farkı sınıflamak için akış. **Amaç:** "Biz hatalıyız" refleksini kırmak.
>
> **Temel ilke:** MD ve Guru *ground truth değildir* — ikisi birbiriyle de anlaşmaz. Fark, **suçlu ilanı değil,
> araştır sinyalidir.** Hakem her zaman **GTFS spec metni** (+ resmi örnek feed'ler), başka bir araç değil.

## Adım 0 — Farkı netleştir
- Hangi `rule_id` / hangi entity / hangi satır?
- Biz mi fazladan bayrak kaldırdık (onlarda yok), yoksa biz mi kaçırdık (onlarda var)?
- Karşı taraf MD mi, Guru mu? (İkisi farklıysa zaten "tek doğru" yok.)

## Adım 1 — 7 kategoriden hangisi?

| # | Fark türü | Nasıl anlaşılır | "Biz hatalı mıyız?" | Aksiyon |
|---|---|---|---|---|
| 1 | **Kapsam** | Kuralın `Sınıf`'ı Analytics/Quality ve karttaki "Dış araç eşleşmesi" **proje-özel** diyor | Hayır, tasarım | Kapat; sapma defterine "kapsam" yaz |
| 2 | **Eşik** | Aynı kavram, farklı sayısal sınır (km, karakter, süre, oran) | Hayır, politika | Eşiği teyit et; gerekirse config'le hizala; deftere "eşik" |
| 3 | **Severity/Sınıf** | Aynı bulgu, farklı önem/blocker | Hayır, politika | Deftere "severity"; istersen hizala |
| 4 | **Granülarite** | Biz satır-bazlı, onlar feed-özet (veya tersi) — karttaki `Varlık` Feed/Row/Entity'e bak | Hayır, aynı tespit | Kapat; deftere "granülarite" |
| 5 | **Spec sürümü** | Fares v2 / NetworksAndAreas / Flex gibi uzantı; biri uygular biri etmez | Hayır, sürüm | Kapat; deftere "spec-sürüm" |
| 6 | **Gerçek yanlış pozitif** | Spec "sorun yok" diyor ama biz bayrak kaldırıyoruz | **EVET** | Spec'ten doğrula → regresyon testi → düzelt |
| 7 | **Gerçek yanlış negatif** | Spec zorunlu/önerir, biz kaçırıyoruz | **EVET** | Spec'ten doğrula → regresyon testi → ekle |

## Adım 2 — Önce karta bak (ilk durak)
`docs/rules/<GRUP>/<ID>.md`:
- **"Dış araç eşleşmesi"** → MD/Guru karşılığı zaten yazılı mı? "proje-özel" mi?
- **"Yanlış pozitif / negatif"** → bu fark zaten bilinen tasarım riski mi?
- **"Karışan komşular"** → fark aslında komşu bir kuralın kapsamında mı?

Çoğu fark bu adımda 1–5'e düşer ve **kapanır.**

## Adım 3 — Sapma Defterine bak
`docs/DIVERGENCE_LEDGER.md`:
- Bu kural için sapma **kayıtlı** mı? Kayıtlıysa → "beklenen, biz hatalı değiliz". Bitti.
- Kayıtlı değilse → muhtemelen 6/7. Adım 4.

## Adım 4 — Yalnızca 6/7 ise: spec'ten karara bağla
- **Hakem:** GTFS Reference (`gtfs.org/documentation/schedule/reference/`) + Best Practices + resmi örnek feed'ler.
- MD/Guru'nun **ne dediği değil**, **spec'in ne dediği** belirleyici.
- Karar netse:
  1. Minimal bir **test feed'i** (beklenen sonucu spec'e göre elle doğrulanmış) ekle.
  2. Regresyon testi yaz (kural tetiklenmeli/tetiklenmemeli).
  3. Kodu düzelt; testi yeşile al.
- Karar belirsizse (spec gri alan): sapma defterine "spec-belirsiz" olarak kaydet, **gerekçeyle**; ileride MD/Guru ile aynı yöne hizalamayı tartış.

## Adım 5 — Defteri güncelle
Hangi sonuca varırsan var (kapsam/eşik/severity/granülarite/spec-sürüm/düzeltildi), `DIVERGENCE_LEDGER.md`'ye
o kuralın satırını **gerekçeyle** yaz. Böylece aynı fark bir daha çıktığında Adım 3'te anında kapanır.

---

## Hızlı hatırlatma
- **Farklı çıktı = araştır, suçlanma.** Yalnızca spec metni seni "haksız" çıkarabilir.
- **MD ile Guru çelişiyorsa** zaten mutlak doğru yok; kendi spec-temelli kararın geçerli.
- Bir kez karara bağlanan sapma **deftere** girer; ikinci kez tartışılmaz.
