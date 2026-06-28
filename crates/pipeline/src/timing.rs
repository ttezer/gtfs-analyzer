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

#[cfg(not(target_family = "wasm"))]
pub(crate) struct Timer {
    label: String,
    start: std::time::Instant,
}

#[cfg(not(target_family = "wasm"))]
impl Timer {
    pub(crate) fn start(label: impl Into<String>) -> Self {
        Self { label: label.into(), start: std::time::Instant::now() }
    }
}

#[cfg(not(target_family = "wasm"))]
impl Drop for Timer {
    fn drop(&mut self) {
        if std::env::var_os("GTFS_QUIET").is_none() {
            eprintln!("[timing] {}: {}ms", self.label, self.start.elapsed().as_millis());
        }
    }
}

/// #15 Adım-2: K2-içi peak attribution. WASM linear memory high-water (MB) konsola basar.
/// Bellek yalnız büyür → her etiket o ana dek erişilen tepe. Native'de no-op.
#[cfg(target_arch = "wasm32")]
pub(crate) fn mem_log(label: &str) {
    let pages = core::arch::wasm32::memory_size(0);
    let mb = (pages as f64) * 65536.0 / 1_048_576.0;
    web_sys::console::log_1(&format!("[mem] {label}: {mb:.1} MB").into());
}
// Rust core henüz wasm64 için wasm32::memory_size eşdeğeri sunmuyor. Memory64
// smoke testinde yanlış native zaman/bellek yoluna düşmek yerine teşhisi no-op yap.
#[cfg(target_arch = "wasm64")]
pub(crate) fn mem_log(_label: &str) {}
#[cfg(not(target_family = "wasm"))]
pub(crate) fn mem_log(_label: &str) {}

/// #15: serbest teşhis satırı (yapı-boyutu kırılımı vb.). high-water DEĞİL — çağıran
/// gerçek `len × size_of` canlı boyutu hesaplar. Native'de no-op.
#[cfg(target_family = "wasm")]
pub(crate) fn mem_note(msg: &str) {
    web_sys::console::log_1(&format!("[mem] {msg}").into());
}
#[cfg(not(target_family = "wasm"))]
pub(crate) fn mem_note(_msg: &str) {}

#[cfg(target_family = "wasm")]
pub(crate) struct Timer(String);

#[cfg(target_family = "wasm")]
impl Timer {
    pub(crate) fn start(label: impl Into<String>) -> Self {
        let s: String = label.into();
        web_sys::console::time_with_label(&s);
        Self(s)
    }
}

#[cfg(target_family = "wasm")]
impl Drop for Timer {
    fn drop(&mut self) {
        web_sys::console::time_end_with_label(&self.0);
    }
}
