use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use chrono::{Datelike, Local};
use clap::{Args, Parser, Subcommand, ValueEnum};
use gtfs_config::{merge_delta, ValidatorConfig};
use gtfs_core::{
    AuthoritySource, NameIndex, Notice, RuleClass, Severity, ValidateResult, ValidationResult,
    ValidationStatus,
};
use gtfs_pipeline::validate_bytes;
use gtfs_rules::RULES;
use serde::Serialize;

mod i18n;
use i18n::{LangArg, Translator};

#[derive(Debug, Parser)]
#[command(name = "gtfs-analyzer", version)]
#[command(about = "GTFS feed validator CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate a GTFS ZIP feed.
    Validate(ValidateArgs),
    /// List the rule registry (id, severity, class, authority source, title).
    Rules(RulesArgs),
}

#[derive(Debug, Args)]
struct ValidateArgs {
    /// Path to the GTFS ZIP feed, or `-` to read it from stdin.
    feed: PathBuf,

    /// Emit the full result as JSON.
    #[arg(long)]
    json: bool,

    /// Emit a short text summary. This is also the default when --json is absent.
    #[arg(long, conflicts_with = "json")]
    summary: bool,

    /// Keep only notices with this rule id.
    #[arg(long)]
    rule: Option<String>,

    /// Keep only notices with exactly this severity.
    #[arg(long, conflicts_with = "min_severity")]
    severity: Option<SeverityArg>,

    /// Keep notices at this severity or worse (critical is the worst).
    #[arg(long)]
    min_severity: Option<SeverityArg>,

    /// Keep only notices in these rule classes (repeatable, comma separated).
    #[arg(long, value_delimiter = ',')]
    class: Vec<RuleClassArg>,

    /// Exit 1 only when a notice at this severity or worse exists.
    #[arg(long)]
    fail_on: Option<SeverityArg>,

    /// Exit 1 only when a notice in one of these rule classes exists.
    #[arg(long, value_delimiter = ',')]
    fail_on_class: Vec<RuleClassArg>,

    /// Pretty-print the JSON output.
    #[arg(long, requires = "json")]
    pretty: bool,

    /// Include name_index (stop/route/shape lookup tables) in the JSON output.
    /// Omitted by default: on large feeds it dominates the payload.
    #[arg(long, requires = "json")]
    include_name_index: bool,

    /// Write the output to this file instead of stdout.
    #[arg(long, short = 'o')]
    output: Option<PathBuf>,

    /// Language for notice titles, messages and remediations.
    #[arg(long, value_enum, default_value = "tr")]
    lang: LangArg,

    /// JSON config delta to merge over ValidatorConfig::default().
    #[arg(long)]
    config: Option<PathBuf>,

    /// Validation date as YYYYMMDD. Defaults to the local current date.
    #[arg(long, value_parser = parse_today)]
    today: Option<u32>,
}

#[derive(Debug, Args)]
struct RulesArgs {
    /// Emit the registry as JSON.
    #[arg(long)]
    json: bool,

    /// Keep only rules in these classes (repeatable, comma separated).
    #[arg(long, value_delimiter = ',')]
    class: Vec<RuleClassArg>,

    /// Keep only rules with exactly this severity.
    #[arg(long, conflicts_with = "min_severity")]
    severity: Option<SeverityArg>,

    /// Keep rules at this severity or worse (critical is the worst).
    #[arg(long)]
    min_severity: Option<SeverityArg>,

    /// Keep only the rule with this id.
    #[arg(long)]
    rule: Option<String>,

    /// Pretty-print the JSON output.
    #[arg(long, requires = "json")]
    pretty: bool,

    /// Write the output to this file instead of stdout.
    #[arg(long, short = 'o')]
    output: Option<PathBuf>,

