//! Zip-bomb / sınırsız-decompression koruması.
//!
//! `GuardedReader`, sardığı `Read` kaynağından **gerçekten açılan** baytları sayar
//! ve bir arşiv girdisi ya da tüm arşiv traversal'ı sınırları aşarsa okumayı durdurur.
//! Kontrol inflate sırasında (post-hoc değil) yapılır: sınır aşıldığı anda `read`
//! hata döndürür, böylece OOM gerçekleşmeden önce akış kesilir.
//!
//! Tasarım notları:
//! - Ratio paydası daima `zf.compressed_size()` (elimizdeki gerçek bayt); beyan edilen
//!   açılmış boyut (`zf.size()`) saldırgan-kontrollü olduğundan asla kullanılmaz.
//! - `ratio_floor`: küçük ama çok sıkışan meşru girdilerde (ör. tekrarlı integer CSV)
//!   yanlış-pozitifi önlemek için oran kontrolü yalnızca bu boyutun üstünde uygulanır.
//! - Sabitler `DEFAULT_DECOMPRESSION_LIMITS` placeholder'dır; gerçek ~9.4 GB feed
//!   ölçümü + wasm64 bellek zarfına göre kalibre edilmelidir.

use std::io::{self, Read};

/// Decompression guard sınırları (bayt cinsinden, oran birimsiz).
#[derive(Clone, Copy, Debug)]
pub struct DecompressionLimits {
    /// Tek bir arşiv traversal'ında toplam açılmış bayt için mutlak tavan.
    pub max_total_decompressed: u64,
    /// Tek bir girdi için açılmış bayt mutlak tavanı.
    pub max_entry_decompressed: u64,
    /// Bu açılmış-boyutun altında oran kontrolü uygulanmaz (küçük girdiler zararsız).
    pub ratio_floor: u64,
    /// Taban üstünde izin verilen en yüksek açılmış:sıkıştırılmış oranı.
    pub max_ratio: u64,
}

/// Decompression guard varsayılan sınırları — 2026-07 gerçek bir 419 MB DELFI
/// aggregate feed'inin (46,3M stop_times) merkezi-dizin taramasıyla kalibre edildi
/// (issue #46; rapor `list_zip_files` içindeki `log_zip_ratio_report`).
///
/// Ölçülen (per-entry, decompress YOK; guard false-trip ETMEDİ):
/// - en yüksek oran 17,8:1 (bir `.out` artefaktı); en yüksek GTFS dosyası trips.txt 12,3:1,
/// - en büyük girdi stop_times ~2,74 GiB; toplam açılmış ~3,39 GiB; feed oranı ~8,3:1.
///
/// **2026-08-01 — Entur (Norveç) yeni EN-KÖTÜ ORAN oldu; kalibrasyon tabanı güncellendi:**
/// - `calendar_dates.txt` **32,8:1** (31,2 MiB açılmış / 1,0 MiB) — DELFI'deki 17,8:1'in ~1,8×'i
///   ve `ratio_floor`'un (16 MiB) ÜSTÜNDE, yani oran kontrolüne TABİ,
/// - en büyük girdi `shapes.txt` 2.651,8 MiB; toplam açılmış 3.640,2 MiB (feed oranı 6,2:1).
///
/// Entur 80 sınırını geçmedi ama pay 4,5×'ten **2,4×**'e düştü. `calendar_dates` yapısı gereği
/// aşırı sıkışır (tekrar eden tarih + service_id) ve Entur'dan 2,4× daha tekrarlı meşru bir feed
/// FATAL ARC_029 ile REDDEDİLİRDİ — mümkün olan en ağır yanlış pozitif. Sınır 80 → 100.
///
/// Değerler ölçülen en-kötünün belirgin üstünde (meşru büyük feed'i reddetmemek için)
/// ama gerçek zip-bomba şekillerini (minik zip → devasa açılım) yakalayacak kadar dar.
/// Yeniden ayarlamak için TEK nokta burasıdır; yeni bir en-kötü feed'de raporu tekrar
/// koştur ve pay bırakarak güncelle.
pub const DEFAULT_DECOMPRESSION_LIMITS: DecompressionLimits = DecompressionLimits {
    // 16 GiB — ölçülen ~3,4 GiB toplamın çok üstünde BİLİNÇLİ tutuldu: daha büyük
    // gerçek feed'ler (~9.4 GB sınıfı) mevcut, "2× toplam" (~7 GiB) onları reddederdi.
    // wasm64 OOM duvarının (~16 GiB) hemen altında. NOT: normal feed'de asıl OOM
    // bariyeri bu DEĞİL (tepe bellek ~14 GiB K6 analitiğinden); bu tavan yalnızca
    // decompression-baskın bombalar içindir.
    max_total_decompressed: 16 * 1024 * 1024 * 1024, // 16 GiB
    // Ölçülen ~2,74 GiB en-büyük girdinin ~4,4×'i. Daha büyük tek girdi zaten 16 GiB
    // duvarında işlenemez → false-trip riski yok; büyük girdilerde asıl kontrol ratio.
    max_entry_decompressed: 12 * 1024 * 1024 * 1024, // 12 GiB
    // Bu açılmış-boyutun altında oran yok sayılır (küçük, çok sıkışan girdiler zararsız).
    ratio_floor: 16 * 1024 * 1024, // 16 MiB
    // Ölçülen en-kötü per-entry oran **32,8:1** (Entur `calendar_dates.txt`, 2026-08-01)
    // → 100 ~3× pay bırakır. Bir bombanın ihtiyacı ~200:1+ ve tek DEFLATE tavanı ~1032:1,
    // yani 100 hâlâ bomba bölgesinin bir kat altında; aradaki boşluğu entry/total cap'leri
    // kapatır (mutlak zarar 12/16 GiB ile sınırlı, oran ne olursa olsun).
    max_ratio: 100,
};

