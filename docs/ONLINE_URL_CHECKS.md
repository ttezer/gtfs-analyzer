# Online URL Checks Boundary

Core analiz deterministiktir ve yüklenen GTFS verisini ağ üzerinden dışarı göndermez.

- `source_url`, çağıran ürün tarafından sağlanan isteğe bağlı metadata'dır; `ARC_028` yalnız URL yolunun `.zip` dosya adı taşımasını kontrol eder.
- GTFS içindeki URL'lere HTTP isteği yapılmaz. Bu nedenle aynı feed aynı config ile çevrimdışı ve çevrimiçi ortamda aynı sonucu verir.
- Gelecekteki 404/erişilebilirlik denetimi ayrı, açıkça opt-in bir online adapter olmalıdır. Adapter timeout, redirect sınırı, DNS/IP SSRF koruması, concurrency, HEAD→GET fallback, CORS ve gizlilik politikasını tanımlamadan core'a bağlanmamalıdır.
- Online sonuçlar ana yayın skorunu değiştirmemeli; zaman damgalı harici gözlem olarak raporlanmalıdır.
