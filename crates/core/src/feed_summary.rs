//! Raw metadata for the report page's "agency and publisher" card: contact fields from
//! `agency.txt` and publisher fields from `feed_info.txt`. Values are carried as written
//! in the files (with only leading/trailing whitespace trimmed); validation findings live
//! in separate rules.

use serde::{Deserialize, Serialize};

/// Maximum number of agencies carried in the result. Limits result size for national feeds
/// with many agencies (thousands of records); the actual count is [`FeedSummary::agency_count`].
pub const MAX_AGENCIES: usize = 100;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgencyContact {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agency_id: Option<String>,
    pub agency_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agency_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agency_timezone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agency_lang: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agency_phone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agency_email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agency_fare_url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedInfoSummary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_publisher_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_publisher_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_lang: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_contact_email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_contact_url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedSummary {
    /// Number of records in `agency.txt` (may exceed the carried list).
    pub agency_count: u32,
    /// The first [`MAX_AGENCIES`] agencies, in file order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agencies: Vec<AgencyContact>,
    /// The first record in `feed_info.txt`; `None` if the file is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_info: Option<FeedInfoSummary>,
}

/// Returns `None` for an empty or whitespace-only value; trims all other values.
pub fn clean(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_drops_blank_and_trims() {
        assert_eq!(clean("  "), None);
        assert_eq!(clean(""), None);
        assert_eq!(clean(" a b "), Some("a b".to_string()));
    }

    #[test]
    fn empty_fields_are_not_serialized() {
        let s = FeedSummary {
            agency_count: 1,
            agencies: vec![AgencyContact { agency_name: "A".into(), ..Default::default() }],
            feed_info: None,
        };
        assert_eq!(
            serde_json::to_string(&s).unwrap(),
            r#"{"agency_count":1,"agencies":[{"agency_name":"A"}]}"#
        );
    }
}
