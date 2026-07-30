//! `cronlens` — the command-line front end. A thin shell over
//! [`cronlens_core`]: it parses arguments, renders output (respecting
//! `NO_COLOR` and pipe/TTY detection), and maps results to CI-friendly exit
//! codes.

use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use cronlens_core::ast::Dialect;
use cronlens_core::describe::DescribeOptions;
use cronlens_core::parser::ParseOptions;
use cronlens_core::{CronExpr, DstWarning, DstWarningKind, Run, RunIterator, RunKind, Tz};

mod clock;
mod render;

/// Exit codes, chosen so `cronlens` composes in CI.
///
/// `WARN` and `ERROR` are wired up as later phases add warnings and I/O.
#[allow(dead_code)]
mod exit {
    /// Success, no warnings.
    pub const OK: u8 = 0;
    /// Completed, but with warnings.
    pub const WARN: u8 = 1;
    /// An operational error.
    pub const ERROR: u8 = 2;
    /// The input expression was invalid.
    pub const INVALID: u8 = 3;
}

/// cron for humans — with a safety net.
///
/// Paste a cron expression to get a plain-English translation. Timing,
/// timezone/DST warnings, overlap detection and crontab linting arrive in
/// later releases.
#[derive(Debug, Parser)]
#[command(name = "cronlens", version, about, long_about = None)]
#[command(disable_help_subcommand = true)]
struct Cli {
    /// The cron expression, e.g. "0 9 * * 1-5".
    expr: Option<String>,

    /// Show a field-by-field breakdown.
    #[arg(short, long)]
    verbose: bool,

    /// Pin the input dialect instead of auto-detecting it.
    #[arg(short, long, value_enum)]
    dialect: Option<DialectArg>,

    /// Use a 24-hour clock in descriptions and the run list.
    #[arg(long)]
    use_24h: bool,

    /// Number of upcoming runs to list (0 shows only the translation).
    #[arg(short = 'n', long, default_value_t = 5)]
    next: usize,

    /// Target IANA timezone for the run list (default: the system zone).
    #[arg(long, value_name = "IANA")]
    tz: Option<String>,

    /// Shortcut for --tz UTC.
    #[arg(long)]
    utc: bool,

    /// Compute runs from this local datetime instead of now
    /// (e.g. 2026-07-22 or "2026-07-22 14:30").
    #[arg(long, value_name = "DATETIME")]
    from: Option<String>,

    /// Compare the expression against another one (before → after), for
    /// reviewing a schedule change.
    #[arg(long, value_name = "EXPR")]
    diff: Option<String>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// Force-disable colored output (also honors NO_COLOR).
    #[arg(long, global = true)]
    no_color: bool,
}

/// Output format for the default command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    Text,
    Json,
}

/// CLI-facing mirror of [`Dialect`] so `core` stays free of a clap dependency.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum DialectArg {
    Posix,
    Quartz,
    Aws,
    #[value(name = "k8s", alias = "kubernetes")]
    Kubernetes,
    #[value(name = "github-actions", alias = "gha")]
    GithubActions,
}

