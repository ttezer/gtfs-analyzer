# RULE_ID — Kural başlığı

> **Karar:** Kuralın tek cümlelik kararı. **Düzeltme:** En kısa düzeltme yönü.
> ⚠️ Bu kontrolün en önemli kapsam sınırı.

| Alan | Değer |
|---|---|
| Grup | GRP — Grup adı |
| Önem | Kritik/Yüksek/Orta/Düşük/Bilgi |
| Sınıf | Spec/Quality/Interop/Analytics |
| Otorite kaynağı | GTFS_SPEC / GTFS_BEST_PRACTICE / MOBILITYDATA_PARITY / GOOGLE_TRANSIT_INTEROP / REGIONAL_PROFILE / PROJECT_QUALITY / PROJECT_ANALYTICS / UNKNOWN |
| Aşama | K1/K2/K3/K4/K6/K7 |
| Varlık | Feed/File/Entity/Row |
| Kimlik alanı | `scope_key` veya - |
| Skor tabanı | n |
| Görünürlük | VS_K/VS/VI/VA (+ _GEO/_ACC varyantları) (`R...`) |
| Bloke ettiği kurallar | `RULE_X`, `RULE_Y` veya - |

## Tetikleme koşulu (kesin mantık)
`kaynak_dosya::fonksiyon()` içinde:
```
koşul
→ notice
```
- Kritik eşikler ve karşılaştırmalar.
- Kapsam birimi.

## Yanlış pozitif / negatif (asıl tasarım riski)
- **Yanlış negatif:** Hangi durumda çıkmaz ama çıkması beklenebilir?
- **Yanlış pozitif:** Hangi durumda çıkar ama gerçekte sorun olmayabilir?

## Karışan komşular (ayrım)
| Kural | Ne yakalar | Eksen | Önem · Sınıf · Aşama |
|---|---|---|---|
| **RULE_X** | ... | ... | ... |

## Mesaj & çözüm önerisi
### R9 Kural Mesajı
- **Kural mesajı:** Karttaki `Başlık` değeri.
- **Çözüm önerisi:** ...

### R2 mesajı/mesajları
- **Ayrıntılı bulgu mesajı:** *"..."*

## Ölçülen alan / değer
- **Dosya/alan:** `file.txt` → `field`
- **Gözlenen değer:** ...
- **Beklenen değer:** ...

## Kod değiştirirken minimum test matrisi
- Temel pozitif örnek.
- Temel negatif örnek.
- Sınır değer.
- İlgili komşu kural ile karışmama testi.

---
## Teknik ek

### Skora katkı
- **severity.weight() = ...**
- R1/R5/R8/R9 etkisi.

### Hangi raporlarda görünür
`report_views = ...`

### Bağımlılık (bunu maskeleyen root cause'lar)
Varsa `blocks` ilişkileri.

### Dış araç eşleşmesi
<!-- Makine-okunur özet (Sapma Defteri bu satırdan üretilir). Tür: MD-parite | proje-özel | spec-sürüm | eşik | severity | İNCELE -->
- **Eşleşme:** `md_notice_adı` veya — · **Tür:** MD-parite · **Bilinçli:** — (gerekçe / parite-test, belirlenince doldur)
Varsa MobilityData veya GTFS Guru tarafında yer alan tarih damgalı, doğrulanmış eşleşme (uzun açıklama, prose).

### GTFS spec referansı
Varsa ilgili GTFS spec maddesi ve kaynak linki

### Kod referansı
- Emit: [dosya:line](../../...)
- Test: [dosya:line](../../...)

