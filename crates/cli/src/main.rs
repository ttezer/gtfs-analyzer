use std::collections::HashSet;
use std::path::PathBuf;
use std::process::ExitCode;

use chrono::{Datelike, Local};
use clap::{Args, Parser, Subcommand, ValueEnum};
use gtfs_config::{merge_delta, ValidatorConfig};
use gtfs_core::{Severity, ValidateResult, ValidationResult};
use gtfs_pipeline::validate_bytes;

#[derive(Debug, Parser)]
#[command(name = "gtfs-analyzer")]
#[command(about = "GTFS feed validator CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate a GTFS ZIP feed.
    Validate(ValidateArgs),
}

#[derive(Debug, Args)]
struct ValidateArgs {
    /// Path to the GTFS ZIP feed.
    feed: PathBuf,

    /// Emit the full ValidateResult as JSON.
    #[arg(long)]
    json: bool,

    /// Emit a short text summary. This is also the default when --json is absent.
    #[arg(long)]
    summary: bool,

    /// Keep only notices with this rule id.
    #[arg(long)]
    rule: Option<String>,

    /// Keep only notices with this severity.
    #[arg(long)]
    severity: Option<SeverityArg>,

    /// JSON config delta to merge over ValidatorConfig::default().
    #[arg(long)]
    config: Option<PathBuf>,

    /// Validation date as YYYYMMDD. Defaults to the local current date.
    #[arg(long, value_parser = parse_today)]
    today: Option<u32>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Validate(args) => run_validate(args),
    }
}

fn run_validate(args: ValidateArgs) -> ExitCode {
    let zip_bytes = match std::fs::read(&args.feed) {
        Ok(bytes) => bytes,
        Err(err) => return cli_error(format!("failed to read '{}': {err}", args.feed.display())),
    };

    let config = match load_config(args.config.as_ref()) {
        Ok(config) => config,
        Err(err) => return cli_error(err),
    };

    suppress_pipeline_timing_by_default();

    let today = args.today.unwrap_or_else(today_yyyymmdd);
    let mut result = validate_bytes(&zip_bytes, &config, today);
    apply_filters(
        &mut result,
        args.rule.as_deref(),
        args.severity.map(Into::into),
    );

    if args.json {
        match serde_json::to_string(&result) {
            Ok(json) => println!("{json}"),
            Err(err) => return cli_error(format!("failed to serialize result as JSON: {err}")),
        }
    } else {
        print_summary(&result);
    }

    exit_code(&result)
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

fn apply_filters(result: &mut ValidateResult, rule: Option<&str>, severity: Option<Severity>) {
    let ValidateResult::Ok(vr) = result else {
        return;
    };

    if rule.is_none() && severity.is_none() {
        return;
    }

    vr.notices.retain(|notice| {
        rule.map_or(true, |wanted| notice.rule_id == wanted)
            && severity.map_or(true, |wanted| notice.severity == wanted)
    });

    prune_reports(vr);
}

fn prune_reports(vr: &mut ValidationResult) {
    let kept_notice_ids: HashSet<&str> =
        vr.notices.iter().map(|notice| notice.id.as_str()).collect();

    vr.reports
        .r1
        .blocker_notice_ids
        .retain(|id| kept_notice_ids.contains(id.as_str()));
    vr.reports.r1.publishable = vr.reports.r1.blocker_notice_ids.is_empty();

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

fn print_summary(result: &ValidateResult) {
    match result {
        ValidateResult::Fatal(err) => {
            println!("status: FATAL");
            println!("fatal_code: {:?}", err.code);
            println!("fatal_message: {}", err.message);
        }
        ValidateResult::Ok(vr) => {
            println!("status: OK");
            println!("notices: {}", vr.notices.len());
            println!("publishable: {}", vr.reports.r1.publishable);
            println!("score: {:.1}", vr.reports.r5.score);
            println!("pub_score: {:.1}", vr.reports.r5.pub_score);
            println!("spec_score: {:.1}", vr.reports.r5.spec_score);
            println!("interop_score: {:.1}", vr.reports.r5.interop_score);
            println!("quality_score: {:.1}", vr.reports.r5.quality_score);
            println!("analytics_score: {:.1}", vr.reports.r5.analytics_score);
        }
    }
}

fn exit_code(result: &ValidateResult) -> ExitCode {
    match result {
        ValidateResult::Fatal(_) => ExitCode::from(2),
        ValidateResult::Ok(vr) if vr.notices.is_empty() => ExitCode::SUCCESS,
        ValidateResult::Ok(_) => ExitCode::from(1),
    }
}

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
