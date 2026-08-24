# Rust / NPM SDK / WASM parite kontrolü — 2026-08-25

Bu kontrol, GTFS Analyzer `0.9.7` motorunun Rust, NPM SDK ve UI/WASM yüzeylerinde aynı sonucu verdiğini doğrular. Herhangi bir npm publish veya Rust crate publish yapılmamıştır.

## Sonuçlar

- `cargo build --release -p gtfs-analyzer`: başarılı.
- `sdk`: `npm run test:smoke`: başarılı; 19 notice, skor `97.2`, session rerun başarılı.
- `sdk`: `npm run package:check`: başarılı; 9 dosya, packed boyut `887858` byte.
- UI serial WASM: başarılı.
- UI threaded WASM: başarılı.
- UI memory64 WASM: başarılı.
- wasm32/wasm64 parity: başarılı; fixture sonucu 19 notice ile birebir eşit.
- TypeScript typecheck: başarılı.
- Vitest: 10 dosya, 98 test başarılı.
- Locale export drift kontrolü: başarılı.
- Vite production build: başarılı.

Rust çekirdeği tek kaynak olarak kalır; CLI, SDK ve WASM aynı pipeline/rule registry kodundan üretilir. SDK build yalnızca bu motoru Node/browser API'sine paketler. Bu nedenle bu kontrolde bağımsız bir JavaScript kural kopyası veya ayrı bir sürümleme yolu eklenmemiştir.
