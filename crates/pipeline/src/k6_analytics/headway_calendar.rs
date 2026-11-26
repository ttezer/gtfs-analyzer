//! OPR_001/003: evaluate one timetable for each distinct active service-day profile.
//! The index holds trip first departures; exact frequency slots are expanded locally.
//! A profile is analysed once regardless of how many dates share its active service set.

use super::*;

#[derive(Default)]
struct ServiceSchedule<'a> {
    departures: Vec<Departure<'a>>,
    patterns: HashMap<(&'a str, &'a str), Vec<Departure<'a>>>,
    windows: Vec<frequency_schedule::Window>,
    pattern_windows: HashMap<(&'a str, &'a str), Vec<frequency_schedule::Window>>,
    unknown_patterns: HashSet<(&'a str, &'a str)>,
    has_unknown: bool,
}

struct Candidate<'a> {
    notice: Notice,
    interval: u32,
    first_date: u32,
    services: Vec<&'a str>,
    before: String,
    after: String,
    direction: &'a str,
}

impl Candidate<'_> {
    fn better_than(&self, other: &Self, largest: bool) -> bool {
        if self.interval != other.interval {
            return if largest {
                self.interval > other.interval
            } else {
                self.interval < other.interval
            };
        }
        (
            self.first_date,
            &self.services,
            &self.before,
            &self.after,
            self.direction,
        ) < (
            other.first_date,
            &other.services,
            &other.before,
            &other.after,
            other.direction,
        )
    }
}

struct Finding<'a> {
    best: Candidate<'a>,
    affected_dates: HashSet<u32>,
}

fn record_finding<'a>(
    findings: &mut BTreeMap<(&'a str, &'static str), Finding<'a>>,
    route: &'a str,
    rule: &'static str,
    candidate: Candidate<'a>,
    dates: &[u32],
) {
    use std::collections::btree_map::Entry;
    match findings.entry((route, rule)) {
        Entry::Vacant(v) => {
            v.insert(Finding {
                best: candidate,
                affected_dates: dates.iter().copied().collect(),
            });
        }
        Entry::Occupied(mut o) => {
            let finding = o.get_mut();
            finding.affected_dates.extend(dates);
            if candidate.better_than(&finding.best, rule == "OPR_001") {
                finding.best = candidate;
            }
        }
    }
}

fn notice_context(
    notice: &mut Notice,
    departures: &[Departure<'_>],
    pair: (u32, u32),
    direction: &str,
    service_ids: &[&str],
    sample_date: u32,
    unique_departures: usize,
) {
    headway_details(
        notice,
        departures,
        pair.0,
        pair.1,
        direction,
        unique_departures,
    );
    notice.service_id = (service_ids.len() == 1).then(|| service_ids[0].to_string());
    let details = notice.details.get_or_insert_with(Default::default);
    details.insert("sample_date", sample_date.to_string());
    details.insert("service_ids", serde_json::to_string(service_ids).unwrap());
}

fn ymd(date: (u32, u32, u32)) -> u32 {
    date.0 * 10_000 + date.1 * 100 + date.2
}

/// Interval in minutes for OPR_003 text and `observed_value`. Whole minutes as before; an
/// interval under one minute keeps two decimals, so 15 s reads "0.25" instead of "0".
fn interval_minutes(seconds: u32) -> String {
    if seconds < 60 {
        format!("{:.2}", seconds as f64 / 60.0)
    } else {
        format!("{:.0}", seconds as f64 / 60.0)
    }
}

pub(super) fn sample_date(records: &EntityRecords, dates: &[u32]) -> u32 {
    if let Some(info) = records.feed_info.first() {
        let start = info.feed_start_date.map(ymd).unwrap_or(0);
        let end = info.feed_end_date.map(ymd).unwrap_or(u32::MAX);
        if let Some(&date) = dates.iter().find(|&&date| date >= start && date <= end) {
            return date;
        }
    }
    dates[0]
}

pub(super) fn direction_key(direction: Option<u32>) -> &'static str {
    match direction {
        Some(0) => "0",
        Some(1) => "1",
        _ => "",
    }
}

/// Group calendar dates by the active services present in one route/direction.
/// Only date and service IDs are indexed; no trip rows are copied for each day.
pub(super) fn active_service_profiles<'a>(
    services: impl IntoIterator<Item = &'a str>,
    active_dates: &HashMap<String, HashSet<u32>>,
) -> BTreeMap<Vec<&'a str>, Vec<u32>> {
    let mut by_date: HashMap<u32, Vec<&str>> = HashMap::new();
    for service in services {
        if let Some(dates) = active_dates.get(service) {
            for &date in dates {
                by_date.entry(date).or_default().push(service);
            }
        }
    }
    let mut profiles: BTreeMap<Vec<&str>, Vec<u32>> = BTreeMap::new();
    for (date, mut services) in by_date {
        services.sort_unstable();
        services.dedup();
        profiles.entry(services).or_default().push(date);
    }
    for dates in profiles.values_mut() {
        dates.sort_unstable();
    }
    profiles
}

