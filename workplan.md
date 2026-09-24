# Python paketi ve PyPI yayını — çalışma planı

## Amaç

Mevcut Rust tabanlı GTFS Analyzer motorunu Python kullanıcılarının şu şekilde
kurup kullanabileceği bir pakete dönüştürmek:

```bash
pip install gtfs-analyzer
```

İlk sürüm yeni bir doğrulama motoru yazmayacak. `gtfs-analyzer` Cargo CLI,
`gtfs-sdk` ve Python paketi aynı Rust pipeline'ını paylaşacak. Python wheel'ı
PyO3/maturin ile native modül olarak üretilecek; kullanıcıdan ayrıca Cargo CLI
kurması beklenmeyecek.

## Mevcut dağıtımlar

- `cargo install gtfs-analyzer`: Rust CLI; JSON veya insan okunabilir çıktı.
- `npm install gtfs-sdk`: JavaScript/TypeScript SDK; Rust/WebAssembly motorunu
  tarayıcıda ve Node.js'te çalıştırır.
- Python paketi geliştirme aşamasında. `pyproject.toml`, `crates/python/` ve
  `python/gtfs_analyzer/` iskeleti eklendi; yayın workflow'u henüz yok.

## Kararlar ve hesap durumu

- PyPI proje adı: `gtfs-analyzer`
- Python paket sürümü bağımsız bir bump hattı olmayacak; engine/Cargo sürümünü
  izleyecek.
- npm SDK sürümü ayrı kalabilir (`gtfs-sdk` şu anda `0.5.0`); mevcut SDK
  `check-version.mjs` engine sürümünü ayrıca doğrular.
- Beklenen kurulum: `pip install gtfs-analyzer`
- Beklenen import adı: `gtfs_analyzer`
- GitHub deposu: `ttezer/gtfs-analyzer`
- PyPI hesabında iki faktörlü doğrulama açıldı.
- PyPI Trusted Publisher için bekleyen kayıt oluşturuldu:
  - Owner: `ttezer`
  - Repository: `gtfs-analyzer`
  - Workflow: `publish-python.yml`
  - Environment: boş
- TestPyPI hesabı ve Trusted Publisher kaydı şimdilik ertelendi. Gerçek PyPI
  yayını öncesinde test yayını yapılacaksa ayrıca açılmalıdır; TestPyPI ayrı
  bir hesap ve ayrı bir publisher kaydı kullanır.

## Uygulama adımları

### 1. Python paket iskeleti

- `pyproject.toml` ekle.
- Paket metadata'sını MIT lisansına, GitHub deposuna ve `gtfs-analyzer`
  projesine bağla.
- Python sürüm aralığını ve desteklenen platformları açıkça belirle.
- `src/gtfs_analyzer/` düzenini kullan.
- Kullanıcıya sunulacak ilk API'yi belirle; en azından:
  - `validate(path, options=None)` veya eşdeğer bir açık fonksiyon,
  - JSON sonucu Python sözlüğü/listeleri olarak döndürme,
  - stdout metnini ayrıştırmak yerine CLI'nin `--json` çıktısını kullanma.

### 2. Rust native modülü ve Python API'si

- `crates/python` PyO3 modülü `gtfs_pipeline::validate_bytes` fonksiyonunu
  çağırmalı; ayrı bir doğrulama mantığı kopyalamamalı.
- `python/gtfs_analyzer` path/bytes girdi, `today`, config delta ve
  `include_name_index` seçeneklerini sunmalı.
- `ValidateResult::Fatal` Python `ValidationError` olarak, başarılı sonuçlar
  sözlük olarak dönmeli.
- JSON sonucu CLI ve SDK ile aynı şemayı korumalı; `validation_status` ve
  `PARTIAL` durumları kaybedilmemeli.
- Native modülün yerel derlemesi ve wheel içindeki import adı her platformda
  doğrulanmalı.

### 3. Testler ve yerel doğrulama

- `python -m build` ile sdist ve wheel üret.
- Temiz bir virtualenv içinde wheel kurup `import gtfs_analyzer` çalıştır.
- Küçük temiz feed, notice üreten feed, bozuk ZIP ve binary bulunamaması için
  test ekle.
