//! `AgencyResolver` sözleşmesi (agency attribution P1).
//!
//! Resolver gerçek K2 kayıtları üzerinde kurulur: küçük feed'ler `validate_bytes_inspected`
//! ile ayrıştırılır, resolver gözlemcinin içinde oluşturulur. Böylece ham (kırpılmamış)
//! kimlikler K2'nin gerçekten sakladığı biçimde sınanır.

use std::io::Write as _;

use gtfs_core::{EntityType, Notice};
use gtfs_pipeline::agency_attribution::{
    AgencyAttribution, AgencyResolver, Resolution, UnattributedReason,
};
use gtfs_pipeline::{validate_bytes, validate_bytes_inspected, ValidateResult, ValidatorConfig};
use zip::write::SimpleFileOptions;

const AGENCIES_AB: &str = "agency_id,agency_name,agency_url,agency_timezone\n\
A,Alpha,http://a.example,UTC\nB,Beta,http://b.example,UTC\n";
const STOPS: &str = "stop_id,stop_name,stop_lat,stop_lon\nS1,One,41.0,29.0\nS2,Two,41.1,29.1\n";
const CALENDAR: &str = "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,\
start_date,end_date\nSVC,1,1,1,1,1,0,0,20250101,20271231\n";

/// Feed'i kurar; `probes` = (kural scope türü, scope_key). Sonuç, sorgu sırasıyla
/// (atıf, atfedilen agency'nin ham kimliği).
fn attribute(
    agency: &str,
    routes: &str,
    trips: &str,
    probes: &[(Option<&str>, Option<&str>)],
) -> Vec<(AgencyAttribution, Option<String>)> {
    let notices: Vec<Notice> = probes.iter().map(|(scope, key)| probe(*scope, *key)).collect();
    attribute_in(agency, routes, trips, &[], &notices)
}

/// Hazır notice'ları iki agency'li küçük bir feed'de atfeder.
fn attribute_notices(notices: &[Notice]) -> Vec<(AgencyAttribution, Option<String>)> {
    attribute_in(
        AGENCIES_AB,
        "route_id,agency_id,route_short_name,route_type\nR1,A,1,3\n",
        "route_id,service_id,trip_id\nR1,SVC,T1\n",
        &[],
        notices,
    )
}

fn attribute_in(
    agency: &str,
    routes: &str,
    trips: &str,
    extra: &[(&str, &str)],
    notices: &[Notice],
) -> Vec<(AgencyAttribution, Option<String>)> {
    let stop_times = {
        let mut rows = String::from("trip_id,arrival_time,departure_time,stop_id,stop_sequence\n");
        for line in trips.lines().skip(1) {
            let trip = line.split(',').nth(2).unwrap();
            rows.push_str(&format!("{trip},08:00:00,08:00:00,S1,1\n{trip},08:10:00,08:10:00,S2,2\n"));
        }
        rows
    };
    let mut files = vec![
        ("agency.txt", agency),
        ("stops.txt", STOPS),
        ("routes.txt", routes),
        ("trips.txt", trips),
        ("stop_times.txt", stop_times.as_str()),
        ("calendar.txt", CALENDAR),
    ];
    files.extend_from_slice(extra);
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in files {
        writer.start_file(name, SimpleFileOptions::default()).unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    let zip = writer.finish().unwrap().into_inner();

    let mut out = Vec::new();
    validate_bytes_inspected(&zip, &ValidatorConfig::default(), 20_260_930, &mut |_, records, _, _| {
        let resolver = AgencyResolver::new(records);
        for notice in notices {
            let attribution = resolver.attribute(notice);
            let id = attribution.agency().map(|a| resolver.agency_id(a).to_string());
            out.push((attribution, id));
        }
    });
    assert_eq!(out.len(), notices.len(), "gözlemci çalışmadı (feed fatal?)");
    out
}

/// Registry'de istenen scope alanını beyan eden ilk kural için sahte notice.
fn probe(scope: Option<&str>, key: Option<&str>) -> Notice {
    let rule = gtfs_rules::RULES
        .iter()
        .find(|m| m.scope_key_field == scope)
        .unwrap_or_else(|| panic!("scope {scope:?} beyan eden kural yok"));
    Notice {
        id: "probe".to_string(),
        rule_id: rule.id.to_string(),
        severity: rule.severity,
        rule_class: rule.rule_class,
        entity_type: EntityType::Row,
        entity_id: None,
        scope_key: key.map(str::to_string),
        file: None,
        line: None,
        field: None,
        observed_value: None,
        expected_value: None,
        details: None,
        whitespace_derived: false,
        whitespace_candidate: false,
        title: String::new(),
        message: String::new(),
        remediation: String::new(),
        blocks: Vec::new(),
        base_effort: rule.base_effort,
        service_id: None,
        agency_distribution: None,
    }
}

fn resolved(id: &str, resolution: Resolution) -> impl Fn(&(AgencyAttribution, Option<String>)) -> bool + '_ {
    move |(attribution, agency)| {
        matches!(attribution, AgencyAttribution::Resolved { resolution: r, .. } if *r == resolution)
            && agency.as_deref() == Some(id)
    }
}

