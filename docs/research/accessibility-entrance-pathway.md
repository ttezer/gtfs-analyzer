# Accessibility candidate: station entrance/pathway coverage

## 2026-09-21 araştırma sonucu

GTFS Pathways, istasyon giriş/çıkışlarını (`location_type=2`) platformlara veya
boarding area'lara bağlayan bir grafik olarak tanımlar. Pathways dosyası isteğe
bağlıdır; bu nedenle `station + entrance` bulunup `pathways.txt` bulunmaması tek
başına Spec veya Quality ihlali sayılamaz.

Mevcut validator davranışı korunmalı:

- `PTH_012`, pathway grafiği bulunan istasyonlarda platformun entrance'tan
  erişilebilirliğini denetler.
- `PTH_013`, entrance-platform yolundaki erişilebilirlik koşullarını denetler.
- Yeni aday yalnızca `INFO · Accessibility Analytics` olabilir ve mevcut
  `stop_access`/`wheelchair_*` sinyallerini tekrar puanlamamalıdır.

Önerilen sonraki ölçüm: station altında en az bir entrance ve en az bir platform
olup pathway grafiği bulunmayan feed'leri saymak; pathway yokluğunu otomatik hata
olarak değil, yalnızca veri kapsamı uyarısı olarak raporlamak. Bu ölçüm yapılmadan
registry'ye yeni kural eklenmeyecek.

Kaynaklar:

- https://gtfs.org/getting-started/features/pathways/
- https://gtfs.org/documentation/schedule/reference/#pathwaystxt
- https://gtfs.org/getting-started/features/accessibility/
