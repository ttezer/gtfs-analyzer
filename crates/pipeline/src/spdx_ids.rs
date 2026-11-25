//! SPDX License List IDs generated at build time from the pinned official JSON file.
include!(concat!(env!("OUT_DIR"), "/spdx_ids.rs"));

pub(crate) fn contains(id: &str) -> bool {
    let normalized = id.to_ascii_lowercase();
    IDS.binary_search(&normalized.as_str()).is_ok()
}
