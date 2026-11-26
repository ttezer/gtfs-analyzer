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

#[derive(Clone, Copy)]
pub(super) struct GapBound {
    pub from: u32,
    pub to: u32,
    /// A window boundary is not itself a confirmed departure. The interval is a
    /// guaranteed lower bound on the actual gap, not an observed trip-to-trip gap.
    pub lower_bound: bool,
}

#[derive(Clone, Copy)]
struct CoverageComponent {
    start: u32,
    end: u32,
    start_fixed: bool,
    end_fixed: bool,
}

/// Collapse fixed departures and frequency windows into service-coverage components.
/// Gaps between components are guaranteed to contain no departure; a window edge
/// yields only a lower bound on the actual trip-to-trip gap.
pub(super) fn service_gap_bounds(times: &[u32], windows: &[Window]) -> Vec<GapBound> {
    let mut coverage: Vec<CoverageComponent> = times
        .iter()
        .map(|&t| CoverageComponent {
            start: t,
            end: t,
            start_fixed: true,
            end_fixed: true,
        })
        .collect();
    coverage.extend(windows.iter().map(|w| CoverageComponent {
        start: w.start,
        end: w.end,
        start_fixed: false,
        end_fixed: false,
    }));
    coverage.sort_unstable_by_key(|item| (item.start, item.end));
    let mut components: Vec<CoverageComponent> = Vec::new();
    for item in coverage {
        if let Some(last) = components.last_mut() {
            if item.start <= last.end {
                if item.start == last.start {
                    last.start_fixed |= item.start_fixed;
                }
                if item.end > last.end {
                    last.end = item.end;
                    last.end_fixed = item.end_fixed;
                } else if item.end == last.end {
                    last.end_fixed |= item.end_fixed;
                }
                continue;
            }
        }
        components.push(item);
    }
    components
        .windows(2)
        .map(|pair| GapBound {
            from: pair[0].end,
            to: pair[1].start,
            lower_bound: !(pair[0].end_fixed && pair[1].start_fixed),
        })
        .collect()
}

/// Estimated route-level mean across the full service span of a pure frequency
/// profile. Overlapping windows add rates, while the span is counted once.
/// Gaps between windows remain in the numerator, as with scheduled profiles.
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
    let mut expected_departures = 0.0;
    for pair in edges.windows(2) {
        active_rate += pair[0].1;
        let span = f64::from(pair[1].0 - pair[0].0);
        if active_rate > 0.0 && span > 0.0 {
            expected_departures += span * active_rate;
        }
    }
    (expected_departures > 0.0).then(|| {
        (
            (f64::from(last - first) / expected_departures).round() as u32,
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
        let morning = Window {
            start: 7 * 3600,
            end: 9 * 3600,
            headway: 600,
        };
        let evening = Window {
            start: 17 * 3600,
            end: 19 * 3600,
            headway: 600,
        };
        assert_eq!(declared_mean(&[morning, evening]).unwrap().0, 1800);
    }

    #[test]
    fn a_fixed_departure_at_a_window_edge_keeps_the_next_gap_exact() {
        let window = Window {
            start: 8 * 3600,
            end: 9 * 3600,
            headway: 600,
        };
        let bounds = service_gap_bounds(&[9 * 3600, 9 * 3600 + 900], &[window]);
        assert_eq!(bounds.len(), 1);
        assert!(!bounds[0].lower_bound);
        let bounds = service_gap_bounds(&[10 * 3600], &[window]);
        assert!(bounds[0].lower_bound);
    }
}