    /// Language for rule titles.
    #[arg(long, value_enum, default_value = "tr")]
    lang: LangArg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SeverityArg {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl From<SeverityArg> for Severity {
    fn from(value: SeverityArg) -> Self {
        match value {
            SeverityArg::Critical => Severity::Kritik,
            SeverityArg::High => Severity::Yuksek,
            SeverityArg::Medium => Severity::Orta,
            SeverityArg::Low => Severity::Dusuk,
            SeverityArg::Info => Severity::Bilgi,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum RuleClassArg {
    Spec,
    Interop,
    Quality,
    Analytics,
}

impl From<RuleClassArg> for RuleClass {
    fn from(value: RuleClassArg) -> Self {
        match value {
            RuleClassArg::Spec => RuleClass::Spec,
            RuleClassArg::Interop => RuleClass::Interop,
            RuleClassArg::Quality => RuleClass::Quality,
            RuleClassArg::Analytics => RuleClass::Analytics,
        }
    }
}

/// Machine-readable severity token; matches the `Severity` serde representation.
fn severity_str(severity: Severity) -> &'static str {
    match severity {
        Severity::Kritik => "CRITICAL",
        Severity::Yuksek => "HIGH",
        Severity::Orta => "MEDIUM",
        Severity::Dusuk => "LOW",
        Severity::Bilgi => "INFO",
    }
}

/// Machine-readable rule class token; matches the `RuleClass` serde representation.
fn class_str(class: RuleClass) -> &'static str {
    match class {
        RuleClass::Spec => "SPEC",
        RuleClass::Interop => "INTEROP",
        RuleClass::Quality => "QUALITY",
        RuleClass::Analytics => "ANALYTICS",
    }
}

fn authority_str(source: AuthoritySource) -> &'static str {
    match source {
        AuthoritySource::GtfsSpec => "GTFS_SPEC",
        AuthoritySource::GtfsBestPractice => "GTFS_BEST_PRACTICE",
        AuthoritySource::MobilitydataParity => "MOBILITYDATA_PARITY",
        AuthoritySource::GoogleTransitInterop => "GOOGLE_TRANSIT_INTEROP",
        AuthoritySource::RegionalProfile => "REGIONAL_PROFILE",
        AuthoritySource::ProjectQuality => "PROJECT_QUALITY",
        AuthoritySource::ProjectAnalytics => "PROJECT_ANALYTICS",
        AuthoritySource::Unknown => "UNKNOWN",
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Validate(args) => run_validate(args),
        Command::Rules(args) => run_rules(args),
    }
}

// ── validate ──────────────────────────────────────────────────────────────────

/// Notice-level filters. These are *display* filters: they never change the
/// publish decision (R1) or the scores (R5), which always describe the whole feed.
#[derive(Debug, Default)]
struct Filters {
    rule: Option<String>,
    severity: Option<Severity>,
    min_severity: Option<Severity>,
    classes: Vec<RuleClass>,
}

impl Filters {
    fn is_empty(&self) -> bool {
        self.rule.is_none()
            && self.severity.is_none()
            && self.min_severity.is_none()
            && self.classes.is_empty()
    }

    fn matches(&self, notice: &Notice) -> bool {
        if let Some(rule) = &self.rule {
            if notice.rule_id != *rule {
                return false;
            }
        }
        if let Some(wanted) = self.severity {
            if notice.severity != wanted {
                return false;
            }
        }
        // `Severity` orders worst-first (Kritik is the smallest variant), so
        // "at least as severe as X" is `notice.severity <= X`.
        if let Some(floor) = self.min_severity {
            if notice.severity > floor {
                return false;
            }
        }
        if !self.classes.is_empty() && !self.classes.contains(&notice.rule_class) {
            return false;
        }
        true
    }

    fn describe(&self) -> Vec<String> {
        let mut parts = Vec::new();
        if let Some(rule) = &self.rule {
            parts.push(format!("rule={rule}"));
        }
        if let Some(severity) = self.severity {
            parts.push(format!("severity={}", severity_str(severity)));
        }
        if let Some(floor) = self.min_severity {
            parts.push(format!("min_severity={}", severity_str(floor)));
        }
        if !self.classes.is_empty() {
            let joined: Vec<&str> = self.classes.iter().map(|c| class_str(*c)).collect();
            parts.push(format!("class={}", joined.join(",")));
        }
        parts
    }
}

fn run_validate(args: ValidateArgs) -> ExitCode {
    let zip_bytes = match read_feed(&args.feed) {
        Ok(bytes) => bytes,
        Err(err) => return cli_error(err),
    };

    let config = match load_config(args.config.as_ref()) {
        Ok(config) => config,
        Err(err) => return cli_error(err),
    };

    // Parsed before the pipeline runs: a broken dictionary should fail fast
    // rather than after a multi-minute validation.
    let translator = match Translator::new(args.lang) {
        Ok(translator) => translator,
        Err(err) => return cli_error(err),
    };

    suppress_pipeline_timing_by_default();

    let today = args.today.unwrap_or_else(today_yyyymmdd);
    let mut result = validate_bytes(&zip_bytes, &config, today);

    let filters = Filters {
        rule: args.rule.clone(),
        severity: args.severity.map(Into::into),
        min_severity: args.min_severity.map(Into::into),
        classes: args.class.iter().copied().map(Into::into).collect(),
    };
    apply_filters(&mut result, &filters);

    // After filtering: no point translating notices that were dropped.
    if let (Some(translator), ValidateResult::Ok(vr)) = (&translator, &mut result) {
        for notice in &mut vr.notices {
            translator.translate(notice);
        }
    }

    if !args.include_name_index {
        if let ValidateResult::Ok(vr) = &mut result {
            vr.name_index = NameIndex::default();
        }
    }

    let rendered = if args.json {
        match render_json(&result, &filters, args.pretty) {
            Ok(json) => json,
            Err(err) => return cli_error(format!("failed to serialize result as JSON: {err}")),
        }
    } else {
        render_summary(&result, &filters)
    };

    if let Err(err) = write_output(&rendered, args.output.as_deref()) {
        return cli_error(err);
    }

    exit_code(
        &result,
        args.fail_on.map(Into::into),
        &args
            .fail_on_class
            .iter()
            .copied()
            .map(Into::into)
            .collect::<Vec<RuleClass>>(),
    )
}

/// Reads the feed ZIP, or the whole of stdin when the path is `-`.
///
/// The pipeline needs the archive as one buffer (the ZIP central directory sits
/// at the end), so a piped feed is read to memory rather than streamed.
fn read_feed(path: &Path) -> Result<Vec<u8>, String> {
    if path.as_os_str() == "-" {
        let mut buf = Vec::new();
        return std::io::Read::read_to_end(&mut std::io::stdin().lock(), &mut buf)
            .map(|_| buf)
            .map_err(|err| format!("failed to read the feed from stdin: {err}"));
    }

    std::fs::read(path).map_err(|err| format!("failed to read '{}': {err}", path.display()))
}

fn load_config(path: Option<&PathBuf>) -> Result<ValidatorConfig, String> {
    let base = ValidatorConfig::default();
    let Some(path) = path else {
        return Ok(base);
    };

    let delta = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read config '{}': {err}", path.display()))?;
    merge_delta(&base, &delta).map_err(|err| format!("invalid config '{}': {err}", path.display()))
}

fn apply_filters(result: &mut ValidateResult, filters: &Filters) {
    let ValidateResult::Ok(vr) = result else {
        return;
    };

    if filters.is_empty() {
        return;
    }

    vr.notices.retain(|notice| filters.matches(notice));
    prune_reports(vr);
}

/// Drops report rows whose notice was filtered out.
///
/// R1 (`publishable` / `blocker_notice_ids`) and R5 (scores) are deliberately
/// left alone: they are verdicts about the whole feed. Recomputing them over a
/// filtered subset used to report `publishable: true` for a feed with critical
/// blockers whenever a `--rule`/`--severity` filter hid them.
fn prune_reports(vr: &mut ValidationResult) {
    let kept_notice_ids: HashSet<&str> =
        vr.notices.iter().map(|notice| notice.id.as_str()).collect();

    vr.reports
        .r2
        .items
        .retain(|item| kept_notice_ids.contains(item.notice_id.as_str()));
    vr.reports
        .r3
        .items
        .retain(|item| kept_notice_ids.contains(item.notice_id.as_str()));
    vr.reports
        .r4
        .items
        .retain(|item| kept_notice_ids.contains(item.notice_id.as_str()));
    vr.reports
        .r7
        .items
        .retain(|item| kept_notice_ids.contains(item.notice_id.as_str()));
    vr.reports
        .r8
        .items
        .retain(|item| kept_notice_ids.contains(item.notice_id.as_str()));

    for item in &mut vr.reports.r9.items {
        item.notice_ids
            .retain(|id| kept_notice_ids.contains(id.as_str()));
        item.affected_instance_count = item.notice_ids.len() as u32;
    }
    vr.reports
        .r9
        .items
        .retain(|item| !item.notice_ids.is_empty());
}

// ── output ────────────────────────────────────────────────────────────────────

/// Flat JSON envelope. Deliberately *not* `ValidateResult`'s serde shape: that
/// one is an externally tagged enum (`{"Ok":{…}}` / `{"Fatal":{…}}`) consumed by
/// the WASM/UI boundary, which must keep its representation.
#[derive(Serialize)]
struct JsonOk<'a> {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    filtered: Option<FilterMeta>,
    #[serde(flatten)]
    result: &'a ValidationResult,
}

#[derive(Serialize)]
struct JsonFatal<'a> {
    status: &'static str,
    code: &'a gtfs_core::FatalCode,
    message: &'a str,
}

