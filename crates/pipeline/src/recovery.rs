use std::collections::HashSet;

/// K1'in dosya keşfi ile sonraki aşamalar arasındaki güvenli kurtarma sözleşmesi.
///
/// Eksik opsiyonel bir dosya boş veri olarak güvenle işlenebilir. Gerçek pipeline K1'in
/// okunamayan veya zorunlu olup eksik kalan dosyalarını `unavailable` olarak taşır; böylece
/// yalnızca gerçekten kullanılamayan dosyaya bağlı alt kurallar durdurulur.
#[derive(Debug, Clone, Copy)]
pub struct FileAvailability<'a> {
    unavailable: &'a [String],
}

impl<'a> FileAvailability<'a> {
    pub fn complete() -> FileAvailability<'static> {
        FileAvailability { unavailable: &[] }
    }

    pub fn from_k1(_present: &HashSet<String>, unavailable: &'a [String]) -> Self {
        Self { unavailable }
    }

    /// Dosya okunabilir durumda mı? Dosyanın hiç bulunmaması, opsiyonel bir dosya için
    /// boş veri olarak işlenebildiğinden bu sonucu false yapmaz.
    pub fn available(&self, file: &str) -> bool {
        !self.unavailable.iter().any(|f| f == file)
    }

    pub fn any(&self, files: &[&str]) -> bool {
        files.iter().any(|file| self.available(file))
    }

    pub fn all(&self, files: &[&str]) -> bool {
        files.iter().all(|file| self.available(file))
    }
}
