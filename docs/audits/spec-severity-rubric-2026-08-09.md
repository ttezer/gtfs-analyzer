# Spec Severity Rubric Audit — 2026-08-09

## Amaç

Bu audit, `Spec` sınıfındaki önem seviyelerini MobilityData etiketlerine eşitlemek yerine
aynı iç etki rubric'iyle karşılaştırır. Ölçüt, bir ihlalin feed'in tüketilebilirliğini ve
GTFS semantiğini ne kadar bozduğudur; dış doğrulayıcının aynı bulguya verdiği etiket değildir.

## Uygulanan rubric

| Severity | Kriter | Yayın etkisi |
|---|---|---|
| **Kritik** | Required dosya/alan, primary key, foreign key veya çekirdek type/range ihlali; feed güvenilir biçimde tüketilemez | `Spec + Kritik` → R1 blocker |
| **Yüksek** | Feed parse edilebilir kalsa da sefer, ücret, erişilebilirlik veya Flex/pathway semantiğini maddi biçimde değiştiren doğrudan normatif ihlal | R1 değil; R2/R5/R9 |
| **Orta** | Etkisi sınırlı dosya, alan veya koşullu semantik ihlali; ana model okunabilir kalır | R1 değil; R2/R5/R9 |
| **Düşük** | Dar etkili metadata/opsiyonel alan ölçeğinde normatif sapma | R1 değil; R2/R5/R9 |
| **Bilgi** | Normatif ihlal değil, yalnız ölçüm veya bağlam sinyali | Spec sınıfında kullanılamaz |

Değişmez: `Spec` sınıfında `Bilgi` severity bulunamaz. `STM_048` ve `STM_049`, ham
servis-günü saat notasyonunun sefer zamanlarını yanlış yorumlatabilmesi nedeniyle **Bilgi →
Yüksek** olarak düzeltildi. İki kural ayrı feed-level finding olarak kalır: biri trip'ler
arası `arrival_time` rollover'ını, diğeri aynı satırdaki `departure_time` rollover'ını ölçer.

## Sonuç

Registry'de toplam **307 Spec kuralı** audit edildi:

| Severity | Kural sayısı | Karar |
|---|---:|---|
| Kritik | 183 | Required/PK/FK/çekirdek geçerlilik ve yayın kapısı kuralları |
| Yüksek | 50 | Maddi schedule/fare/Flex/pathway semantik etkisi |
| Orta | 43 | Sınırlı veya koşullu normatif etki |
| Düşük | 31 | Dar etkili normatif metadata/alan sapması |
| Bilgi | 0 | Bilinçli olarak boş |

### Kritik (183)

`ARC_001`, `ARC_004`, `ARC_008`, `ARC_031`, `ARC_012`, `ARC_013`, `ARC_025`, `BKR_007`,
`BKR_008`, `BKR_009`, `BKR_015`, `BKR_016`, `BKR_017`, `BKR_018`, `BKR_019`, `AGN_002`,
`AGN_003`, `AGN_004`, `AGN_010`, `AGN_011`, `AGN_014`, `STP_001`, `STP_002`, `STP_003`,
`STP_004`, `STP_005`, `STP_006`, `STP_007`, `STP_009`, `STP_011`, `STP_012`, `STP_015`,
`STP_018`, `RTS_001`, `RTS_002`, `RTS_003`, `RTS_004`, `TRP_001`, `TRP_002`, `TRP_003`,
`TRP_031`, `TRP_035`, `STM_001`, `STM_002`, `STM_003`, `STM_004`, `STM_005`, `STM_006`,
`STM_015`, `STM_016`, `STM_039`, `STM_046`, `STM_047`, `STM_056`, `STM_058`, `LOC_002`,
`LOC_003`, `LOC_010`, `CAL_001`, `CAL_002`, `CAL_003`, `CAL_004`, `CAL_022`, `CAL_025`,
`CLD_001`, `CLD_002`, `CLD_003`, `SHP_001`, `SHP_002`, `SHP_003`, `SHP_004`, `SHP_005`,
`SHP_008`, `FRQ_001`, `FRQ_002`, `FRQ_003`, `FRQ_004`, `FRQ_008`, `TRF_001`, `TRF_002`,
`TRF_003`, `TRF_005`, `TRF_006`, `TRF_007`, `TRF_008`, `TRF_009`, `TRF_012`, `TRF_016`,
`TRF_021`, `FAR_001`, `FAR_002`, `FAR_003`, `FAR_004`, `FAR_005`, `FAR_008`, `FAR_011`,
`FAR_012`, `FRL_001`, `FRL_002`, `FRL_003`, `FRL_004`, `FRL_005`, `RCT_001`, `RCT_002`,
`RCT_003`, `RCT_008`, `FMD_001`, `FMD_002`, `FMD_004`, `FPD_001`, `FPD_002`, `FPD_003`,
`FPD_004`, `FPD_005`, `FLG_001`, `FLG_002`, `FLG_003`, `FLG_004`, `FLG_005`, `FLG_006`,
`FLJ_001`, `FLJ_002`, `FLJ_003`, `FLJ_004`, `FTR_001`, `FTR_002`, `FTR_003`, `FTR_004`,
`FTR_005`, `ARS_001`, `ARS_002`, `SAR_001`, `SAR_002`, `SAR_003`, `SAR_004`, `NET_001`,
`NET_002`, `NET_003`, `NET_004`, `TFR_001`, `TFR_002`, `TFR_006`, `TFR_007`, `TFR_008`,
`PTH_001`, `PTH_002`, `PTH_003`, `PTH_004`, `PTH_005`, `PTH_020`, `PTH_021`, `PTH_022`,
`PTH_023`, `PTH_024`, `PTH_026`, `PTH_031`, `LVL_001`, `LVL_002`, `LVL_007`, `LVL_008`,
`FIN_001`, `FIN_002`, `FIN_003`, `TRN_001`, `TRN_002`, `TRN_005`, `TRN_006`, `TRN_008`,
`ATR_002`, `ATR_004`, `ATR_005`, `ATR_006`, `ATR_007`, `XFL_015`, `XFL_020`, `XFL_022`,
`XFL_023`, `XFL_024`, `XFL_025`, `XFL_031`, `XFL_032`, `XFL_033`, `XFL_034`.