#[derive(Serialize)]
struct FilterMeta {
    applied: Vec<String>,
    note: &'static str,
}

const FILTER_NOTE: &str =
    "notices and reports r2-r9 are filtered; r1 publishability and r5 scores describe the whole feed";

fn filter_meta(filters: &Filters) -> Option<FilterMeta> {
    if filters.is_empty() {
        return None;
    }
    Some(FilterMeta {
        applied: filters.describe(),
        note: FILTER_NOTE,
    })
}

fn render_json(
    result: &ValidateResult,
    filters: &Filters,
    pretty: bool,
) -> Result<String, serde_json::Error> {
    match result {
        ValidateResult::Fatal(err) => {
            let payload = JsonFatal {
                status: "fatal",
                code: &err.code,
                message: &err.message,
            };
            serialize(&payload, pretty)
        }
        ValidateResult::Ok(vr) => {
            let payload = JsonOk {
                status: if vr.status == ValidationStatus::Partial { "partial" } else { "ok" },
                filtered: filter_meta(filters),
                result: vr,
            };
            serialize(&payload, pretty)
        }
    }
}

fn serialize<T: Serialize>(value: &T, pretty: bool) -> Result<String, serde_json::Error> {
    if pretty {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    }
}

fn render_summary(result: &ValidateResult, filters: &Filters) -> String {
    let mut out = String::new();
    match result {
        ValidateResult::Fatal(err) => {
            out.push_str("status: FATAL\n");
            out.push_str(&format!("fatal_code: {:?}\n", err.code));
            out.push_str(&format!("fatal_message: {}\n", err.message));
        }
        ValidateResult::Ok(vr) => {
            out.push_str(if vr.status == ValidationStatus::Partial {
                "status: PARTIAL\n"
            } else {
                "status: OK\n"
            });
            if let Some(partial) = &vr.partial {
                out.push_str(&format!("partial_root_errors: {}\n", partial.root_structural_errors.len()));
                out.push_str(&format!("partial_unavailable_files: {}\n", partial.unavailable_files.len()));
                out.push_str(&format!("partial_skipped_stages: {}\n", partial.skipped_stages.len()));
            }
            out.push_str(&format!("notices: {}\n", vr.notices.len()));
            if !filters.is_empty() {
                out.push_str(&format!("filter: {}\n", filters.describe().join(" ")));
                out.push_str(&format!("filter_note: {FILTER_NOTE}\n"));
            }
            out.push_str(&format!("publishable: {}\n", vr.reports.r1.publishable));
            out.push_str(&format!("score: {:.1}\n", vr.reports.r5.score));
            out.push_str(&format!("pub_score: {:.1}\n", vr.reports.r5.pub_score));
            out.push_str(&format!("spec_score: {:.1}\n", vr.reports.r5.spec_score));
            out.push_str(&format!(
                "interop_score: {:.1}\n",
                vr.reports.r5.interop_score
            ));
            out.push_str(&format!(
                "quality_score: {:.1}\n",
                vr.reports.r5.quality_score
            ));
            out.push_str(&format!(
                "analytics_score: {:.1}\n",
                vr.reports.r5.analytics_score
            ));
        }
    }
    out
}