impl From<DialectArg> for Dialect {
    fn from(value: DialectArg) -> Self {
        match value {
            DialectArg::Posix => Dialect::Posix,
            DialectArg::Quartz => Dialect::Quartz,
            DialectArg::Aws => Dialect::Aws,
            DialectArg::Kubernetes => Dialect::Kubernetes,
            DialectArg::GithubActions => Dialect::GithubActions,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if cli.no_color {
        anstream::ColorChoice::Never.write_global();
    }

    let Some(expr) = cli.expr.as_deref() else {
        // No expression: print help and signal misuse.
        let _ = <Cli as clap::CommandFactory>::command().print_help();
        println!();
        return ExitCode::from(exit::INVALID);
    };

    ExitCode::from(run(expr, &cli))
}

fn run(expr: &str, cli: &Cli) -> u8 {
    let mut parse_opts = ParseOptions::new();
    parse_opts.dialect = cli.dialect.map(Into::into);

    if let Some(other) = cli.diff.as_deref() {
        return run_diff(expr, other, cli, &parse_opts);
    }

    let parsed = match cronlens_core::parse_with(expr, &parse_opts) {
        Ok(parsed) => parsed,
        // `@reboot` and friends are valid input with no schedule to translate.
        Err(err @ cronlens_core::CronError::Unschedulable { .. }) => {
            if cli.format == Format::Json {
                print_json(&serde_json::json!({
                    "expression": expr, "valid": true, "unschedulable": true,
                    "message": err.to_string(),
                }));
            } else {
                println!("{err}");
            }
            return exit::OK;
        }
        Err(err) => {
            if cli.format == Format::Json {
                print_json(&serde_json::json!({
                    "expression": expr, "valid": false, "error": err.to_string(),
                }));
            } else {
                render::print_parse_error(expr, &err);
            }
            return exit::INVALID;
        }
    };

    let describe_opts = DescribeOptions {
        verbose: cli.verbose,
        use_24h: cli.use_24h,
        ..DescribeOptions::default()
    };
    let description = cronlens_core::describe_with(&parsed, &describe_opts);

    // Runs and DST warnings share the same anchor/zone; compute both when a run
    // list was requested.
    let (tz, runs, warnings) = if cli.next > 0 {
        let tz = match clock::resolve_tz(cli.tz.as_deref(), cli.utc) {
            Ok(tz) => tz,
            Err(msg) => {
                eprintln!("{msg}");
                return exit::ERROR;
            }
        };
        let anchor = match clock::resolve_anchor(cli.from.as_deref(), tz) {
            Ok(anchor) => anchor,
            Err(msg) => {
                eprintln!("{msg}");
                return exit::ERROR;
            }
        };
        let runs = cronlens_core::next_runs(&parsed, anchor, cli.next);
        let warnings = cronlens_core::dst_warnings(&parsed, anchor, cli.next);
        (Some(tz), runs, warnings)
    } else {
        (None, Vec::new(), Vec::new())
    };

    match cli.format {
        Format::Json => print_json(&build_json(
            expr,
            &parsed,
            &description,
            tz,
            &runs,
            &warnings,
        )),
        Format::Text => {
            println!("{description}");
            if let Some(tz) = tz {
                render::print_runs(&runs, tz, cli.use_24h, parsed.second.is_some());
                render::print_warnings(&warnings);
            }
            if parsed.day_fields_are_or() {
                render::print_or_trap_note();
            }
        }
    }

    if warnings.is_empty() {
        exit::OK
    } else {
        exit::WARN
    }
}

/// Compare two expressions: their English, and how often each fires per year.
fn run_diff(before_expr: &str, after_expr: &str, cli: &Cli, opts: &ParseOptions) -> u8 {
    let before = match cronlens_core::parse_with(before_expr, opts) {
        Ok(p) => p,
        Err(err) => {
            render::print_parse_error(before_expr, &err);
            return exit::INVALID;
        }
    };
    let after = match cronlens_core::parse_with(after_expr, opts) {
        Ok(p) => p,
        Err(err) => {
            render::print_parse_error(after_expr, &err);
            return exit::INVALID;
        }
    };

    let tz = match clock::resolve_tz(cli.tz.as_deref(), cli.utc) {
        Ok(tz) => tz,
        Err(msg) => {
            eprintln!("{msg}");
            return exit::ERROR;
        }
    };
    let dopts = DescribeOptions {
        use_24h: cli.use_24h,
        ..DescribeOptions::default()
    };

    println!("Before: {}", cronlens_core::describe_with(&before, &dopts));
    println!("After:  {}", cronlens_core::describe_with(&after, &dopts));

    let n = runs_per_year(&before, tz);
    let m = runs_per_year(&after, tz);
    let delta = m as i64 - n as i64;
    let delta = match delta {
        0 => "no change".to_string(),
        d if d > 0 => format!("+{d}"),
        d => d.to_string(),
    };
    println!("Δ Before fires {n} times/year; after fires {m} ({delta}).");
    exit::OK
}

/// Count firing runs within the next 365 days from now, in `tz`.
fn runs_per_year(expr: &CronExpr, tz: Tz) -> usize {
    use cronlens_core::chrono::{Duration, Utc};
    let now = Utc::now().with_timezone(&tz);
    let bound = now.naive_local() + Duration::days(365);
    RunIterator::new(expr, now)
        .take_while(|r| r.wall < bound)
        .filter(Run::fires)
        .count()
}

fn print_json(value: &serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("json serialization is infallible here")
    );
}

fn build_json(
    expr: &str,
    parsed: &CronExpr,
    description: &str,
    tz: Option<Tz>,
    runs: &[Run],
    warnings: &[DstWarning],
) -> serde_json::Value {
    serde_json::json!({
        "expression": expr,
        "valid": true,
        "dialect": parsed.dialect.label(),
        "description": description,
        "or_trap": parsed.day_fields_are_or(),
        "timezone": tz.map(|t| t.name()),
        "runs": runs.iter().map(|r| serde_json::json!({
            "local": r.local().map(|l| l.to_rfc3339()),
            "utc": r.utc().map(|u| u.to_rfc3339()),
            "ambiguous": matches!(r.kind, RunKind::Ambiguous { .. }),
        })).collect::<Vec<_>>(),
        "warnings": warnings.iter().map(|w| serde_json::json!({
            "wall": w.wall.format("%Y-%m-%dT%H:%M:%S").to_string(),
            "kind": match w.kind {
                DstWarningKind::Skipped => "skipped",
                DstWarningKind::Doubled => "doubled",
            },
        })).collect::<Vec<_>>(),
    })
}