- Native modülün derlenmesini ve import edilmesini test et.
- `today` ve `gtfs_jp_profile` config aktarımını test et.
- Testlerde sistemde rastgele bulunan başka bir `gtfs-analyzer` binary'sine
  sessizce bağlanma.
- README'ye kurulum, ilk kullanım, sonuç yapısı ve hata davranışını ekle.
- PyPI metadata'sını ve pakete dahil edilen dosyaları kontrol et.

### 4. GitHub Actions yayın workflow'u

`.github/workflows/publish-python.yml` oluştur.

- Yayın yalnızca `v*` release/tag veya açıkça seçilmiş GitHub Release olayında
  çalışmalı; her push'ta PyPI'ye yükleme yapmamalı.
- `permissions: id-token: write` kullan.
- `actions/checkout` sonrası Python kurulumu, build ve metadata kontrolü yap.
- Trusted Publishing ile `pypa/gh-action-pypi-publish` kullan; API token'ı
  repoya veya GitHub secret'a koyma.
- Workflow adı PyPI Trusted Publisher kaydındaki adla birebir aynı olmalı:
  `publish-python.yml`.
- İlk gerçek yayın öncesi workflow'un tag sürümü ile paket sürümünün aynı
  olduğunu fail-closed kontrol et.
- Cargo/npm release akışlarından bağımsız olarak Python paketinin hangi sürümde
  yayımlandığını release notlarına yaz.

### 5. TestPyPI ve gerçek PyPI yayını

- Uygulama ve yerel build tamamlandıktan sonra TestPyPI hesabı aç.
- TestPyPI'de aynı repo/workflow için ayrı Trusted Publisher kaydı oluştur.
- Test wheel/sdist'ini TestPyPI'ye yükle ve temiz virtualenv'de:

  ```bash
  pip install --index-url https://test.pypi.org/simple/ gtfs-analyzer
  ```

- Kurulum, import, temel doğrulama ve README örneğini TestPyPI paketinden
  çalıştır.
- Sorun yoksa gerçek PyPI Trusted Publisher kaydını kullanarak sürüm tag'i ile
  yayınla.
- Yayından sonra `pip index versions gtfs-analyzer`, PyPI proje sayfası ve
  temiz ortam kurulumu ile doğrula.

## Tamamlanma ölçütleri

- Temiz bir Python ortamında `pip install gtfs-analyzer` başarılı.
- `import gtfs_analyzer` başarılı.
- Bir GTFS ZIP'i Python API'si ile doğrulanabiliyor.
- Notice üreten feed exception atmadan yapılandırılmış sonuç döndürüyor.
- Fatal hata ile notice sonucu birbirinden ayrılıyor.
- Wheel/sdist yalnız gerekli dosyaları içeriyor.
- GitHub Actions Trusted Publishing token kullanmadan çalışıyor.
- TestPyPI kurulumu ve gerçek PyPI kurulumu aynı API davranışını veriyor.
- README ve sürüm notları kullanıcıya Python desteğinin kapsamını açıkça
  anlatıyor.

## Mevcut durum

- `crates/python` eklendi ve ortak Rust pipeline'ına bağlandı.
- `pyproject.toml` ve `python/gtfs_analyzer/__init__.py` eklendi.
- `.github/workflows/publish-python.yml` eklendi; yalnızca GitHub Release
  yayımlandığında ve tag `v*` olduğunda Trusted Publishing ile çalışacak
  şekilde ayarlandı. Tag ile `pyproject.toml` ve Python crate sürümü eşleşmezse
  yayın durur.
- `.github/workflows/publish-python-test.yml` eklendi; yalnızca manuel
  `workflow_dispatch` ile TestPyPI’ye yayın yapar. Henüz çalıştırılmadı.
- `.github/workflows/ci.yml` içine `python-package` kapısı eklendi; Rust binding
  testini, release wheel build'ini ve temiz virtualenv API testlerini çalıştırır.