/// Daha küçük wasm32 belleğinde çalışan SDK için decompression sınırları.
///
/// Native CLI'nin büyük feed desteği için kullandığı placeholder sınırları WASM için
/// fazla gevşektir. SDK kendi giriş boyutu kapısını ayrıca uygular; bu sınır ikinci
/// savunma hattıdır ve özellikle K2'nin ZIP'i yeniden açtığı yollar için gereklidir.
pub const SDK_DECOMPRESSION_LIMITS: DecompressionLimits = DecompressionLimits {
    max_total_decompressed: 6 * 1024 * 1024 * 1024,
    max_entry_decompressed: 3 * 1024 * 1024 * 1024,
    ratio_floor: 16 * 1024 * 1024,
    max_ratio: 100,
};

/// K1'de ham Vec'e alınan (stream edilmeyen) tek dosyanın SDK üst sınırı.
pub const SDK_MAX_BUFFERED_FILE_BYTES: u64 = 512 * 1024 * 1024;

/// Bir K2 ZIP stream'inin satır okuyucusuna uygulanacak toplam bütçe.
pub struct DecompressionBudget {
    limits: DecompressionLimits,
    total_decompressed: u64,
}

impl DecompressionBudget {
    pub fn new(limits: DecompressionLimits) -> Self {
        Self { limits, total_decompressed: 0 }
    }

    pub fn reader<'a, R: Read>(&'a mut self, inner: R, compressed_size: u64) -> GuardedReader<'a, R> {
        GuardedReader::new(inner, self.limits, &mut self.total_decompressed, compressed_size)
    }
}

/// Sınır aşımının nedeni (hata mesajı ve teşhis için).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardTrip {
    /// Girdi başına mutlak açılmış bayt tavanı aşıldı.
    EntryCap { got: u64, cap: u64 },
    /// Arşiv geneli toplam açılmış bayt tavanı aşıldı.
    TotalCap { got: u64, cap: u64 },
    /// Taban üstü açılmış:sıkıştırılmış oranı tavanı aştı.
    Ratio { decompressed: u64, compressed: u64, cap: u64 },
}

impl GuardTrip {
    /// İnsan-okur açıklama (FatalError mesajına gömülür).
    pub fn describe(&self) -> String {
        match self {
            GuardTrip::EntryCap { got, cap } =>
                format!("girdi başına açılmış-boyut sınırı aşıldı ({got} > {cap} bayt)"),
            GuardTrip::TotalCap { got, cap } =>
                format!("arşiv geneli açılmış-boyut sınırı aşıldı ({got} > {cap} bayt)"),
            GuardTrip::Ratio { decompressed, compressed, cap } =>
                format!("sıkıştırma oranı {cap}:1 sınırını aştı (açılmış {decompressed}, sıkıştırılmış {compressed} bayt)"),
        }
    }
}

/// Açılan baytları sayan `Read` adaptörü. Sınır aşılınca `read` hata döndürür ve
/// nedeni [`GuardedReader::tripped`] ile okunabilir; çağıran plain ZIP hatasından
/// ayırıp uygun `FatalCode`'u seçer.
///
/// `total` bir traversal'ın tüm girdileri arasında paylaşılır (dış döngüde tutulur).
pub struct GuardedReader<'a, R: Read> {
    inner: R,
    limits: DecompressionLimits,
    total: &'a mut u64,
    entry_compressed: u64,
    entry_decompressed: u64,
    tripped: Option<GuardTrip>,
}

impl<'a, R: Read> GuardedReader<'a, R> {
    pub fn new(inner: R, limits: DecompressionLimits, total: &'a mut u64, entry_compressed: u64) -> Self {
        Self { inner, limits, total, entry_compressed, entry_decompressed: 0, tripped: None }
    }

