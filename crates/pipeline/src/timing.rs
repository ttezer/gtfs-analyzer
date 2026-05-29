/// Granüler zamanlama yardımcısı.
///
/// Native: Drop anında stderr'e "[timing] label: Xms" basar.
/// WASM:   web_sys::console::time_with_label / time_end_with_label kullanır
///         → Chrome DevTools Performance sekmesinde görünür.
///
/// Kullanım:
///   let _t = Timer::start("K6::speed_and_duration");
///   // ... iş ...
///   // _t drop olduğunda otomatik ölçüm basılır

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct Timer {
    label: String,
    start: std::time::Instant,
}

#[cfg(not(target_arch = "wasm32"))]
impl Timer {
    pub(crate) fn start(label: impl Into<String>) -> Self {
        Self { label: label.into(), start: std::time::Instant::now() }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for Timer {
    fn drop(&mut self) {
        if std::env::var_os("GTFS_QUIET").is_none() {
            eprintln!("[timing] {}: {}ms", self.label, self.start.elapsed().as_millis());
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) struct Timer(String);

#[cfg(target_arch = "wasm32")]
impl Timer {
    pub(crate) fn start(label: impl Into<String>) -> Self {
        let s: String = label.into();
        web_sys::console::time_with_label(&s);
        Self(s)
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for Timer {
    fn drop(&mut self) {
        web_sys::console::time_end_with_label(&self.0);
    }
}
