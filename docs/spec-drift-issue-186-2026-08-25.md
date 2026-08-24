# Spec-drift issue #186 denetimi — 2026-08-25

Issue #186, bot tarafından yeniden açılmış eski bir kayıt değil; `spec-drift` workflow'unun 2026-08-24 tarihinde upstream commit ilerlemesi nedeniyle açtığı yeni bir uyarıdır.

Yerel denetim komutları:

```text
python3 spec-audit/spec_drift.py --mode upstream
python3 spec-audit/spec_drift.py
```

## Sonuç

- Upstream `reference.md` commit'i: `ac663b574aae` → `3215f98f2661`.
- Canlı sayfa okunabildi; `auto` modu upstream sinyalini de ölçtü.
- Katalog aday sayısı: `300 → 300`.
- Eklenen aday: `0`.
- Kaybolan aday: `0`.
- `spec_revision`: `April 27, 2026 → April 27, 2026`.
- `provisions_sha256`: `0c49d5d2a5fa → 0c49d5d2a5fa`.
- Ham sayfa özeti eşit değil; bu proje politikasına göre site/Cloudflare sayfa iskeleti sinyalidir ve drift kararı için kullanılmaz.

## Karar

Normatif katalog değişmediği için `spec-audit/spec_provisions.json`, `PROVISION_TRIAGE.md` ve kural registry'si değiştirilmedi. Issue'nun mevcut durumu gerçek bir upstream hareketidir, fakat bu koşuda yeni hüküm veya adjudication işi üretmemiştir. Bir sonraki upstream commit'te katalog özeti değişirse `extract_provisions.py` ve provision triyajı yeniden çalıştırılmalıdır.
