//! OPR_005: compare average headways of active route/direction/service-day profiles.
//! One distinct active service set contributes one observation to the existing
//! route-type median/MAD baseline. Calendar-day weighting is a separate method.

use super::*;

#[derive(Default)]
struct ServiceTimes {
    departures: Vec<u32>,
    has_frequency: bool,
}

struct Profile<'a> {
    route: &'a str,
    direction: &'a str,
    services: Vec<&'a str>,
    dates: Vec<u32>,
    average: u32,
    route_type: u32,
    first: u32,
    last: u32,
    count: usize,
}

struct Finding {
    best: usize,
    affected_dates: HashSet<u32>,
}

pub(super) fn check_opr005<'a>(
    records: &'a EntityRecords,
    derived: &DerivedData,
    config: &ValidatorConfig,
    idx: &StopTimesIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) -> HeadwayCoverage {
    let ti = &records.trip_interns;
    let frequency_trips: HashSet<&str> = records
        .frequencies
        .iter()
        .map(|f| f.trip_id.as_str())
        .collect();
    let route_labels: HashMap<&str, &str> = records
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
    let route_types: HashMap<&str, u32> = records
        .routes
        .iter()
        .map(|r| (r.route_id.as_str(), r.route_type.unwrap_or(3)))
        .collect();

    // First departures are retained once per trip. Calendar dates only index service IDs.
    let mut routes: BTreeMap<(&str, &str), HashMap<&str, ServiceTimes>> = BTreeMap::new();
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
        let schedule = routes
            .entry((route, headway_calendar::direction_key(trip.direction_id)))
            .or_default()
            .entry(service)
            .or_default();
        schedule.departures.push(departure);
        schedule.has_frequency |= frequency_trips.contains(trip.trip_id.as_str());
    }

    let mut coverage = HeadwayCoverage::default();
    let mut skipped_days = HashSet::new();
    let mut profiles = Vec::new();
    for ((route, direction), schedules) in routes {
        let date_profiles = headway_calendar::active_service_profiles(
            schedules.keys().copied(),
            &derived.calendar_bitmap.active_dates,
        );
        for (services, dates) in date_profiles {
            if services
                .iter()
                .any(|service| schedules[service].has_frequency)
            {
                coverage.opr_005_skipped_profiles += 1;
                skipped_days.extend(dates);
                continue;
            }
            let mut departures = Vec::new();
            for service in &services {
                departures.extend_from_slice(&schedules[service].departures);
            }
            coverage.opr_005_merged_departure_rows += departures.len() as u64;
            coverage.opr_005_largest_merged_departures = coverage
                .opr_005_largest_merged_departures
                .max(departures.len() as u32);
            departures.sort_unstable();
            departures.dedup();
            if departures.len() < 2 {
                continue;
            }
            coverage.opr_005_evaluated_profiles += 1;
            let first = departures[0];
            let last = departures[departures.len() - 1];
            let count = departures.len();
            // Same arithmetic as the previous consecutive-gap average, without a gap vector.
            let average = (last - first) / (count as u32 - 1);
            profiles.push(Profile {
                route,
                direction,
                services,
                dates,
                average,
                route_type: route_types.get(route).copied().unwrap_or(3),
                first,
                last,
                count,
            });
        }
    }
    coverage.opr_005_skipped_days = skipped_days.len() as u32;

    // Preserve the existing upper median, scaled MAD, 5-observation minimum and threshold.
    let mut by_type: HashMap<u32, Vec<u32>> = HashMap::new();
    for profile in &profiles {
        by_type
            .entry(profile.route_type)
            .or_default()
            .push(profile.average);
    }
    let mut type_stats = HashMap::new();
    for (route_type, mut averages) in by_type {
        if averages.len() < 5 {
            continue;
        }
        averages.sort_unstable();
        let median = averages[averages.len() / 2] as f64;
        let mut deviations: Vec<f64> = averages
            .iter()
            .map(|&average| (average as f64 - median).abs())
            .collect();
        deviations.sort_by(|a, b| a.total_cmp(b));
        let mad = deviations[deviations.len() / 2] * 1.4826;
        type_stats.insert(route_type, (median, mad));
    }

    // The public entity is Route. Select one deterministic example per route before K7 dedup.
    let mut findings: BTreeMap<&str, Finding> = BTreeMap::new();
    for (index, profile) in profiles.iter().enumerate() {
        let Some(&(median, mad)) = type_stats.get(&profile.route_type) else {
            continue;
        };
        let average = profile.average as f64;
        let outlier = if mad < 30.0 {
            !(0.5..=2.0).contains(&(average / median))
        } else {
            (average - median).abs() > config.headway_outlier_sigma * mad
        };
        if !outlier {
            continue;
        }
        use std::collections::btree_map::Entry;
        match findings.entry(profile.route) {
            Entry::Vacant(entry) => {
                entry.insert(Finding {
                    best: index,
                    affected_dates: profile.dates.iter().copied().collect(),
                });
            }
            Entry::Occupied(mut entry) => {
                let finding = entry.get_mut();
                finding.affected_dates.extend(profile.dates.iter().copied());
                let previous = &profiles[finding.best];
                let previous_median = type_stats[&previous.route_type].0;
                let deviation = (average - median).abs();
                let previous_deviation = (previous.average as f64 - previous_median).abs();
                if deviation > previous_deviation
                    || (deviation == previous_deviation
                        && (profile.dates[0], &profile.services, profile.direction)
                            < (previous.dates[0], &previous.services, previous.direction))
                {
                    finding.best = index;
                }
            }
        }
    }

    for (route, finding) in findings {
        let profile = &profiles[finding.best];
        let (median, _) = type_stats[&profile.route_type];
        let average = profile.average as f64;
        let sparse = average > median;
        let kind = if sparse {
            "unusually sparse"
        } else {
            "unusually dense"
        };
        let direction = if profile.direction.is_empty() {
            "-"
        } else {
            profile.direction
        };
        let label = route_labels.get(route).copied().unwrap_or(route);
        let average_min = average / 60.0;
        let median_min = median / 60.0;
        let example = headway_calendar::sample_date(records, &profile.dates);
        let message = if profile.services.len() == 1 {
            format!("Route '{label}' direction {direction}, calendar {}: average headway {average_min:.0} min — the feed median for routes of the same type is {median_min:.0} min ({kind}).", profile.services[0])
        } else {
            format!("Route '{label}' direction {direction}, service day {example} (services {}): average headway {average_min:.0} min — the feed median for routes of the same type is {median_min:.0} min ({kind}).", profile.services.join(", "))
        };
        let mut notice = k6_notice(
            ctr,
            "OPR_005",
            EntityType::Route,
            Some(route.to_string()),
            Some(route.to_string()),
            "stop_times.txt",
            None,
            None,
            Some(format!("{average_min:.0}")),
            None,
            message,
            "Informational notice; this route's frequency is unusual for its mode.",
        );
        notice.service_id = (profile.services.len() == 1).then(|| profile.services[0].to_string());
        let hm = |seconds: u32| format!("{:02}:{:02}", seconds / 3600, seconds / 60 % 60);
        let details = notice.details.get_or_insert_with(Default::default);
        details.insert("median_min", format!("{median_min:.0}"));
        details.insert("kind", if sparse { "sparse" } else { "dense" }.to_string());
        details.insert("first_departure", hm(profile.first));
        details.insert("last_departure", hm(profile.last));
        details.insert("departures", profile.count.to_string());
        details.insert("sample_date", example.to_string());
        details.insert("affected_days", finding.affected_dates.len().to_string());
        details.insert(
            "service_ids",
            serde_json::to_string(&profile.services).unwrap(),
        );
        if !profile.direction.is_empty() {
            details.insert("direction_id", profile.direction.to_string());
        }
        notices.push(notice);
    }
    coverage
}
