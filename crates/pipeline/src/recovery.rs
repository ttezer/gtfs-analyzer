use std::collections::HashSet;

/// K1'in dosya keşfi ile sonraki aşamalar arasındaki güvenli kurtarma sözleşmesi.
///
/// Eksik opsiyonel bir dosya boş veri olarak güvenle işlenebilir. Gerçek pipeline K1'in
/// okunamayan veya zorunlu olup eksik kalan dosyalarını `unavailable` olarak taşır; böylece
/// yalnızca gerçekten kullanılamayan dosyaya bağlı alt kurallar durdurulur.
#[derive(Debug, Clone, Copy)]
pub struct FileAvailability<'a> {
    /// `None` is the compatibility mode used by direct K4/K5/K6 callers and tests:
    /// their in-memory records are the complete input inventory.
    present: Option<&'a HashSet<String>>,
    unavailable: &'a [String],
}

impl<'a> FileAvailability<'a> {
    pub fn complete() -> FileAvailability<'static> {
        FileAvailability { present: None, unavailable: &[] }
    }

    pub fn from_k1(present: &'a HashSet<String>, unavailable: &'a [String]) -> Self {
        Self { present: Some(present), unavailable }
    }

    /// Dosya okunabilir durumda mı? Dosyanın hiç bulunmaması, opsiyonel bir dosya için
    /// boş veri olarak işlenebildiğinden bu sonucu false yapmaz.
    pub fn available(&self, file: &str) -> bool {
        !self.unavailable.iter().any(|f| f == file)
    }

    /// Dosya envanterde mevcut mu? `complete()` çağrılarında sentetik kayıtların tümü
    /// mevcut kabul edilir; gerçek K1 akışında ise ZIP'te gerçekten görülen dosyaları
    /// kullanır.
    pub fn present(&self, file: &str) -> bool {
        self.present.is_none_or(|files| files.contains(file))
    }

    /// Dosya hem gerçekten mevcut hem de okunabilir mi?
    pub fn present_and_available(&self, file: &str) -> bool {
        self.present(file) && self.available(file)
    }

    /// Bu nesne gerçek K1 dosya envanterinden mi üretildi?
    pub fn has_inventory(&self) -> bool {
        self.present.is_some()
    }

    pub fn any(&self, files: &[&str]) -> bool {
        files.iter().any(|file| self.available(file))
    }

    pub fn all(&self, files: &[&str]) -> bool {
        files.iter().all(|file| self.available(file))
    }

    pub fn any_present_and_available(&self, files: &[&str]) -> bool {
        files.iter().any(|file| self.present_and_available(file))
    }

    pub fn all_present_and_available(&self, files: &[&str]) -> bool {
        files.iter().all(|file| self.present_and_available(file))
    }
}

#[cfg(test)]
mod tests {
    use super::FileAvailability;
    use std::collections::HashSet;

    #[test]
    fn inventory_separates_missing_from_unreadable_files() {
        let present = HashSet::from(["calendar.txt".to_string()]);
        let unavailable = vec!["calendar.txt".to_string()];
        let availability = FileAvailability::from_k1(&present, &unavailable);

        assert!(availability.present("calendar.txt"));
        assert!(!availability.available("calendar.txt"));
        assert!(!availability.present_and_available("calendar.txt"));

        assert!(!availability.present("calendar_dates.txt"));
        assert!(availability.available("calendar_dates.txt"));
        assert!(!availability.present_and_available("calendar_dates.txt"));
        assert!(!availability.any_present_and_available(&["calendar.txt", "calendar_dates.txt"]));
    }
}
