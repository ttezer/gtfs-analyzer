//! Diagnostic heap accounting (feature `alloc-stats`, native only). Counts the bytes the
//! program really holds, unlike RSS, which on macOS keeps freed pages until memory pressure
//! and varied by ~500 MB between identical runs. Not for release builds: every allocation
//! pays two atomic operations.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static WINDOW_PEAK: AtomicUsize = AtomicUsize::new(0);

struct Counting;

impl Counting {
    fn grow(n: usize) {
        let live = LIVE.fetch_add(n, Relaxed) + n;
        PEAK.fetch_max(live, Relaxed);
        WINDOW_PEAK.fetch_max(live, Relaxed);
    }
}

// SAFETY: every call is forwarded unchanged to `System`; only counters are updated.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = System.alloc(layout);
        if !p.is_null() {
            Self::grow(layout.size());
        }
        p
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let p = System.alloc_zeroed(layout);
        if !p.is_null() {
            Self::grow(layout.size());
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        LIVE.fetch_sub(layout.size(), Relaxed);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = System.realloc(ptr, layout, new_size);
        if !p.is_null() {
            // Count the move as old + new for its duration: a growing Vec briefly holds both.
            Self::grow(new_size);
            LIVE.fetch_sub(layout.size(), Relaxed);
        }
        p
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Live heap bytes, peak since start, and peak since the previous call (the window resets).
pub(crate) fn snapshot() -> (usize, usize, usize) {
    let live = LIVE.load(Relaxed);
    (live, PEAK.load(Relaxed), WINDOW_PEAK.swap(live, Relaxed))
}
