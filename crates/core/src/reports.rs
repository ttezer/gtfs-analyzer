use serde::{Deserialize, Serialize};
use crate::{R9Label, ReportItem};

/// R1: Yayınlanabilirlik kararı.
/// KRİTİK+SPEC/INTEROP → red; YÜKSEK+INTEROP → koşullu; diğer → yeşil.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R1Report {
    pub publishable: bool,
    /// true ise conditional_blocker bulguları mevcut — uyarıyla yayın.
    pub conditional: bool,
    pub blocker_notice_ids: Vec<String>,
    pub conditional_blocker_notice_ids: Vec<String>,
}

/// R2: Tüm notice'ların teknik listesi — file/line/field/observed_value ile birlikte.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R2Report {
    pub items: Vec<ReportItem>,
}

/// R3: Operasyonel kalite — ANALYTICS sınıfı notice'ları.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R3Report {
    pub items: Vec<ReportItem>,
}

/// R4: Coğrafi doğruluk raporu.
/// SHP_013/014/016 notice'larından üretilir; display_label = "GEO_008/010/011".
/// GEO_008/010/011 için ayrı Notice üretilmez.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R4Report {
    pub items: Vec<ReportItem>,
}

/// R5: Genel Skor kartı — 0–100 ağırlıklı genel skor (Spec×40%+Interop×30%+Quality×20%+Analytics×10%) + sınıf kırılımı (MET_004).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R5Report {
    pub score: f64,
    /// Yayınlanabilirlik skoru: yalnızca blocker-eligible notice'lardan (Kritik SPEC/INTEROP
    /// + Yüksek INTEROP). 100 = temiz, 0 = ağır blocker yükü.
    pub pub_score: f64,
    pub spec_score: f64,
    pub interop_score: f64,
    pub quality_score: f64,
    pub analytics_score: f64,
}

/// R7: Erişilebilirlik raporu — wheelchair_*, stop_access, PTH_012/013.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R7Report {
    pub items: Vec<ReportItem>,
}

/// R8: Journey planner hazırlık raporu — yalnızca INTEROP sınıfı.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R8Report {
    pub items: Vec<ReportItem>,
}

/// R9 kuyruğundaki tek bir düzeltme kalemi.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R9Item {
    pub rule_id: String,
    pub labels: Vec<R9Label>,
    pub priority_score: f64,
    /// Bu kuralın notice'ları düzeltilseydi R5 overall skoru kaç puan artardı.
    pub score_delta: f64,
    /// Bu kuralın notice'ları düzeltilseydi R5 pub_score kaç puan artardı.
    /// Yalnızca blocker-eligible kurallar için sıfırdan büyük olur.
    pub pub_score_delta: f64,
    pub affected_instance_count: u32,
    /// Bu feed'de blocks[] içinden gerçekten ateşlenmiş kural sayısı.
    pub realized_dependent_count: u32,
    pub base_effort: u8,
    pub fix_effort: f64,
    /// İlgili canonical Notice id'leri.
    pub notice_ids: Vec<String>,
}

/// R9: Remediation queue — bucket-first + priority_score azalan sıra.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R9Report {
    pub items: Vec<R9Item>,
}

/// Tüm rapor görünümlerinin kapsayıcısı.
/// R6 export aksiyonudur — veri yapısı UI tarafında yönetilir, burada yer almaz.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSet {
    pub r1: R1Report,
    pub r2: R2Report,
    pub r3: R3Report,
    pub r4: R4Report,
    pub r5: R5Report,
    pub r7: R7Report,
    pub r8: R8Report,
    pub r9: R9Report,
}
