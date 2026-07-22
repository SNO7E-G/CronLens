//! Terminal rendering helpers: colored, TTY-aware output that degrades to plain
//! text when piped or when `NO_COLOR` is set (via `anstream`).

use anstyle::{AnsiColor, Color, Style};
use cronlens_core::error::CronError;

const RED: Style = Style::new()
    .fg_color(Some(Color::Ansi(AnsiColor::Red)))
    .bold();
const DIM: Style = Style::new().dimmed();

/// Print a parse error to stderr, underlining the offending token when the
/// error carries a span.
pub fn print_parse_error(source: &str, err: &CronError) {
    eprintln!("{RED}✗ invalid cron expression{RED:#}: {err}");

    if let Some(span) = err.span() {
        // Cron tokens are ASCII, so byte offsets line up with columns.
        let start = span.start.min(source.len());
        let end = span.end.min(source.len()).max(start);
        let indent = " ".repeat(start);
        let width = end.saturating_sub(start).max(1);
        let carets = "^".repeat(width);
        eprintln!("  {DIM}{source}{DIM:#}");
        eprintln!("  {indent}{RED}{carets}{RED:#}");
    }
}