fn unattributed(reason: UnattributedReason) -> AgencyAttribution {
    AgencyAttribution::Unattributed(reason)
}

#[test]
fn single_agency_owns_routes_without_agency_id() {
    let out = attribute(
        "agency_id,agency_name,agency_url,agency_timezone\nA,Alpha,http://a.example,UTC\n",
        "route_id,agency_id,route_short_name,route_type\nR1,,1,3\n",
        "route_id,service_id,trip_id\nR1,SVC,T1\n",
        &[(Some("route_id"), Some("R1")), (Some("trip_id"), Some("T1"))],
    );
    assert!(resolved("A", Resolution::Exact)(&out[0]), "{out:?}");
    assert!(resolved("A", Resolution::Exact)(&out[1]), "{out:?}");
}

#[test]
fn single_agency_without_agency_id_column() {
    let out = attribute(
        "agency_name,agency_url,agency_timezone\nAlpha,http://a.example,UTC\n",
        "route_id,route_short_name,route_type\nR1,1,3\n",
        "route_id,service_id,trip_id\nR1,SVC,T1\n",
        &[(Some("trip_id"), Some("T1"))],
    );
    assert!(resolved("", Resolution::Exact)(&out[0]), "{out:?}");
}

#[test]
fn routes_and_trips_resolve_to_their_own_agency() {
    let out = attribute(
        AGENCIES_AB,
        "route_id,agency_id,route_short_name,route_type\nR1,A,1,3\nR2,B,2,3\n",
        "route_id,service_id,trip_id\nR1,SVC,T1\nR2,SVC,T2\n",
        &[
            (Some("route_id"), Some("R2")),
            (Some("trip_id"), Some("T1")),
            (Some("trip_id"), Some("T2")),
            (Some("agency_id"), Some("B")),
        ],
    );
    assert!(resolved("B", Resolution::Exact)(&out[0]), "{out:?}");
    assert!(resolved("A", Resolution::Exact)(&out[1]), "{out:?}");
    assert!(resolved("B", Resolution::Exact)(&out[2]), "{out:?}");
    assert!(
        matches!(out[3].0, AgencyAttribution::Direct { resolution: Resolution::Exact, .. })
            && out[3].1.as_deref() == Some("B"),
        "{out:?}"
    );
}

#[test]
fn multi_agency_route_without_agency_id_is_not_guessed() {
    let out = attribute(
        AGENCIES_AB,
        "route_id,agency_id,route_short_name,route_type\nR1,,1,3\n",
        "route_id,service_id,trip_id\nR1,SVC,T1\n",
        &[(Some("route_id"), Some("R1")), (Some("trip_id"), Some("T1"))],
    );
    assert_eq!(out[0].0, unattributed(UnattributedReason::RouteWithoutAgency));
    assert_eq!(out[1].0, unattributed(UnattributedReason::RouteWithoutAgency));
}

