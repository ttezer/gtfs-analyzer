# GTFS Validator Phase 4 — İlk Limit Kararları

Bu kararlar 2026-10-03 tarihli Phase 0 ve Phase 3 ölçümlerine dayanır. Bunlar
Cloud Run/Linux üzerinde memory ceiling doğrulanana kadar **provisional**
kararlardır; final production zarfı değildir.

## Zaman sınırları

| Sınır | Değer | Gerekçe |
|---|---:|---|
| Analyzer subprocess | 90 s | Phase 0'da 120 s tool çağrısı başarılıydı; native process için ayrı kill sınırı gerekir. |
| Toplam MCP isteği | 105 s | Download + stdin aktarımı + validation için 120 s gözlemine 15 s pay bırakır. |
| HTTP read timeout | 30 s | Yavaş/kesilen indirmede toplam deadline'ı beklemeden kontrollü hata üretir. |

18.9 MB ZIP benchmark'ı 14.740 s'de tamamlandı. 171 MB ZIP 180 s içinde sonuç
üretmedi. Bu nedenle 105 s toplam sınırının üzerindeki koşumlar synchronous
ChatGPT akışında kabul edilmemelidir.

## Compressed input sınırı

İlk aday admission cap: **20 MiB**.

Bu değer 18,875,148 baytlık tamamlanan benchmark'ı kapsar; 171,413,597 baytlık
ve sonuç üretmeyen örneği dışarıda bırakır. Ancak ZIP boyutu RAM garantisi değildir:
18.9 MB ZIP yaklaşık 503.5 MiB peak RSS kullandı. Memory ceiling Linux/Cloud Run
üzerinde doğrulanmadan bu aday production'da kesin limit olarak ilan edilmemelidir.

## Memory ve CPU

Henüz final değer seçilmedi. macOS üzerinde child process'e `RLIMIT_AS` uygulama
denemesi subprocess başlangıcında platform hatası verdi; bu ölçüm Linux/Cloud Run
ortamında `RLIMIT_AS`, `prlimit` veya cgroup yöntemiyle tekrarlanmalıdır.

Memory ceiling devreye alınmadan yalnızca RSS gözlemine bakarak `RESOURCE_LIMIT`
üretilmeyecektir. Native exit/signal ve uygulanan limit birlikte doğrulanmalıdır.

## Sonraki release gate'leri

1. Linux container içinde 512/768 MiB limit denemeleri.
2. 20 MiB admission cap ile gerçek feed yeniden testi.
3. Child kill sonrası MCP server'ın ayakta kaldığının testi.
4. Eşzamanlı iki büyük feed ile concurrency ve CPU ölçümü.
5. Bu kanıtlardan sonra final memory, CPU, concurrency ve max-instance değerleri.
