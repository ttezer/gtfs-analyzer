//! Many stops at one point must not cost n² (2026-11 security review: 10,000 stops at one
//! coordinate took >90 s and 7.6 GB because STP_016 emitted one notice per PAIR before dedup).
//! The report keeps one STP_016 per stop either way; this pins the raw count.
use std::io::Write as _;

use gtfs_pipeline::{validate_bytes_inspected, ValidatorConfig};
use zip::write::SimpleFileOptions;

const STOPS: usize = 2_000;

fn feed() -> Vec<u8> {
    let mut stops = String::from("stop_id,stop_name,stop_lat,stop_lon\n");
    for i in 0..STOPS {
        stops.push_str(&format!("S{i},n,41.5,29.5\n"));
    }
    let files = [
        ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\nA,a,https://a.b,UTC\n".to_string()),
        ("stops.txt", stops),
        ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR,A,1,3\n".to_string()),
        ("trips.txt", "route_id,service_id,trip_id\nR,SV,T\n".to_string()),
        ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT,08:00:00,08:00:00,S0,1\n".to_string()),
        ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSV,1,1,1,1,1,1,1,20260101,20261231\n".to_string()),
    ];
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in files {
        writer.start_file(name, SimpleFileOptions::default()).unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn colocated_stops_emit_one_stp_016_per_stop_before_dedup() {
    let mut raw = None;
    validate_bytes_inspected(&feed(), &ValidatorConfig::default(), 20_260_930, &mut |notices, _, _, _| {
        raw = Some(notices.iter().filter(|n| n.rule_id == "STP_016").count());
    });
    let raw = raw.expect("observer did not run (fatal feed?)");
    // Every stop but the last has a later partner at the same point: one notice each.
    // The old pairwise emission produced STOPS * (STOPS - 1) / 2 = 1,999,000.
    assert_eq!(raw, STOPS - 1);
}
