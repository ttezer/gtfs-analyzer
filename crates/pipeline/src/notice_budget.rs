use gtfs_core::Notice;

/// Hard upper bound for any producer-side notice collection. WASM applies the
/// same bound when stages are combined; keeping it here too prevents a single
/// noisy rule from building an unbounded stage-local Vec first.
pub const MAX_STAGE_NOTICES: usize = 1_000_000;

pub fn push(notices: &mut Vec<Notice>, notice: Notice) {
    if notices.len() < MAX_STAGE_NOTICES {
        notices.push(notice);
    }
}

pub fn extend(notices: &mut Vec<Notice>, incoming: Vec<Notice>) {
    let remaining = MAX_STAGE_NOTICES.saturating_sub(notices.len());
    notices.extend(incoming.into_iter().take(remaining));
}
