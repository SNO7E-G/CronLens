//! `cronlens` — the command-line front end. A thin shell over
//! [`cronlens_core`]: it parses arguments, renders output (respecting
//! `NO_COLOR` and pipe/TTY detection), and maps results to CI-friendly exit
//! codes.

use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use cronlens_core::ast::Dialect;
use cronlens_core::describe::DescribeOptions;
use cronlens_core::parser::ParseOptions;

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

    /// Use a 24-hour clock in descriptions.
    #[arg(long)]
    use_24h: bool,

    /// Force-disable colored output (also honors NO_COLOR).
    #[arg(long, global = true)]
    no_color: bool,
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

    match run_translate(expr, &cli) {
        Ok(()) => ExitCode::from(exit::OK),
        Err(code) => ExitCode::from(code),
    }
}

fn run_translate(expr: &str, cli: &Cli) -> Result<(), u8> {
    let mut parse_opts = ParseOptions::new();
    parse_opts.dialect = cli.dialect.map(Into::into);

    let parsed = match cronlens_core::parse_with(expr, &parse_opts) {
        Ok(parsed) => parsed,
        Err(err) => {
            render::print_parse_error(expr, &err);
            return Err(exit::INVALID);
        }
    };

    let describe_opts = DescribeOptions {
        verbose: cli.verbose,
        use_24h: cli.use_24h,
        ..DescribeOptions::default()
    };

    let text = cronlens_core::describe_with(&parsed, &describe_opts);
    println!("{text}");
    Ok(())
}
