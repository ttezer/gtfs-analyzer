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
    let max_rss_bytes = if cfg!(target_os = "macos") {
        usage.ru_maxrss as u64
    } else {
        (usage.ru_maxrss as u64).saturating_mul(1024)
    };
    let current_rss_bytes = current_rss_bytes();
    let current = current_rss_bytes
        .map(|bytes| format!("{:.1} MB", bytes as f64 / 1_048_576.0))
        .unwrap_or_else(|| "?".to_owned());
    eprintln!(
        "[mem] {label}: native rss={current}, maxrss={:.1} MB",
        max_rss_bytes as f64 / 1_048_576.0
    );
}

#[cfg(target_os = "macos")]
fn current_rss_bytes() -> Option<u64> {
    // SAFETY: task_info fills the caller-provided mach_task_basic_info structure.
    unsafe {
        let mut info: libc::mach_task_basic_info = std::mem::zeroed();
        let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
        #[allow(deprecated)]
        let task = libc::mach_task_self();
        let result = libc::task_info(
            task,
            libc::MACH_TASK_BASIC_INFO,
            (&mut info as *mut libc::mach_task_basic_info).cast(),
            &mut count,
        );
        (result == libc::KERN_SUCCESS).then_some(info.resident_size as u64)
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn current_rss_bytes() -> Option<u64> {
    // Linux exposes the current resident set in /proc; keep this diagnostic
    // dependency-free for native builds on other Unix hosts.
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let resident_pages = statm.split_whitespace().nth(1)?.parse::<u64>().ok()?;
    Some(resident_pages.saturating_mul(unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 }))
}

#[cfg(all(not(target_family = "wasm"), not(unix)))]
pub(crate) fn mem_log(_label: &str) {}

/// #15: free-form diagnostic line (size breakdown, etc.). NOT high-water — the caller
/// calculates the actual live `len × size_of` size.
#[cfg(all(target_family = "wasm", not(feature = "quiet")))]
pub(crate) fn mem_note(msg: &str) {
    web_sys::console::log_1(&format!("[mem] {msg}").into());
}

/// Diagnostic estimate for a live notice vector. This intentionally reports the
/// vector element storage plus String capacities; allocator metadata is excluded
/// and the result is clearly presented as an estimate rather than RSS.
pub(crate) fn log_notice_memory(label: &str, notices: &[gtfs_core::Notice]) {
    if std::env::var_os("GTFS_QUIET").is_some() {
        return;
    }

    fn string_bytes(value: &String) -> usize {
        value.capacity()
    }
    fn details_parts(
        value: &Option<gtfs_core::NoticeDetails>,
    ) -> (usize, usize, usize, usize) {
        let Some(details) = value else { return (0, 0, 0, 0) };
        let (keys, values) = details.iter().fold((0, 0), |(keys, values), (key, value)| {
            (keys + key.capacity(), values + value.capacity())
        });
        let slots = details.capacity() * std::mem::size_of::<(String, String)>();
        (slots + keys + values, slots, keys, values)
    }

    let flat = notices.len() * std::mem::size_of::<gtfs_core::Notice>();
    let (
        heap,
        title_heap,
        message_heap,
        remediation_heap,
        id_heap,
        rule_heap,
        entity_heap,
        details_heap,
        details_nodes,
        details_keys,
        details_values,
        observed_heap,
        blocks_heap,
    ) = notices.iter().fold(
        (
            0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize,
            0usize, 0usize, 0usize, 0usize,
        ),
        |(
            heap,
            title_heap,
            message_heap,
            remediation_heap,
            id_heap,
            rule_heap,
            entity_heap,
            details_heap,
            details_nodes,
            details_keys,
            details_values,
            observed_heap,
            blocks_heap,
        ), notice| {
            let id = notice.id.capacity();
            let rule = notice.rule_id.capacity();
            let entity = notice.entity_id.as_ref().map_or(0, String::capacity)
                + notice.scope_key.as_ref().map_or(0, String::capacity)
                + notice.file.as_ref().map_or(0, String::capacity)
                + notice.field.as_ref().map_or(0, String::capacity)
                + notice.service_id.as_ref().map_or(0, String::capacity);
            let observed = notice.observed_value.as_ref().map_or(0, String::capacity)
                + notice.expected_value.as_ref().map_or(0, String::capacity);
            let (details, detail_nodes, detail_keys, detail_values) = details_parts(&notice.details);
            let blocks = notice.blocks.capacity() * std::mem::size_of::<String>()
                + notice
                    .blocks
                    .iter()
                    .map(string_bytes)
                    .sum::<usize>();
            let title = notice.title.capacity();
            let message = notice.message.capacity();
            let remediation = notice.remediation.capacity();
            let other = id + rule + entity + observed + details + blocks;
            (
                heap + other + title + message + remediation,
                title_heap + title,
                message_heap + message,
                remediation_heap + remediation,
                id_heap + id,
                rule_heap + rule,
                entity_heap + entity,
                details_heap + details,
                details_nodes + detail_nodes,
                details_keys + detail_keys,
                details_values + detail_values,
                observed_heap + observed,
                blocks_heap + blocks,
            )
        },
    );
    let estimate = flat + heap;
    mem_note(&format!(
        "{label}: notices len={}; flat={:.1}MB; heap={:.1}MB (id={:.1} rule={:.1} entity/file/field={:.1} observed/expected={:.1} details={:.1} [slots={:.1} keys={:.1} values={:.1}] blocks={:.1} title={:.1} message={:.1} remediation={:.1}); allocation estimate={:.1}MB; avg={:.0}B/notice",
        notices.len(),
        flat as f64 / 1_048_576.0,
        heap as f64 / 1_048_576.0,
        id_heap as f64 / 1_048_576.0,
        rule_heap as f64 / 1_048_576.0,
        entity_heap as f64 / 1_048_576.0,
        observed_heap as f64 / 1_048_576.0,
        details_heap as f64 / 1_048_576.0,
        details_nodes as f64 / 1_048_576.0,
        details_keys as f64 / 1_048_576.0,
        details_values as f64 / 1_048_576.0,
        blocks_heap as f64 / 1_048_576.0,
        title_heap as f64 / 1_048_576.0,
        message_heap as f64 / 1_048_576.0,
        remediation_heap as f64 / 1_048_576.0,
        estimate as f64 / 1_048_576.0,
        estimate as f64 / notices.len().max(1) as f64,
    ));
}

/// Diagnostic estimate for the K7 report vectors. It excludes allocator metadata
/// and the already-counted canonical notices.
pub(crate) fn log_report_memory(label: &str, reports: &gtfs_core::ReportSet) {
    if std::env::var_os("GTFS_QUIET").is_some() {
        return;
    }

    fn report_item_bytes(items: &[gtfs_core::ReportItem]) -> usize {
        std::mem::size_of_val(items)
            + items
                .iter()
                .map(|item| item.notice_id.capacity() + item.display_label.capacity())
                .sum::<usize>()
    }
    fn string_vec_bytes(values: &[String]) -> usize {
        std::mem::size_of_val(values)
            + values.iter().map(String::capacity).sum::<usize>()
    }
    fn r9_item_bytes(item: &gtfs_core::R9Item) -> usize {
        std::mem::size_of::<gtfs_core::R9Item>()
            + item.rule_id.capacity()
            + item.labels.capacity() * std::mem::size_of::<gtfs_core::R9Label>()
            + string_vec_bytes(&item.notice_ids)
    }

    let bytes = std::mem::size_of::<gtfs_core::ReportSet>()
        + string_vec_bytes(&reports.r1.blocker_notice_ids)
        + report_item_bytes(&reports.r2.items)
        + report_item_bytes(&reports.r3.items)
        + report_item_bytes(&reports.r4.items)
        + report_item_bytes(&reports.r7.items)
        + report_item_bytes(&reports.r8.items)
        + std::mem::size_of::<gtfs_core::R9Report>()
        + reports
            .r9
            .items
            .iter()
            .map(r9_item_bytes)
            .sum::<usize>();
    mem_note(&format!(
        "{label}: report allocation estimate={:.1}MB (r1={} r2={} r3={} r4={} r7={} r8={} r9={})",
        bytes as f64 / 1_048_576.0,
        reports.r1.blocker_notice_ids.len(),
        reports.r2.items.len(),
        reports.r3.items.len(),
        reports.r4.items.len(),
        reports.r7.items.len(),
        reports.r8.items.len(),
        reports.r9.items.len(),
    ));
}
#[cfg(any(
    not(target_family = "wasm"),
    all(target_family = "wasm", feature = "quiet"),
))]
pub(crate) fn mem_note(msg: &str) {
    if std::env::var_os("GTFS_QUIET").is_none() {
        eprintln!("[mem] {msg}");
    }
}

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
