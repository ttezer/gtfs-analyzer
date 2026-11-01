use std::collections::HashSet;

/// Safe recovery contract between K1 file discovery and later stages.
///
/// A missing optional file can safely be processed as empty data. The real pipeline carries
/// K1 files that are unreadable or required but missing as `unavailable`; only downstream
/// rules that truly depend on an unavailable file are stopped.
#[derive(Debug, Clone, Copy)]
pub struct FileAvailability<'a> {
    /// `None` is the compatibility mode used by direct K4/K5/K6 callers and tests:
    /// their in-memory records are the complete input inventory.
    present: Option<&'a HashSet<String>>,
    unavailable: &'a [String],
}

impl<'a> FileAvailability<'a> {
    pub fn complete() -> FileAvailability<'static> {
        FileAvailability {
            present: None,
            unavailable: &[],
        }
    }

    pub fn from_k1(present: &'a HashSet<String>, unavailable: &'a [String]) -> Self {
        Self {
            present: Some(present),
            unavailable,
        }
    }

    /// Is the file readable? A file being absent does not make this false because an optional
    /// file can be processed as empty data.
    pub fn available(&self, file: &str) -> bool {
        !self.unavailable.iter().any(|f| f == file)
    }

    /// Is the file present in the inventory? In `complete()` calls, all synthetic records
    /// are considered present; in the real K1 flow, it uses the files actually seen in the ZIP.
    pub fn present(&self, file: &str) -> bool {
        self.present.is_none_or(|files| files.contains(file))
    }

    /// Is the file both actually present and readable?
    pub fn present_and_available(&self, file: &str) -> bool {
        self.present(file) && self.available(file)
    }

    /// Was this object created from the real K1 file inventory?
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
