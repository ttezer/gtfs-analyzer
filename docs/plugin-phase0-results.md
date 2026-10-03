# GTFS Validator plugin — Phase 0 ölçüm sonuçları

Tarih: 3 Ekim 2026  
Branch: `feat/gtfs-validator-plugin`  
MCP spike: `gtfs-validator-plugin`  

Bu sonuçlar ChatGPT Plus hesabındaki bağlı özel MCP sunucusu üzerinden alınmıştır.
`response_probe` ve `sleep_probe` gerçek ChatGPT tool çağrıları olarak çalıştırıldı.

## Tool timeout

| İstenen süre | Sonuç |
|---:|---|
| 0 sn | Başarılı, `elapsed_ms=0` |
| 15 sn | Başarılı, `elapsed_ms=15001` |
| 30 sn | Başarılı, `elapsed_ms=30001` |
| 60 sn | Başarılı, `elapsed_ms=60001` |
| 90 sn | Başarılı, `elapsed_ms=90001` |
| 120 sn | Başarılı, `elapsed_ms=120001` |
| 180 sn | `Code Mode execution timed out`; tool sonucu dönmedi |

Sonuç: 120 saniye doğrulanmış üst başarı noktasıdır. Production toplam request
timeout'u 180 saniye olarak seçilmemelidir; download, stdin aktarımı ve serialization
için pay bırakılarak daha düşük bir bütçe seçilmelidir.

## Response size

| İstenen payload | Gerçek payload | Sonuç |
|---:|---:|---|
| 50 KB | 51,173 bytes | Başarılı |
| 100 KB | 102,374 bytes | Başarılı |
| 250 KB | 255,974 bytes | Başarılı |
| 500 KB | 511,974 bytes | Başarılı |
| 1 MB | 1,048,551 bytes | Başarılı |
| 2 MB | 2,097,127 bytes | Başarılı |
| 4 MB | 4,194,279 bytes | Başarılı |
| 8 MB | 8,388,583 bytes | Başarılı |

Sonuç: 8 MB'a kadar response başarılı oldu. Bu, 8 MB'ın production için güvenli
canonical bütçe olduğu anlamına gelmez; gerçek compact JSON, ChatGPT model context'i
ve farklı client davranışı ayrıca değerlendirilmelidir. MCP katmanında R9 prefix
truncation için ölçüm sonrası bütçe bırakılmalıdır.

## Yerel transport kontrolleri

- Streamable HTTP `/mcp` initialize başarılı.
- `tools/list` üç probe'u döndürdü.
- `file_probe` ile `example.com` ve GTFS Analyzer Pages yanıtları okundu.
- ChatGPT bağlantısında ilk hata `Invalid Host header` idi; geçici tünel hostname'i
  allowlist'e alınarak düzeltildi.

## Eksik ölçüm

ChatGPT'nin `_meta["openai/fileParams"]` ile gerçek yüklenmiş dosyayı MCP'ye aktarması
henüz test edilmedi. Bu, küçük ve hassas olmayan bir dosya yüklenerek ayrıca ölçülmelidir.