- Pages deploy artık `python-package` kapısı yeşil olmadan başlamaz.
- SDK sürüm kontrolüne Python paketinin engine sürümüyle eşleşme kapısı eklendi.
- `tests/test_python_api.py` eklendi; sonuç şeması, fatal ZIP hatası, geçersiz
  config ve geçersiz takvim tarihi davranışını kapsıyor.
- `cargo check -p gtfs-analyzer-python` başarılı.
- Maturin ile CPython 3.14 macOS ARM64 wheel'ı üretildi ve temiz virtualenv'de
  kurulup smoke test edildi.
- PyO3 `abi3-py39` etkinleştirildi; hedef, platform başına tek wheel ile CPython
  3.9+ desteği.
- Python `today` değeri artık CLI ile uyumlu gerçek takvim tarihi doğrulaması
  yapıyor.
- Yerel ortamda `python3 -m maturin` mevcut değil; wheel build doğrulaması
  için geçici `/tmp` virtualenv kullanıldı.
- Python testleri ve README örnekleri henüz eklenmedi.

## Sürüm politikası

- Tek engine release tag'i `vX.Y.Z` kabul edilir. Mevcut
  `.github/workflows/release.yml` bu tag'i `crates/cli/Cargo.toml` sürümüyle
  karşılaştırır; tag CLI/engine sürümüyle eşleşmezse release durur.
- Aynı engine bump'ında workspace crate'leri, `crates/wasm`,
  `crates/python/Cargo.toml` ve `pyproject.toml` aynı `X.Y.Z` değerine
  getirilmelidir.
- `sdk/src/index.ts` içindeki `ENGINE_VERSION` engine sürümünü izler; mevcut
  `sdk/scripts/check-version.mjs` artık Python sürümünün de engine ile eşit
  olduğunu kontrol eder.
- `sdk/package.json` ve `SDK_VERSION` npm SDK'nin kendi sürümüdür (`0.5.0`);
  bu sürüm engine sürümüyle aynı olmak zorunda değildir.
- Engine release tag'i yayımlandığında Python workflow'u aynı tag'den wheel ve
  sdist üretip PyPI'ye yayınlar. `audit-*` gibi ürün release'i olmayan tag'ler
  Python workflow'unda atlanır.
- Yalnız npm SDK değiştiğinde engine tag'i üretme; npm paketi kendi sürüm bump
  akışıyla yayınlanır. Mevcut repository'de npm publish için ayrı bir workflow
  bulunmadığından, npm yayın adımı ayrıca ve açıkça planlanmalıdır.

### Engine release bump sırası

1. Workspace/Cargo engine sürümlerini ve `crates/python/Cargo.toml` ile
   `pyproject.toml` sürümünü aynı değere bump et.
2. `sdk/src/index.ts` içindeki `ENGINE_VERSION` değerini güncelle.
3. SDK değiştiyse yalnız ayrıca `SDK_VERSION` ve `sdk/package.json` sürümünü
   bump et; `sdk/scripts/check-version.mjs` çalıştır.
4. Cargo, SDK ve Python build/test kapılarını çalıştır.
5. `vX.Y.Z` tag'ini engine sürümüyle aynı oluştur; GitHub Release'i yayınla.
6. Release workflow CLI asset'lerini üretir; Python workflow aynı release'ten
   PyPI dağıtımlarını üretir.

Bu plan içinde Python için ayrı bir `v0.14.0`/`v0.14.1` tag hattı açılmamalı.

## Dikkat edilecek noktalar

- Python paketi Rust motoruyla aynı sürüm sözleşmesini ve JSON şemasını
  belgelemeli; schema değişiklikleri sessizce yutulmamalı.
- Cargo CLI exit code `1` notice anlamına gelebildiği için bunu genel hata gibi
  ele alma.
- PyPI hesabı veya token bilgisi depoya, workplan'a ya da commit mesajına
  yazılmamalı.
- İlk sürümde PyO3 binding'i ile CLI wrapper'ını aynı anda uygulama; önce ince
  wrapper ile dağıtım ve API sözleşmesini doğrula.
