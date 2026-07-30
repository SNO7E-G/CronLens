//! Terminal rendering helpers: colored, TTY-aware output that degrades to plain
//! text when piped or when `NO_COLOR` is set (via `anstream`).

use anstyle::{AnsiColor, Color, Style};
use cronlens_core::error::CronError;
use cronlens_core::{DstWarning, DstWarningKind, Run, RunKind, Tz};

const RED: Style = Style::new()
    .fg_color(Some(Color::Ansi(AnsiColor::Red)))
    .bold();
const YELLOW: Style = Style::new()
    .fg_color(Some(Color::Ansi(AnsiColor::Yellow)))
    .bold();
const CYAN: Style = Style::new()
    .fg_color(Some(Color::Ansi(AnsiColor::Cyan)))
    .bold();
const DIM: Style = Style::new().dimmed();

/// Note the day-of-month / day-of-week OR-trap: the classic surprise where
/// setting both day fields fires on *either*, not both.
pub fn print_or_trap_note() {
    anstream::println!(
        "\n{CYAN}ⓘ note{CYAN:#} both day-of-month and day-of-week are set — this fires when EITHER matches (day-of-month OR day-of-week), which surprises most people."
    );
}

/// Print DST warnings (spring-forward skips, fall-back doubles). Colored via
/// `anstream`, so it degrades cleanly when piped or under `NO_COLOR`.
pub fn print_warnings(warnings: &[DstWarning]) {
    if warnings.is_empty() {
        return;
    }
    println!();
    for w in warnings {
        let wall = w.wall.format("%Y-%m-%d %H:%M");
        match w.kind {
            DstWarningKind::Skipped => anstream::println!(
                "{YELLOW}⚠{YELLOW:#} {wall} does not exist (spring-forward) — this run will be SKIPPED."
            ),
            DstWarningKind::Doubled => anstream::println!(
                "{YELLOW}⚠{YELLOW:#} {wall} occurs twice (fall-back) — this run may fire TWICE."
            ),
        }
    }
}

/// Print a parse error to stderr, underlining the offending token when the
/// error carries a span. Routed through `anstream`, so color is stripped
/// automatically when stderr is not a TTY and when `NO_COLOR` is set.
pub fn print_parse_error(source: &str, err: &CronError) {
    match err {
        // A validation pass can return several field errors at once.
        CronError::Multiple(errors) => {
            anstream::eprintln!(
                "{RED}✗ invalid cron expression{RED:#} ({} problems):",
                errors.len()
            );
            for error in errors {
                print_one(source, error);
            }
        }
        other => {
            anstream::eprint!("{RED}✗ invalid cron expression{RED:#}: ");
            print_one(source, other);
        }
    }
}

/// Print a numbered list of upcoming runs in the target zone. Ambiguous
/// (fall-back) runs are flagged; nonexistent runs are already filtered out.
pub fn print_runs(runs: &[Run], tz: Tz, use_24h: bool, show_seconds: bool) {
    println!();
    if runs.is_empty() {
        println!("No upcoming runs found within the search horizon.");
        return;
    }

    let n = runs.len();
    println!(
        "Next {n} run{} ({}):",
        if n == 1 { "" } else { "s" },
        tz.name()
    );

    let fmt = match (use_24h, show_seconds) {
        (true, true) => "%a %Y-%m-%d %H:%M:%S %Z",
        (true, false) => "%a %Y-%m-%d %H:%M %Z",
        (false, true) => "%a %Y-%m-%d %I:%M:%S %p %Z",
        (false, false) => "%a %Y-%m-%d %I:%M %p %Z",
    };

    for (i, run) in runs.iter().enumerate() {
        let Some(local) = run.local() else { continue };
        let note = if matches!(run.kind, RunKind::Ambiguous { .. }) {
            "  (ambiguous — fall-back, fires twice)"
        } else {
            ""
        };
        println!("  {:>2}. {}{}", i + 1, local.format(fmt), note);
    }
}

/// Print a single error and, when it carries a span, underline the offending
/// token beneath the source.
fn print_one(source: &str, err: &CronError) {
    anstream::eprintln!("{err}");
    if let Some(span) = err.span() {
        // Cron tokens are ASCII, so byte offsets line up with columns.
        let start = span.start.min(source.len());
        let end = span.end.min(source.len()).max(start);
        let indent = " ".repeat(start);
        let width = end.saturating_sub(start).max(1);
        let carets = "^".repeat(width);
        anstream::eprintln!("  {DIM}{source}{DIM:#}");
        anstream::eprintln!("  {indent}{RED}{carets}{RED:#}");
    }
}
