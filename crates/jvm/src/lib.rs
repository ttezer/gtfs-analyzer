use gtfs_config::{merge_delta, ValidatorConfig};
use gtfs_core::i18n::{Lang, Translator};
use gtfs_core::{NameIndex, ValidateResult};
use gtfs_pipeline::validate_bytes;

/// Validate one GTFS ZIP buffer using the shared Rust validation pipeline.
pub fn validate_to_json(
    feed: &[u8],
    today: u32,
    config_json: &str,
    include_name_index: bool,
    lang: &str,
) -> Result<String, String> {
    // Lang is checked BEFORE validation: an invalid code should fail immediately,
    // not after the feed has been processed.
    let lang: Lang = lang.parse()?;
    let translator = Translator::new(lang)?;
    let config = merge_delta(&ValidatorConfig::default(), config_json)?;
    let mut result = validate_bytes(feed, &config, today);

    // Same translation as the CLI (crates/cli/src/main.rs): title/message/remediation and fatal.
    if let Some(translator) = &translator {
        match &mut result {
            ValidateResult::Ok(validation) => {
                for notice in &mut validation.notices {
                    translator.translate(notice);
                }
            }
            ValidateResult::Fatal(err) => translator.translate_fatal(err),
        }
    }

    if !include_name_index {
        if let ValidateResult::Ok(validation) = &mut result {
            validation.name_index = NameIndex::default();
        }
    }

    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use zip::write::SimpleFileOptions;

    // Same fixed date used across tests — deterministic analytics output.
    const TODAY: u32 = 20_260_515;

    // ── Helpers ─────────────────────────────────────────────────────────

    fn make_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let cur = std::io::Cursor::new(Vec::new());
        let mut zw = zip::ZipWriter::new(cur);
        let opts = SimpleFileOptions::default();
        for (name, data) in files {
            zw.start_file(*name, opts).unwrap();
            zw.write_all(data).unwrap();
        }
        zw.finish().unwrap().into_inner()
    }

    // Date range (20250101-20271231) covers the test day (20260515, Friday).
    // Monday-Friday=1 -> SVC1 active -> no service-gap notice.
    static AGENCY: &[u8] =
        b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://test.example,UTC\n";
    static STOPS: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n";
    static ROUTES: &[u8] = b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n";
    static TRIPS: &[u8] = b"route_id,service_id,trip_id\nR1,SVC1,T1\n";
    static STOP_TIMES: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n";
    static CALENDAR: &[u8] =
        b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
          SVC1,1,1,1,1,1,0,0,20250101,20271231\n";

    fn base_files() -> Vec<(&'static str, &'static [u8])> {
        vec![
            ("agency.txt", AGENCY),
            ("stops.txt", STOPS),
            ("routes.txt", ROUTES),
            ("trips.txt", TRIPS),
            ("stop_times.txt", STOP_TIMES),
            ("calendar.txt", CALENDAR),
        ]
    }

    #[test]
    fn validate_to_json_returns_ok_envelope_for_a_clean_feed() {
        let zip = make_zip(&base_files());
        let json = validate_to_json(&zip, TODAY, "{}", false, "en").unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(
            value.get("Ok").is_some(),
            "expected an {{\"Ok\": ...}} envelope, got: {json}"
        );
        let metrics = &value["Ok"]["metrics"];
        assert_eq!(metrics["stop_count"], 2);
        assert_eq!(metrics["route_count"], 1);
        assert_eq!(metrics["trip_count"], 1);
    }

    #[test]
    fn validate_to_json_rejects_unknown_config_key() {
        let zip = make_zip(&base_files());
        let err =
            validate_to_json(&zip, TODAY, r#"{"not_a_real_key": 1}"#, false, "en").unwrap_err();
        assert!(
            err.contains("not_a_real_key"),
            "error should name the bad key: {err}"
        );
    }

    #[test]
    fn validate_to_json_rejects_unknown_lang() {
        let zip = make_zip(&base_files());
        let err = validate_to_json(&zip, TODAY, "{}", false, "xx").unwrap_err();
        assert!(!err.is_empty());
    }
}
