//! First-stop service represented by exact departures or an unphased headway window.
//! A frequency template's stop_times clock time is never treated as a departure.

use super::*;
use crate::k2::frequencies::FrequencyRecord;

#[derive(Clone, Copy)]
pub(super) struct Window {
    pub start: u32,
    pub end: u32,
    pub headway: u32,
}

pub(super) struct TripService {
    pub exact: Vec<u32>,
    pub windows: Vec<Window>,
    pub valid: bool,
}

pub(super) struct FrequencyIndex<'a> {
    rows: HashMap<&'a str, Vec<&'a FrequencyRecord>>,
}

impl<'a> FrequencyIndex<'a> {
    pub(super) fn new(records: &'a EntityRecords) -> Self {
        let mut rows: HashMap<&str, Vec<&FrequencyRecord>> = HashMap::new();
        for row in &records.frequencies {
            rows.entry(row.trip_id.as_str()).or_default().push(row);
        }
        Self { rows }
    }

    pub(super) fn resolve(&self, trip_id: &str, template_departure: u32) -> TripService {
        let Some(rows) = self.rows.get(trip_id) else {
            return TripService {
                exact: vec![template_departure],
                windows: Vec::new(),
                valid: true,
            };
        };
        let mut exact = Vec::new();
        let mut windows = Vec::new();
        let mut periods = Vec::new();
        for row in rows {
            let (Some(start), Some(end), Some(headway)) =
                (row.start_time, row.end_time, row.headway_secs)
            else {
                return TripService {
                    exact: Vec::new(),
                    windows: Vec::new(),
                    valid: false,
                };
            };
            let (start, end) = (hms_to_secs(start), hms_to_secs(end));
            if start >= end || headway == 0 {
                return TripService {
                    exact: Vec::new(),
                    windows: Vec::new(),
                    valid: false,
                };
            }
            if periods.iter().any(|&(from, to)| from < end && to > start) {
                return TripService {
                    exact: Vec::new(),
                    windows: Vec::new(),
                    valid: false,
                };
            }
            periods.push((start, end));
            match row.exact_times.unwrap_or(0) {
                0 => windows.push(Window {
                    start,
                    end,
                    headway,
                }),
                1 => {
                    // The GTFS end bound is exclusive, including non-divisible windows.
                    // Guard pathological rows without materializing an unbounded schedule.
                    let count = (u64::from(end - start) - 1) / u64::from(headway) + 1;
                    if count > 100_000 {
                        return TripService {
                            exact: Vec::new(),
                            windows: Vec::new(),
                            valid: false,
                        };
                    }
                    for slot in 0..count {
                        exact.push(start + (slot as u32) * headway);
                    }
                }
                _ => {
                    return TripService {
                        exact: Vec::new(),
                        windows: Vec::new(),
                        valid: false,
                    }
                }
            }
        }
        TripService {
            exact,
            windows,
            valid: true,
        }
    }
}

/// A fixed-to-fixed gap can support an OPR_001 claim only if no unphased service
/// window overlaps its interior. FRQ rules assess the declared window headway.
pub(super) fn fixed_gaps_outside_windows(times: &[u32], windows: &[Window]) -> Vec<(u32, u32)> {
    times
        .windows(2)
        .filter_map(|pair| {
            let (from, to) = (pair[0], pair[1]);
            (!windows
                .iter()
                .any(|window| window.start < to && window.end > from))
            .then_some((from, to))
        })
        .collect()
}

/// Equivalent mean of a pure frequency service. For overlapping windows the rates
/// add, while the occupied time is counted once. Empty gaps between windows do
/// not represent a declared headway and are excluded from this mean.
pub(super) fn declared_mean(windows: &[Window]) -> Option<(u32, u32, u32, f64)> {
    let mut edges = Vec::with_capacity(windows.len() * 2);
    for window in windows {
        let rate = 1.0 / f64::from(window.headway);
        edges.push((window.start, rate));
        edges.push((window.end, -rate));
    }
    edges.sort_by_key(|edge| edge.0);
    let first = edges.first()?.0;
    let last = edges.last()?.0;
    let mut active_rate = 0.0;
    let mut active_seconds = 0.0;
    let mut expected_departures = 0.0;
    for pair in edges.windows(2) {
        active_rate += pair[0].1;
        let span = f64::from(pair[1].0 - pair[0].0);
        if active_rate > 0.0 && span > 0.0 {
            active_seconds += span;
            expected_departures += span * active_rate;
        }
    }
    (expected_departures > 0.0).then(|| {
        (
            (active_seconds / expected_departures).round() as u32,
            first,
            last,
            expected_departures,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_mean_weights_nonoverlapping_windows_and_adds_overlapping_rates() {
        let first = Window {
            start: 8 * 3600,
            end: 9 * 3600,
            headway: 600,
        };
        let second = Window {
            start: 9 * 3600,
            end: 11 * 3600,
            headway: 1200,
        };
        assert_eq!(declared_mean(&[first, second]).unwrap().0, 900);
        assert_eq!(declared_mean(&[first, first]).unwrap().0, 300);
    }
}
