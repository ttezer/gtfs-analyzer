//! Rapor sayfasının "ajans ve yayıncı" kartı için ham künye: `agency.txt` iletişim
//! alanları ve `feed_info.txt` yayıncı alanları. Değerler dosyada yazıldığı gibi
//! (yalnız baş/son boşluk kırpılmış) taşınır; doğrulama bulguları ayrı kurallardadır.

use serde::{Deserialize, Serialize};

/// Taşınan en fazla ajans sayısı. Çok ajanslı ulusal feed'lerde (binlerce kayıt) sonuç
/// boyutunu sınırlar; gerçek sayı [`FeedSummary::agency_count`]'tadır.
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
    /// `agency.txt`'teki kayıt sayısı (taşınan listeden büyük olabilir).
    pub agency_count: u32,
    /// İlk [`MAX_AGENCIES`] ajans, dosya sırasıyla.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agencies: Vec<AgencyContact>,
    /// `feed_info.txt`'in ilk kaydı; dosya yoksa `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_info: Option<FeedInfoSummary>,
}

/// Boş ya da yalnız boşluktan oluşan değeri `None` yapar, kalanı kırpar.
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
