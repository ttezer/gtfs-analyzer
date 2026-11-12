//! ARC_017 lists at most 100 unknown columns per file, then one summary notice (2026-11
//! security review: a header with 200,000 junk columns produced 119 MB of JSON).
use std::io::Write as _;

use gtfs_pipeline::{validate_bytes, ValidateResult, ValidatorConfig};
use zip::write::SimpleFileOptions;

fn feed(extra_columns: usize) -> Vec<u8> {
    let mut header = String::from("stop_id,stop_name,stop_lat,stop_lon");
    let mut row = String::from("S1,n,41.5,29.5");
    for i in 0..extra_columns {
        header.push_str(&format!(",x_{i}"));
        row.push_str(",v");
    }
    let files = [
        ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\nA,a,https://a.b,UTC\n".to_string()),
        ("stops.txt", format!("{header}\n{row}\n")),
        ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR,A,1,3\n".to_string()),
        ("trips.txt", "route_id,service_id,trip_id\nR,SV,T\n".to_string()),
        ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT,08:00:00,08:00:00,S1,1\n".to_string()),
        ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSV,1,1,1,1,1,1,1,20260101,20261231\n".to_string()),
    ];
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in files {
        writer.start_file(name, SimpleFileOptions::default()).unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// (listed, summary notices, omitted count reported by the summary)
fn arc017(extra_columns: usize) -> (usize, usize, Option<String>) {
    let ValidateResult::Ok(result) =
        validate_bytes(&feed(extra_columns), &ValidatorConfig::default(), 20_260_930)
    else {
        panic!("feed fatal");
    };
    let notices: Vec<_> = result
        .notices
        .iter()
        .filter(|n| n.rule_id == "ARC_017")
        .collect();
    let summary: Vec<_> = notices
        .iter()
        .filter(|n| n.details.as_ref().and_then(|d| d.get("message_variant")).map(String::as_str) == Some("truncated"))
        .collect();
    let omitted = summary.first().and_then(|n| n.observed_value.clone());
    (notices.len() - summary.len(), summary.len(), omitted)
}

#[test]
fn at_the_cap_every_column_is_listed() {
    assert_eq!(arc017(100), (100, 0, None));
}

#[test]
fn past_the_cap_one_summary_carries_the_rest() {
    assert_eq!(arc017(150), (100, 1, Some("50".to_string())));
}