### Yüksek (50)

`ARC_030`, `ARC_032`, `ARC_033`, `BKR_001`, `BKR_002`, `BKR_003`, `BKR_004`, `BKR_006`,
`BKR_010`, `BKR_013`, `BKR_014`, `STP_008`, `STP_010`, `RTS_028`, `TRP_004`, `TRP_019`,
`STM_009`, `STM_010`, `STM_037`, `STM_060`, `STM_041`, `STM_048`, `STM_049`, `STM_051`,
`STM_052`, `STM_054`, `STM_055`, `LOC_001`, `LOC_011`, `FRQ_011`, `TRF_004`, `TRF_014`,
`TRF_017`, `TRF_022`, `TRF_023`, `TFR_003`, `PTH_012`, `PTH_016`, `FIN_006`, `TRN_004`,
`TRN_009`, `TRN_010`, `TRN_011`, `TRN_013`, `TRN_014`, `TRN_015`, `ATR_003`, `ATR_009`, `XFL_016`,
`DQ_021`.

### Orta (43)

`ARC_024`, `ARC_026`, `BKR_005`, `BKR_012`, `BKR_024`, `BKR_020`, `BKR_023`, `AGN_005`,
`STP_014`, `STP_043`, `RTS_005`, `RTS_006`, `RTS_007`, `TRP_005`, `TRP_034`, `STM_018`,
`STM_019`, `STM_022`, `LOC_004`, `LOC_007`, `LOC_008`, `LOC_009`, `FRQ_007`, `FAR_006`,
`RCT_006`, `FLG_007`, `FTR_006`, `FTR_007`, `FTR_008`, `FTR_009`, `FTR_010`, `FTR_011`,
`TFR_005`, `PTH_006`, `PTH_007`, `PTH_017`, `PTH_027`, `FIN_004`, `FIN_005`, `TRN_003`,
`TRN_017`, `TRN_016`, `XFL_019`.

### Düşük (31)

`BKR_021`, `AGN_006`, `AGN_008`, `AGN_009`, `AGN_012`, `STP_013`, `STP_026`, `STP_036`,
`STP_042`, `RTS_013`, `RTS_018`, `RTS_024`, `RTS_029`, `TRP_006`, `TRP_007`, `TRP_032`,
`STM_030`, `STM_032`, `FRQ_012`, `FAR_013`, `RCT_007`, `FPD_007`, `PTH_010`, `PTH_030`,
`FIN_008`, `FIN_009`, `FIN_012`, `ATR_008`, `ATR_010`, `ATR_011`, `ATR_012`.

## Regression ve bakım kapıları

- `crates/rules/src/registry.rs` testi, `Spec + Bilgi` kombinasyonunu reddeder ve STM_048/049'un Yüksek olduğunu sabitler.
- `stm_048_and_stm_049_are_separate_high_spec_findings`, iki raw olay aynı feed'de bulunduğunda iki ayrı finding üretildiğini ve alanların `arrival_time` / `departure_time` olarak ayrıldığını kanıtlar.
- Kural kartları registry'den `cargo run -p gtfs-rules --example sync_cards` ile güncellenir; kart metadata'sı ve `severity.weight()` registry ile aynı kalmalıdır.