    /// Sınır aşıldıysa nedenini döndürür (aksi halde `None`).
    pub fn tripped(&self) -> Option<GuardTrip> {
        self.tripped
    }
}

fn tripped_err() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "decompression guard tripped")
}

impl<R: Read> Read for GuardedReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.tripped.is_some() {
            return Err(tripped_err()); // zaten tetiklendi → daha fazla okuma yok
        }
        let n = self.inner.read(buf)?;
        if n == 0 {
            return Ok(0);
        }
        let n64 = n as u64;
        self.entry_decompressed += n64;
        *self.total += n64;

        // Çarpım biçimli oran kontrolü: sıfıra bölme yok, taşma yok. Taban üstü
        // sıfır-sıkıştırılmış girdi sonsuz oran demektir → doğru şekilde tetikler.
        let breach = if self.entry_decompressed > self.limits.max_entry_decompressed {
            Some(GuardTrip::EntryCap { got: self.entry_decompressed, cap: self.limits.max_entry_decompressed })
        } else if *self.total > self.limits.max_total_decompressed {
            Some(GuardTrip::TotalCap { got: *self.total, cap: self.limits.max_total_decompressed })
        } else if self.entry_decompressed > self.limits.ratio_floor
            && self.entry_decompressed > self.limits.max_ratio.saturating_mul(self.entry_compressed)
        {
            Some(GuardTrip::Ratio {
                decompressed: self.entry_decompressed,
                compressed: self.entry_compressed,
                cap: self.limits.max_ratio,
            })
        } else {
            None
        };

        if let Some(t) = breach {
            self.tripped = Some(t);
            return Err(tripped_err());
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_LIMITS: DecompressionLimits = DecompressionLimits {
        max_total_decompressed: 1000,
        max_entry_decompressed: 400,
        ratio_floor: 100,
        max_ratio: 10,
    };

    fn read_all<R: Read>(r: &mut R) -> io::Result<usize> {
        let mut buf = Vec::new();
        r.read_to_end(&mut buf).map(|_| buf.len())
    }

    #[test]
    fn passes_within_limits() {
        let data = vec![0u8; 300];
        let mut total = 0u64;
        // compressed=300 → ratio 1:1, entry 300<400, total 300<1000 → geçer
        let mut g = GuardedReader::new(&data[..], TEST_LIMITS, &mut total, 300);
        assert_eq!(read_all(&mut g).unwrap(), 300);
        assert_eq!(g.tripped(), None);
        assert_eq!(total, 300);
    }

    #[test]
    fn trips_on_entry_cap() {
        let data = vec![0u8; 500];
        let mut total = 0u64;
        // entry 500>400 → EntryCap (compressed büyük, oran tetiklenmez)
        let mut g = GuardedReader::new(&data[..], TEST_LIMITS, &mut total, 500);
        assert!(read_all(&mut g).is_err());
        assert!(matches!(g.tripped(), Some(GuardTrip::EntryCap { .. })));
    }

    #[test]
    fn trips_on_ratio_above_floor() {
        let data = [0u8; 200];
        let mut total = 0u64;
        // entry 200>ratio_floor 100 ve 200 > 10*compressed(1)=10 → Ratio
        let mut g = GuardedReader::new(&data[..], TEST_LIMITS, &mut total, 1);
        assert!(read_all(&mut g).is_err());
        assert!(matches!(g.tripped(), Some(GuardTrip::Ratio { .. })));
    }

    #[test]
    fn small_high_ratio_entry_below_floor_passes() {
        let data = [0u8; 50];
        let mut total = 0u64;
        // 50<=ratio_floor 100 → oran yok sayılır; entry/total altında → geçer
        let mut g = GuardedReader::new(&data[..], TEST_LIMITS, &mut total, 1);
        assert_eq!(read_all(&mut g).unwrap(), 50);
        assert_eq!(g.tripped(), None);
    }

    #[test]
    fn trips_on_total_across_entries() {
        let mut total = 0u64;
        // İki girdi: her biri limit altında ama toplam total_cap'i aşar.
        let d1 = vec![0u8; 350];
        {
            let mut g = GuardedReader::new(&d1[..], TEST_LIMITS, &mut total, 350);
            assert_eq!(read_all(&mut g).unwrap(), 350);
        }
        let d2 = vec![0u8; 350];
        {
            let mut g = GuardedReader::new(&d2[..], TEST_LIMITS, &mut total, 350);
            assert_eq!(read_all(&mut g).unwrap(), 350);
        }
        // total=700 < 1000; üçüncü girdi taşırır
        let d3 = vec![0u8; 350];
        let mut g = GuardedReader::new(&d3[..], TEST_LIMITS, &mut total, 350);
        assert!(read_all(&mut g).is_err());
        assert!(matches!(g.tripped(), Some(GuardTrip::TotalCap { .. })));
    }
}