fn write_output(text: &str, path: Option<&Path>) -> Result<(), String> {
    match path {
        Some(path) => std::fs::write(path, text)
            .map_err(|err| format!("failed to write '{}': {err}", path.display())),
        None => {
            print!("{text}");
            Ok(())
        }
    }
}

/// `0` clean · `1` findings or partial · `2` fatal or CLI error.
///
/// Without `--fail-on*` any notice yields 1 (historical behaviour). With them,
/// only a matching notice does — the rest are reported but do not fail the run.
fn exit_code(
    result: &ValidateResult,
    fail_on: Option<Severity>,
    fail_on_class: &[RuleClass],
) -> ExitCode {
    let vr = match result {
        ValidateResult::Fatal(_) => return ExitCode::from(2),
        ValidateResult::Ok(vr) => vr,
    };

    if vr.status == ValidationStatus::Partial {
        return ExitCode::from(1);
    }

    if fail_on.is_none() && fail_on_class.is_empty() {
        return if vr.notices.is_empty() {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        };
    }

    let hit = vr.notices.iter().any(|notice| {
        fail_on.is_some_and(|floor| notice.severity <= floor)
            || fail_on_class.contains(&notice.rule_class)
    });
    if hit {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

// ── rules ─────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct RuleRow<'a> {
    id: &'static str,
    severity: &'static str,
    class: &'static str,
    authority_source: &'static str,
    base_effort: u8,
    blocks: &'static [&'static str],
    title: &'a str,
}

