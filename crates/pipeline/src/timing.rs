/// Fine-grained timing helper.
///
/// Native: prints "[timing] label: Xms" to stderr when dropped.
/// WASM:   uses web_sys::console::time_with_label / time_end_with_label
///         → visible in the Chrome DevTools Performance tab.
///
/// Usage:
///   let _t = Timer::start("K6::speed_and_duration");
///   // ... work ...
///   // timing is printed automatically when _t is dropped
#[cfg(not(target_family = "wasm"))]
pub(crate) struct Timer {
    label: String,
    start: std::time::Instant,
}

#[cfg(not(target_family = "wasm"))]
impl Timer {
    pub(crate) fn start(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            start: std::time::Instant::now(),
        }
    }
}

#[cfg(not(target_family = "wasm"))]
impl Drop for Timer {
    fn drop(&mut self) {
        if std::env::var_os("GTFS_QUIET").is_none() {
            eprintln!(
                "[timing] {}: {}ms",
                self.label,
                self.start.elapsed().as_millis()
            );
        }
    }
}

/// #15 Step 2: peak attribution inside K2. Prints the WASM linear-memory high-water mark (MB)
/// to the console. Native also writes the process high-water RSS, showing which pipeline
/// stage produced the total `/usr/bin/time` value.
#[cfg(all(target_arch = "wasm32", not(feature = "quiet")))]
pub(crate) fn mem_log(label: &str) {
    let pages = core::arch::wasm32::memory_size(0);
    let mb = (pages as f64) * 65536.0 / 1_048_576.0;
    web_sys::console::log_1(&format!("[mem] {label}: {mb:.1} MB").into());
}
// Rust core does not yet expose a wasm32::memory_size equivalent for wasm64. Make the
// diagnostic a no-op in the Memory64 smoke test.
#[cfg(any(target_arch = "wasm64", all(target_family = "wasm", feature = "quiet")))]
pub(crate) fn mem_log(_label: &str) {}

#[cfg(all(not(target_family = "wasm"), unix))]
pub(crate) fn mem_log(label: &str) {
    if std::env::var_os("GTFS_QUIET").is_some() {
        return;
    }

    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: getrusage initializes the caller-provided rusage structure when it
    // returns success; the zero argument selects this process.
    let result = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if result != 0 {
        return;
    }
    // SAFETY: result == 0 means libc filled the structure.
    let usage = unsafe { usage.assume_init() };
    let rss_bytes = if cfg!(target_os = "macos") {
        usage.ru_maxrss as u64
    } else {
        (usage.ru_maxrss as u64).saturating_mul(1024)
    };
    eprintln!(
        "[mem] {label}: native maxrss={:.1} MB",
        rss_bytes as f64 / 1_048_576.0
    );
}

#[cfg(all(not(target_family = "wasm"), not(unix)))]
pub(crate) fn mem_log(_label: &str) {}

/// #15: free-form diagnostic line (size breakdown, etc.). NOT high-water — the caller
/// calculates the actual live `len × size_of` size. No-op on native.
#[cfg(all(target_family = "wasm", not(feature = "quiet")))]
pub(crate) fn mem_note(msg: &str) {
    web_sys::console::log_1(&format!("[mem] {msg}").into());
}
#[cfg(any(
    not(target_family = "wasm"),
    all(target_family = "wasm", feature = "quiet"),
))]
pub(crate) fn mem_note(_msg: &str) {}

#[cfg(all(target_family = "wasm", feature = "quiet"))]
pub(crate) struct Timer;

#[cfg(all(target_family = "wasm", feature = "quiet"))]
impl Timer {
    pub(crate) fn start(_label: impl Into<String>) -> Self {
        Self
    }
}

#[cfg(all(target_family = "wasm", not(feature = "quiet")))]
pub(crate) struct Timer(String);

#[cfg(all(target_family = "wasm", not(feature = "quiet")))]
impl Timer {
    pub(crate) fn start(label: impl Into<String>) -> Self {
        let s: String = label.into();
        web_sys::console::time_with_label(&s);
        Self(s)
    }
}

#[cfg(all(target_family = "wasm", not(feature = "quiet")))]
impl Drop for Timer {
    fn drop(&mut self) {
        web_sys::console::time_end_with_label(&self.0);
    }
}