fn distinct_times(departures: &[Departure<'_>]) -> Vec<u32> {
    let mut times: Vec<u32> = departures.iter().map(|&(time, _)| time).collect();
    times.sort_unstable();
    times.dedup();
    times
}

fn profile_departures<'a>(
    services: &[&str],
    schedules: &HashMap<&str, ServiceSchedule<'a>>,
) -> Vec<Departure<'a>> {
    let mut departures = Vec::new();
    for service in services {
        departures.extend_from_slice(&schedules[service].departures);
    }
    departures
}

fn trip_at(departures: &[Departure<'_>], time: u32) -> String {
    departures
        .iter()
        .filter(|&&(t, _)| t == time)
        .map(|&(_, trip)| trip)
        .min()
        .unwrap_or("")
        .to_string()
}

pub(super) fn check_route_headway<'a>(
    records: &'a EntityRecords,
    derived: &DerivedData,
    config: &ValidatorConfig,
    idx: &StopTimesIndex<'a>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) -> HeadwayCoverage {
    let ti = &records.trip_interns;
    let frequency_index = frequency_schedule::FrequencyIndex::new(records);
    let labels: HashMap<&str, &str> = records
        .routes
        .iter()
        .map(|r| {
            (
                r.route_id.as_str(),
                r.route_short_name
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(r.route_id.as_str()),
            )
        })
        .collect();
    let rail_routes: HashSet<&str> = records
        .routes
        .iter()
        .filter(|r| r.route_type.is_some_and(is_rail_route_type))
        .map(|r| r.route_id.as_str())
        .collect();
    let raw_routes: HashSet<&str> = records.routes.iter().map(|r| r.route_id.as_str()).collect();
    let rail_trimmed: HashMap<&str, bool> = records
        .routes
        .iter()
        .filter(|r| r.route_type.is_some_and(is_rail_route_type))
        .map(|r| (r.route_id.trim(), r.route_id != r.route_id.trim()))
        .collect();

    // Route locality keeps the temporary merged departures bounded by the largest
    // route/direction/profile, rather than the entire feed or every calendar day.
    let mut routes: BTreeMap<(&str, &str), HashMap<&str, ServiceSchedule<'_>>> = BTreeMap::new();
    for trip in &records.trips {
        let route = ti.route_id(trip);
        let service = ti.service_id(trip);
        if route.is_empty()
            || !derived
                .calendar_bitmap
                .active_dates
                .get(service)
                .is_some_and(|dates| !dates.is_empty())
        {
            continue;
        }
        let Some(&departure) = idx.trip_first_dep.get(trip.trip_id.as_str()) else {
            continue;
        };
        let direction = direction_key(trip.direction_id);
        let stops = idx.by_trip.get(trip.trip_id.as_str());
        let first_stop = stops
            .and_then(|v| v.first())
            .map(|s| idx.stop_id_of(s))
            .unwrap_or("");
        let pattern = ti
            .shape_id(trip)
            .filter(|s| !s.is_empty())
            .or_else(|| stops.and_then(|v| v.last()).map(|s| idx.stop_id_of(s)))
            .unwrap_or("");
        let schedule = routes
            .entry((route, direction))
            .or_default()
            .entry(service)
            .or_default();
        let model = frequency_index.resolve(trip.trip_id.as_str(), departure);
        let key = (first_stop, pattern);
        let pattern_departures = schedule.patterns.entry(key).or_default();
        if !model.valid {
            schedule.has_unknown = true;
            schedule.unknown_patterns.insert(key);
            continue;
        }
        for time in model.exact {
            let value = (time, trip.trip_id.as_str());
            schedule.departures.push(value);
            pattern_departures.push(value);
        }
        schedule.windows.extend_from_slice(&model.windows);
        schedule.pattern_windows.entry(key).or_default().extend(model.windows);
    }

    let mut coverage = HeadwayCoverage::default();
    let mut skipped_days: HashSet<u32> = HashSet::new();
    let mut findings: BTreeMap<(&str, &'static str), Finding<'_>> = BTreeMap::new();

    for ((route, direction), schedules) in routes {
        // Date -> active services is only a small calendar index. The departure rows
        // are never copied per date; equal signatures share one analysis.
        let profiles = active_service_profiles(
            schedules.keys().copied(),
            &derived.calendar_bitmap.active_dates,
        );
        for (services, dates) in profiles {
            coverage.route_direction_profiles += 1;
            let example = sample_date(records, &dates);
            let label = labels.get(route).copied().unwrap_or(route);
            let dir_label = if direction.is_empty() { "-" } else { direction };
            let service_label = services.join(", ");
            let unknown_in_route = services.iter().any(|s| schedules[s].has_unknown);
            let windows: Vec<_> = services.iter().flat_map(|s| schedules[s].windows.iter().copied()).collect();

            if unknown_in_route {
                coverage.opr_001_skipped_profiles += 1;
                skipped_days.extend(&dates);
            } else {
                if !windows.is_empty() {
                    coverage.opr_001_window_profiles += 1;
                }
                let departures = profile_departures(&services, &schedules);
                coverage.merged_departure_rows += departures.len() as u64;
                coverage.largest_merged_departures = coverage
                    .largest_merged_departures
                    .max(departures.len() as u32);
                let times = distinct_times(&departures);
                if times.len() + windows.len() >= 2 {
                    let bounds = frequency_schedule::service_gap_bounds(&times, &windows);
                    let gaps: Vec<u32> = bounds.iter().map(|gap| gap.to - gap.from).collect();
                    let maximum = gaps.iter().copied().max().unwrap_or(0);
                    // A window's declared headway supplies the regular-service reference
                    // when a guaranteed boundary gap has no second fixed comparison gap.
                    let mut regularity_gaps = gaps.clone();
                    regularity_gaps.extend(windows.iter().map(|window| window.headway));
                    let threshold = if rail_routes.contains(route) {
                        config.max_headway_warning_min_rail * 60
                    } else {
                        config.max_headway_warning_min * 60
                    };
                    if maximum > threshold
                        && !config.rural_route_ids.iter().any(|r| r == route)
                        && !is_regular_headway(&regularity_gaps)
                    {
                        let at = gaps.iter().position(|&g| g == maximum).unwrap();
                        let bound = bounds[at];
                        let pair = (bound.from, bound.to);
                        let message = if bound.lower_bound {
                            format!("Route '{label}' direction {dir_label}, service day {example} ({service_label}): guaranteed service gap of at least {:.0} min between frequency-window or fixed-departure boundaries — threshold {} min.", maximum as f64 / 60.0, threshold / 60)
                        } else if !windows.is_empty() {
                            format!("Route '{label}' direction {dir_label}, service day {example} ({service_label}): a fixed-departure gap outside frequency windows is {:.0} min — threshold {} min.", maximum as f64 / 60.0, threshold / 60)
                        } else if services.len() == 1 {
                            format!("Route '{label}' direction {dir_label}, calendar {}: maximum headway {:.0} min — threshold {} min.", services[0], maximum as f64 / 60.0, threshold / 60)
                        } else {
                            format!("Route '{label}' direction {dir_label}, service day {example} ({service_label}): maximum headway {:.0} min — threshold {} min.", maximum as f64 / 60.0, threshold / 60)
                        };
                        let mut notice = k6_notice(
                            ctr,
                            "OPR_001",
                            EntityType::Route,
                            Some(route.to_string()),
                            Some(route.to_string()),
                            "stop_times.txt",
                            None,
                            None,
                            Some(format!("{:.0}", maximum as f64 / 60.0)),
                            Some(format!("≤ {}", threshold / 60)),
                            message,
                            "Increase peak/off-peak trip frequency or close large service gaps.",
                        );
                        notice_context(
                            &mut notice,
                            &departures,
                            pair,
                            direction,
                            &services,
                            example,
                            times.len(),
                        );
                        if !windows.is_empty() {
                            let details = notice.details.get_or_insert_with(Default::default);
                            details.insert("frequency_window_basis", if bound.lower_bound { "lower_bound" } else { "fixed_gap_outside_windows" }.to_string());
                            details.remove("departures");
                            details.insert("exact_departure_times", times.len().to_string());
                            details.insert("frequency_windows", windows.len().to_string());
                            if bound.lower_bound {
                                details.remove("trip_before");
                                details.remove("trip_after");
                            }
                        }
                        if !raw_routes.contains(route)
                            && maximum <= config.max_headway_warning_min_rail * 60
                        {
                            if let Some(&padded) = rail_trimmed.get(route.trim()) {
                                mark_whitespace_join(
                                    &mut notice,
                                    if padded { "routes.txt" } else { "trips.txt" },
                                );
                            }
                        }
                        let candidate = Candidate {
                            notice,
                            interval: maximum,
                            first_date: dates[0],
                            services: services.clone(),
                            before: trip_at(&departures, pair.0),
                            after: trip_at(&departures, pair.1),
                            direction,
                        };
                        record_finding(&mut findings, route, "OPR_001", candidate, &dates);
                    }
                }
            }

            let mut patterns: HashMap<(&str, &str), Vec<Departure<'_>>> = HashMap::new();
            let mut unknown_patterns: HashSet<(&str, &str)> = HashSet::new();
            let mut window_patterns: HashSet<(&str, &str)> = HashSet::new();
            for service in &services {
                for (&key, departures) in &schedules[service].patterns {
                    patterns
                        .entry(key)
                        .or_default()
                        .extend_from_slice(departures);
                }
                unknown_patterns.extend(&schedules[service].unknown_patterns);
                window_patterns.extend(schedules[service].pattern_windows.iter().filter(|(_, windows)| !windows.is_empty()).map(|(&key, _)| key));
            }
            let mut patterns: Vec<_> = patterns.into_iter().collect();
            coverage.pattern_profiles += patterns.len() as u32;
            patterns.sort_unstable_by_key(|(key, _)| *key);
            for ((first_stop, pattern), departures) in patterns {
                if unknown_patterns.contains(&(first_stop, pattern)) {
                    coverage.opr_003_skipped_pattern_profiles += 1;
                    continue;
                }
                let has_window = window_patterns.contains(&(first_stop, pattern));
                if has_window {
                    coverage.opr_003_window_pattern_profiles += 1;
                }
                coverage.merged_departure_rows += departures.len() as u64;
                coverage.largest_merged_departures = coverage
                    .largest_merged_departures
                    .max(departures.len() as u32);
                let times = distinct_times(&departures);
                if times.len() < 2 {
                    continue;
                }
                let minimum = times.windows(2).map(|w| w[1] - w[0]).min().unwrap();
                if minimum == 0 || minimum >= config.bunching_threshold_min * 60 {
                    continue;
                }
                let at = times
                    .windows(2)
                    .position(|w| w[1] - w[0] == minimum)
                    .unwrap();
                let pair = (times[at], times[at + 1]);
                let message = if has_window {
                    format!("Route '{label}' direction {dir_label}, service day {example} ({service_label}): two fixed departures are {} min apart ≤ bunching threshold {} min.", interval_minutes(minimum), config.bunching_threshold_min)
                } else if services.len() == 1 {
                    format!("Route '{label}' direction {dir_label}, calendar {}: minimum headway {} min ≤ bunching threshold {} min.", services[0], interval_minutes(minimum), config.bunching_threshold_min)
                } else {
                    format!("Route '{label}' direction {dir_label}, service day {example} ({service_label}): minimum headway {} min ≤ bunching threshold {} min.", interval_minutes(minimum), config.bunching_threshold_min)
                };
                let mut notice = k6_notice(
                    ctr,
                    "OPR_003",
                    EntityType::Route,
                    Some(route.to_string()),
                    Some(route.to_string()),
                    "stop_times.txt",
                    None,
                    None,
                    Some(interval_minutes(minimum)),
                    Some(format!("> {}", config.bunching_threshold_min)),
                    message,
                    "Adjust the trip schedule; very frequent trips may cause congestion.",
                );
                notice_context(
                    &mut notice,
                    &departures,
                    pair,
                    direction,
                    &services,
                    example,
                    times.len(),
                );
                if has_window {
                    let details = notice.details.get_or_insert_with(Default::default);
                    details.insert("frequency_window_basis", "fixed_pair".to_string());
                    details.remove("departures");
                    details.insert("exact_departure_times", times.len().to_string());
                }
                if !first_stop.is_empty() {
                    notice
                        .details
                        .get_or_insert_with(Default::default)
                        .insert("stop_id", first_stop.to_string());
                }
                let candidate = Candidate {
                    notice,
                    interval: minimum,
                    first_date: dates[0],
                    services: services.clone(),
                    before: trip_at(&departures, pair.0),
                    after: trip_at(&departures, pair.1),
                    direction,
                };
                record_finding(&mut findings, route, "OPR_003", candidate, &dates);
            }
        }
    }

    coverage.opr_001_skipped_days = skipped_days.len() as u32;
    for (_, mut finding) in findings {
        finding
            .best
            .notice
            .details
            .get_or_insert_with(Default::default)
            .insert("affected_days", finding.affected_dates.len().to_string());
        notices.push(finding.best.notice);
    }
    coverage
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nelson_services_have_fifteen_minute_combined_intervals() {
        let mut schedules = HashMap::new();
        schedules.insert(
            "base",
            ServiceSchedule {
                departures: vec![(8 * 3600, "a"), (8 * 3600 + 1800, "b"), (9 * 3600, "c")],
                ..Default::default()
            },
        );
        schedules.insert(
            "peak",
            ServiceSchedule {
                departures: vec![(8 * 3600 + 900, "d"), (8 * 3600 + 2700, "e")],
                ..Default::default()
            },
        );
        let times = distinct_times(&profile_departures(&["base", "peak"], &schedules));
        assert_eq!(times, vec![28800, 29700, 30600, 31500, 32400]);
        assert_eq!(
            times.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>(),
            vec![900; 4]
        );
    }
}