fn run_rules(args: RulesArgs) -> ExitCode {
    let severity: Option<Severity> = args.severity.map(Into::into);
    let min_severity: Option<Severity> = args.min_severity.map(Into::into);
    let classes: Vec<RuleClass> = args.class.iter().copied().map(Into::into).collect();

    let translator = match Translator::new(args.lang) {
        Ok(translator) => translator,
        Err(err) => return cli_error(err),
    };

    let rows: Vec<RuleRow> = RULES
        .iter()
        .filter(|meta| {
            args.rule.as_deref().is_none_or(|id| meta.id == id)
                && severity.is_none_or(|wanted| meta.severity == wanted)
                && min_severity.is_none_or(|floor| meta.severity <= floor)
                && (classes.is_empty() || classes.contains(&meta.rule_class))
        })
        .map(|meta| RuleRow {
            id: meta.id,
            severity: severity_str(meta.severity),
            class: class_str(meta.rule_class),
            authority_source: authority_str(meta.authority_source()),
            base_effort: meta.base_effort,
            blocks: meta.blocks,
            title: match &translator {
                Some(translator) => translator.rule_title(meta.id, meta.title),
                None => meta.title,
            },
        })
        .collect();

    if let Some(id) = &args.rule {
        if rows.is_empty() {
            return cli_error(format!("unknown rule id '{id}'"));
        }
    }

    let rendered = if args.json {
        match serialize(&rows, args.pretty) {
            Ok(json) => format!("{json}\n"),
            Err(err) => return cli_error(format!("failed to serialize rules as JSON: {err}")),
        }
    } else {
        render_rules_table(&rows)
    };

    if let Err(err) = write_output(&rendered, args.output.as_deref()) {
        return cli_error(err);
    }

    ExitCode::SUCCESS
}

fn render_rules_table(rows: &[RuleRow]) -> String {
    let mut out = String::new();
    for row in rows {
        out.push_str(&format!(
            "{:<10} {:<8} {:<9} {:<22} {}\n",
            row.id, row.severity, row.class, row.authority_source, row.title
        ));
    }
    out.push_str(&format!("\n{} rules\n", rows.len()));
    out
}

// ── shared ────────────────────────────────────────────────────────────────────

fn cli_error(message: String) -> ExitCode {
    eprintln!("error: {message}");
    ExitCode::from(2)
}

fn suppress_pipeline_timing_by_default() {
    if std::env::var_os("GTFS_CLI_TIMING").is_none() && std::env::var_os("GTFS_QUIET").is_none() {
        std::env::set_var("GTFS_QUIET", "1");
    }
}

fn parse_today(raw: &str) -> Result<u32, String> {
    if raw.len() != 8 || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err("expected YYYYMMDD, for example 20260710".to_string());
    }

    let year: i32 = raw[0..4].parse().map_err(|_| "invalid year".to_string())?;
    let month: u32 = raw[4..6].parse().map_err(|_| "invalid month".to_string())?;
    let day: u32 = raw[6..8].parse().map_err(|_| "invalid day".to_string())?;
    chrono::NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| "invalid calendar date".to_string())?;

    raw.parse()
        .map_err(|_| "invalid YYYYMMDD value".to_string())
}

fn today_yyyymmdd() -> u32 {
    let now = Local::now();
    (now.year() as u32) * 10_000 + now.month() * 100 + now.day()
}