#[test]
fn broken_references_are_unattributed_with_their_reason() {
    let out = attribute(
        AGENCIES_AB,
        "route_id,agency_id,route_short_name,route_type\nR1,A,1,3\nR9,Z,9,3\n",
        "route_id,service_id,trip_id\nR1,SVC,T1\nRX,SVC,T2\n",
        &[
            (Some("trip_id"), Some("T2")),     // trip'in route'u yok
            (Some("trip_id"), Some("NOPE")),   // trip yok
            (Some("route_id"), Some("R9")),    // route'un agency'si yok
            (Some("agency_id"), Some("Z")),    // agency yok
        ],
    );
    assert_eq!(out[0].0, unattributed(UnattributedReason::UnknownRoute));
    assert_eq!(out[1].0, unattributed(UnattributedReason::UnknownTrip));
    assert_eq!(out[2].0, unattributed(UnattributedReason::UnknownAgency));
    assert_eq!(out[3].0, unattributed(UnattributedReason::UnknownAgency));
}

#[test]
fn padded_ids_use_the_unique_trim_fallback_and_never_guess() {
    let out = attribute(
        AGENCIES_AB,
        "route_id,agency_id,route_short_name,route_type\n R1 ,A,1,3\nR2,B,2,3\n R2,A,3,3\n",
        "route_id,service_id,trip_id\n R1 ,SVC,T1\n",
        &[
            (Some("route_id"), Some("R1")),   // tek ham karşılık: " R1 "
            (Some("route_id"), Some(" R1 ")), // birebir
            (Some("route_id"), Some("R2 ")),  // "R2" ve " R2" → belirsiz
            (Some("trip_id"), Some("T1")),    // trip birebir, route ham " R1 "
        ],
    );
    assert!(resolved("A", Resolution::UniqueTrimFallback)(&out[0]), "{out:?}");
    assert!(resolved("A", Resolution::Exact)(&out[1]), "{out:?}");
    assert_eq!(out[2].0, unattributed(UnattributedReason::AmbiguousPaddedId));
    assert!(resolved("A", Resolution::Exact)(&out[3]), "{out:?}");
}

#[test]
fn scope_kinds_outside_p0_are_classified_statically() {
    let out = attribute(
        AGENCIES_AB,
        "route_id,agency_id,route_short_name,route_type\nR1,A,1,3\n",
        "route_id,service_id,trip_id\nR1,SVC,T1\n",
        &[
            (None, None),                   // feed/dosya düzeyi
            (Some("stop_id"), Some("S1")),  // P6
            (Some("trip_id"), None),        // beyan var, değer yok
        ],
    );
    assert_eq!(out[0].0, AgencyAttribution::NotApplicable);
    assert_eq!(out[1].0, AgencyAttribution::Unsupported);
    assert_eq!(out[2].0, unattributed(UnattributedReason::MissingScopeKey));
}

#[test]
fn feed_level_summaries_of_scoped_rules_are_not_missing_keys() {
    let summary = {
        let mut n = probe(Some("trip_id"), None);
        n.entity_type = EntityType::Feed;
        n
    };
    let out = attribute_notices(&[summary]);
    assert_eq!(out[0].0, unattributed(UnattributedReason::FeedLevelSummary));
}

/// #2201 P2: feed-level özet, altındaki ham notice'ların agency dağılımını kaybetmez.
/// İki agency'nin birer seferinde direction_id geçersiz → tek TRP_005 özeti, A=1 B=1.
#[test]
fn aggregated_summary_keeps_the_agency_distribution() {
    let files = [
        ("agency.txt", AGENCIES_AB),
        ("stops.txt", STOPS),
        ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,A,1,3\nR2,B,2,3\n"),
        ("trips.txt", "route_id,service_id,trip_id,direction_id\nR1,SVC,T1,7\nR2,SVC,T2,9\n"),
        (
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n\
T2,09:00:00,09:00:00,S1,1\nT2,09:10:00,09:10:00,S2,2\n",
        ),
        ("calendar.txt", CALENDAR),
    ];
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in files {
        writer.start_file(name, SimpleFileOptions::default()).unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    let zip = writer.finish().unwrap().into_inner();
    let ValidateResult::Ok(result) = validate_bytes(&zip, &ValidatorConfig::default(), 20_260_930)
    else {
        panic!("feed fatal");
    };
    let summary: Vec<&Notice> = result.notices.iter().filter(|n| n.rule_id == "TRP_005").collect();
    assert_eq!(summary.len(), 1, "TRP_005 tek feed özeti olmalı");
    assert_eq!(summary[0].entity_type, EntityType::Feed);
    let distribution = summary[0].agency_distribution.as_ref().expect("dağılım yok");
    let agencies: Vec<(u32, u64)> = distribution
        .iter()
        .map(|(attribution, n)| match attribution {
            AgencyAttribution::Resolved { agency, resolution: Resolution::Exact } => (agency.0, *n),
            other => panic!("beklenmeyen atıf: {other:?}"),
        })
        .collect();
    assert_eq!(agencies, [(0, 1), (1, 1)], "A (0) ve B (1) birer sefer");
}

/// `fare_id` scope'lu kurallar: `fare_attributes.agency_id` → agency; boşsa tek agency.
#[test]
fn fares_resolve_through_fare_attributes() {
    let fares = |agencies: &str, fare_rows: &str, probes: &[Option<&str>]| {
        let notices: Vec<Notice> = probes.iter().map(|key| probe(Some("fare_id"), *key)).collect();
        attribute_in(
            agencies,
            "route_id,agency_id,route_short_name,route_type\nR1,A,1,3\n",
            "route_id,service_id,trip_id\nR1,SVC,T1\n",
            &[(
                "fare_attributes.txt",
                fare_rows,
            )],
            &notices,
        )
    };
    let multi = fares(
        AGENCIES_AB,
        "fare_id,price,currency_type,payment_method,transfers,agency_id\n\
F1,1.00,USD,0,0,B\nF2,2.00,USD,0,0,\n",
        &[Some("F1"), Some("F2"), Some("NOPE")],
    );
    assert!(resolved("B", Resolution::Exact)(&multi[0]), "{multi:?}");
    assert_eq!(multi[1].0, unattributed(UnattributedReason::FareWithoutAgency));
    assert_eq!(multi[2].0, unattributed(UnattributedReason::UnknownFare));

    let single = fares(
        "agency_id,agency_name,agency_url,agency_timezone\nA,Alpha,http://a.example,UTC\n",
        "fare_id,price,currency_type,payment_method,transfers\nF1,1.00,USD,0,0\n",
        &[Some("F1")],
    );
    assert!(resolved("A", Resolution::Exact)(&single[0]), "{single:?}");
}

/// FAR_010 scope beyan etmez (feed özeti) ama ham notice'ları tarife başınadır; toplulama
/// dağılımı tarifenin agency'sinden kurulur (`attribute_member`, `EntityType::Fare`).
#[test]
fn far010_summary_distributes_over_fare_agencies() {
    let files = [
        ("agency.txt", AGENCIES_AB),
        ("stops.txt", STOPS),
        ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,A,1,3\n"),
        ("trips.txt", "route_id,service_id,trip_id\nR1,SVC,T1\n"),
        (
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n",
        ),
        ("calendar.txt", CALENDAR),
        (
            "fare_attributes.txt",
            "fare_id,price,currency_type,payment_method,transfers,agency_id\n\
F1,1.00,USD,0,0,A\nF2,2.00,USD,0,0,B\nF3,3.00,USD,0,0,B\n",
        ),
        ("fare_rules.txt", "fare_id,route_id\nF1,R1\nF2,R1\nF3,R1\n"),
    ];
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in files {
        writer.start_file(name, SimpleFileOptions::default()).unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    let zip = writer.finish().unwrap().into_inner();
    let ValidateResult::Ok(result) = validate_bytes(&zip, &ValidatorConfig::default(), 20_260_930)
    else {
        panic!("feed fatal");
    };
    let summary: Vec<&Notice> = result.notices.iter().filter(|n| n.rule_id == "FAR_010").collect();
    assert_eq!(summary.len(), 1, "FAR_010 tek feed özeti olmalı");
    assert_eq!(summary[0].observed_value.as_deref(), Some("2"));
    let distribution = summary[0].agency_distribution.as_ref().expect("dağılım yok");
    // F2 ve F3, F1 ile çakışır; ikisi de B'nin tarifesi.
    let agencies: Vec<(u32, u64)> = distribution
        .iter()
        .map(|(attribution, n)| (attribution.agency().expect("atfedilmeli").0, *n))
        .collect();
    assert_eq!(agencies, [(1, 2)], "{distribution:?}");
}
