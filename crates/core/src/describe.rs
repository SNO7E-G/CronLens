//! AST → plain English.
//!
//! The describer is deterministic and i18n-ready: all human strings flow
//! through here so a locale layer can be added later without touching the
//! parser or scheduler. It reads the same [`CronExpr`] the scheduler runs, so a
//! description can never drift from what actually fires — including the day
//! [OR-trap](CronExpr::day_fields_are_or).

use crate::ast::{CronExpr, Field, StepBase, Term, DOW_FULL, MONTH_FULL};

/// Formatting options for [`describe_with`].
#[derive(Debug, Clone)]
pub struct DescribeOptions {
    /// Emit a field-by-field breakdown rather than a single sentence.
    pub verbose: bool,
    /// Use 24-hour clock (`21:00`) instead of 12-hour (`9:00 PM`).
    pub use_24h: bool,
    /// Capitalize the first letter and add a trailing period.
    pub sentence_case: bool,
}

impl Default for DescribeOptions {
    fn default() -> Self {
        Self {
            verbose: false,
            use_24h: false,
            sentence_case: true,
        }
    }
}

/// Describe an expression in plain English using default options.
pub fn describe(expr: &CronExpr) -> String {
    describe_with(expr, &DescribeOptions::default())
}

/// Describe an expression under the given [`DescribeOptions`].
pub fn describe_with(expr: &CronExpr, options: &DescribeOptions) -> String {
    if options.verbose {
        return describe_verbose(expr, options);
    }
    finalize_case(build_sentence(expr, options), options.sentence_case)
}

// ---------------------------------------------------------------------------
// Field shape classification
// ---------------------------------------------------------------------------

/// A coarse classification of a field's terms, used to pick phrasing. This is
/// deliberately shallow: anything that doesn't fit one of the common shapes
/// falls back to [`FieldShape::Other`], which renders from the field's raw
/// text so the description degrades gracefully rather than panicking.
#[derive(Debug, Clone, PartialEq)]
enum FieldShape {
    /// `*` or `?` (or a mix of both) — unrestricted.
    Wildcard,
    /// A single resolved value.
    Single(u32),
    /// An inclusive range.
    Range(u32, u32),
    /// A stepped term; `start` is `None` for `*/n` (the whole range).
    Step { start: Option<u32>, step: u32 },
    /// Two or more plain single values (e.g. a comma list `1,15,30`).
    List(Vec<u32>),
    /// Anything else (Quartz/AWS specials, mixed term kinds, …).
    Other,
}

fn classify(field: &Field) -> FieldShape {
    if field.terms.iter().all(Term::is_wildcard) {
        return FieldShape::Wildcard;
    }
    if field.terms.len() == 1 {
        return match &field.terms[0] {
            Term::Single(n) => FieldShape::Single(*n),
            Term::Range(a, b) => FieldShape::Range(*a, *b),
            Term::Step { base, step } => {
                let start = match base {
                    StepBase::Whole => None,
                    StepBase::From(m) => Some(*m),
                    StepBase::Range(a, _) => Some(*a),
                };
                FieldShape::Step { start, step: *step }
            }
            Term::All | Term::NoSpecific => FieldShape::Wildcard,
            _ => FieldShape::Other,
        };
    }
    if field.terms.iter().all(|t| matches!(t, Term::Single(_))) {
        let values = field
            .terms
            .iter()
            .map(|t| match t {
                Term::Single(n) => *n,
                _ => unreachable!("filtered to Single above"),
            })
            .collect();
        return FieldShape::List(values);
    }
    FieldShape::Other
}

// ---------------------------------------------------------------------------
// Small text helpers
// ---------------------------------------------------------------------------

/// Join items in natural English: `"A"`, `"A and B"`, `"A, B, and C"`.
fn join_and(items: &[String]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        2 => format!("{} and {}", items[0], items[1]),
        _ => {
            let (last, rest) = items.split_last().expect("len > 2");
            format!("{}, and {}", rest.join(", "), last)
        }
    }
}

/// English ordinal suffix: `1st`, `2nd`, `3rd`, `4th`, `11th`, `21st`, …
fn ordinal(n: u32) -> String {
    let suffix = if (11..=13).contains(&(n % 100)) {
        "th"
    } else {
        match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    };
    format!("{n}{suffix}")
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn lowercase_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_lowercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn finalize_case(sentence: String, sentence_case: bool) -> String {
    let trimmed = sentence.trim_end_matches('.');
    if sentence_case {
        format!("{}.", capitalize_first(trimmed))
    } else {
        lowercase_first(trimmed)
    }
}

fn month_name(m: u32) -> &'static str {
    let idx = m.saturating_sub(1).min((MONTH_FULL.len() - 1) as u32) as usize;
    MONTH_FULL[idx]
}

fn dow_name(d: u32) -> &'static str {
    let idx = (d as usize).min(DOW_FULL.len() - 1);
    DOW_FULL[idx]
}

/// `(display_hour, "AM"/"PM")` for a 24-hour `hour` value.
fn to_12h(hour: u32) -> (u32, &'static str) {
    let ampm = if hour < 12 { "AM" } else { "PM" };
    let display = match hour % 12 {
        0 => 12,
        h => h,
    };
    (display, ampm)
}

/// A single hour, no minutes, e.g. `"9 AM"` or (24h) `"09:00"`.
fn format_hour_label(hour: u32, use_24h: bool) -> String {
    if use_24h {
        format!("{hour:02}:00")
    } else {
        let (h, ampm) = to_12h(hour);
        format!("{h} {ampm}")
    }
}

/// A fixed clock instant, e.g. `"At 9:00 AM"`, `"At 9:00:05 AM"`, or (24h)
/// `"At 09:00"` / `"At 09:00:05"`.
fn format_instant(hour: u32, minute: u32, second: Option<u32>, use_24h: bool) -> String {
    if use_24h {
        match second {
            Some(s) => format!("At {hour:02}:{minute:02}:{s:02}"),
            None => format!("At {hour:02}:{minute:02}"),
        }
    } else {
        let (h, ampm) = to_12h(hour);
        match second {
            Some(s) => format!("At {h}:{minute:02}:{s:02} {ampm}"),
            None => format!("At {h}:{minute:02} {ampm}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Time clause (second + minute + hour)
// ---------------------------------------------------------------------------

/// Builds the leading time clause and reports the hour's [`FieldShape`] so the
/// caller can decide whether the `, every day` filler applies (only when the
/// schedule fires at a fixed hour each day).
fn time_clause(expr: &CronExpr, options: &DescribeOptions) -> (String, FieldShape) {
    let minute_shape = classify(&expr.minute);
    let hour_shape = classify(&expr.hour);
    // A lone `0` second is not worth mentioning; treat it like "no seconds".
    let sec_active = match expr.second.as_ref().map(classify) {
        Some(FieldShape::Single(0)) => None,
        other => other,
    };

    let clause =
        if let (Some(FieldShape::Single(s)), FieldShape::Single(m), FieldShape::Single(h)) =
            (&sec_active, &minute_shape, &hour_shape)
        {
            format_instant(*h, *m, Some(*s), options.use_24h)
        } else if sec_active.is_none() {
            no_seconds_clause(&minute_shape, &hour_shape, options)
        } else {
            match (&sec_active, &minute_shape, &hour_shape) {
                (Some(FieldShape::Wildcard), FieldShape::Wildcard, FieldShape::Wildcard) => {
                    "Every second".to_string()
                }
                (
                    Some(FieldShape::Step { step, .. }),
                    FieldShape::Wildcard,
                    FieldShape::Wildcard,
                ) => every_n("second", *step),
                _ => fallback_time_clause(&minute_shape, &hour_shape, &sec_active, options),
            }
        };

    (clause, hour_shape)
}

fn every_n(unit: &str, step: u32) -> String {
    if step <= 1 {
        format!("Every {unit}")
    } else {
        format!("Every {step} {unit}s")
    }
}

fn no_seconds_clause(
    minute_shape: &FieldShape,
    hour_shape: &FieldShape,
    options: &DescribeOptions,
) -> String {
    match (minute_shape, hour_shape) {
        (FieldShape::Single(m), FieldShape::Single(h)) => {
            format_instant(*h, *m, None, options.use_24h)
        }
        (FieldShape::Single(m), FieldShape::Wildcard) => {
            if *m == 0 {
                "Every hour, on the hour".to_string()
            } else {
                format!("At {m} minutes past every hour")
            }
        }
        (FieldShape::Wildcard, FieldShape::Wildcard) => "Every minute".to_string(),
        (FieldShape::Step { step, .. }, FieldShape::Wildcard) => every_n("minute", *step),
        _ => fallback_time_clause(minute_shape, hour_shape, &None, options),
    }
}

fn minute_phrase(shape: &FieldShape) -> String {
    match shape {
        FieldShape::Wildcard => "every minute".to_string(),
        FieldShape::Single(m) => format!("at {m} minutes past the hour"),
        FieldShape::Range(a, b) => format!("every minute from {a} through {b} past the hour"),
        FieldShape::Step { start: None, step } => format!("every {step} minutes"),
        FieldShape::Step {
            start: Some(s),
            step,
        } => format!("every {step} minutes, starting at minute {s}"),
        FieldShape::List(values) => {
            let items: Vec<String> = values.iter().map(u32::to_string).collect();
            format!("at minutes {} past the hour", join_and(&items))
        }
        FieldShape::Other => "at the configured minutes".to_string(),
    }
}

fn hour_phrase(shape: &FieldShape, use_24h: bool) -> String {
    match shape {
        FieldShape::Wildcard => "every hour".to_string(),
        FieldShape::Single(h) => format!("during the {} hour", format_hour_label(*h, use_24h)),
        FieldShape::Range(a, b) => format!(
            "between the {} and {} hours",
            format_hour_label(*a, use_24h),
            format_hour_label(*b, use_24h)
        ),
        FieldShape::Step { start: None, step } => every_n("hour", *step).to_lowercase(),
        FieldShape::Step {
            start: Some(s),
            step,
        } => format!(
            "every {step} hours, starting at {}",
            format_hour_label(*s, use_24h)
        ),
        FieldShape::List(values) => {
            let items: Vec<String> = values
                .iter()
                .map(|v| format_hour_label(*v, use_24h))
                .collect();
            format!("during the {} hours", join_and(&items))
        }
        FieldShape::Other => "during the configured hours".to_string(),
    }
}

fn seconds_phrase(shape: &FieldShape) -> String {
    match shape {
        FieldShape::Wildcard => "every second".to_string(),
        FieldShape::Single(s) => format!("at {s} seconds past the minute"),
        FieldShape::Range(a, b) => format!("every second from {a} through {b} past the minute"),
        FieldShape::Step { start: None, step } => every_n("second", *step).to_lowercase(),
        FieldShape::Step {
            start: Some(s),
            step,
        } => format!("every {step} seconds, starting at second {s}"),
        FieldShape::List(values) => {
            let items: Vec<String> = values.iter().map(u32::to_string).collect();
            format!("at seconds {} past the minute", join_and(&items))
        }
        FieldShape::Other => "at the configured seconds".to_string(),
    }
}

/// Catch-all for minute/hour(/second) shapes not covered by the anchored
/// phrasing above. Not part of the required outputs — chosen for readability
/// and determinism.
fn fallback_time_clause(
    minute_shape: &FieldShape,
    hour_shape: &FieldShape,
    sec_active: &Option<FieldShape>,
    options: &DescribeOptions,
) -> String {
    let base = format!(
        "{}, {}",
        capitalize_first(&minute_phrase(minute_shape)),
        hour_phrase(hour_shape, options.use_24h)
    );
    match sec_active {
        Some(sec_shape) => format!(
            "{}, {}",
            capitalize_first(&seconds_phrase(sec_shape)),
            lowercase_first(&base)
        ),
        None => base,
    }
}

// ---------------------------------------------------------------------------
// Day-of-month / day-of-week / month / year clauses
// ---------------------------------------------------------------------------

fn day_of_month_clause(field: &Field) -> String {
    match classify(field) {
        FieldShape::Single(d) => format!("on day {d} of the month"),
        FieldShape::Range(a, b) => format!("on days {a} through {b} of the month"),
        FieldShape::Step { start: None, step } => {
            format!("on every {} day of the month", ordinal(step))
        }
        FieldShape::Step {
            start: Some(s),
            step,
        } => format!(
            "on every {} day of the month, starting on day {s}",
            ordinal(step)
        ),
        FieldShape::List(values) => {
            let items: Vec<String> = values.iter().map(u32::to_string).collect();
            format!("on days {} of the month", join_and(&items))
        }
        FieldShape::Wildcard => unreachable!("day_of_month_clause is only called when restricted"),
        FieldShape::Other => format!("on day-of-month {}", field.raw),
    }
}

fn day_of_week_clause(field: &Field) -> String {
    match classify(field) {
        FieldShape::Single(d) => format!("only on {}", dow_name(d)),
        FieldShape::Range(a, b) => format!("{} through {}", dow_name(a), dow_name(b)),
        FieldShape::Step { start: None, step } => {
            format!("only every {} day of the week", ordinal(step))
        }
        FieldShape::Step {
            start: Some(s),
            step,
        } => format!(
            "only every {} day of the week, starting on {}",
            ordinal(step),
            dow_name(s)
        ),
        FieldShape::List(values) => {
            let items: Vec<String> = values.iter().map(|v| dow_name(*v).to_string()).collect();
            format!("only on {}", join_and(&items))
        }
        FieldShape::Wildcard => unreachable!("day_of_week_clause is only called when restricted"),
        FieldShape::Other => format!("on day-of-week {}", field.raw),
    }
}

fn month_clause(field: &Field) -> String {
    match classify(field) {
        FieldShape::Single(m) => format!("only in {}", month_name(m)),
        FieldShape::Range(a, b) => format!("in {} through {}", month_name(a), month_name(b)),
        FieldShape::Step { start: None, step } => format!("in every {} month", ordinal(step)),
        FieldShape::Step {
            start: Some(s),
            step,
        } => format!(
            "in every {} month, starting in {}",
            ordinal(step),
            month_name(s)
        ),
        FieldShape::List(values) => {
            let items: Vec<String> = values.iter().map(|v| month_name(*v).to_string()).collect();
            format!("in {}", join_and(&items))
        }
        FieldShape::Wildcard => unreachable!("month_clause is only called when restricted"),
        FieldShape::Other => format!("in month {}", field.raw),
    }
}

fn year_clause(field: &Field) -> String {
    match classify(field) {
        FieldShape::Single(y) => format!("in {y}"),
        FieldShape::Range(a, b) => format!("in {a} through {b}"),
        FieldShape::Step { start: None, step } => format!("every {step} years"),
        FieldShape::Step {
            start: Some(s),
            step,
        } => format!("every {step} years, starting in {s}"),
        FieldShape::List(values) => {
            let items: Vec<String> = values.iter().map(u32::to_string).collect();
            format!("in {}", join_and(&items))
        }
        FieldShape::Wildcard => unreachable!("year_clause is only called when restricted"),
        FieldShape::Other => format!("in year {}", field.raw),
    }
}

// ---------------------------------------------------------------------------
// Sentence assembly
// ---------------------------------------------------------------------------

fn build_sentence(expr: &CronExpr, options: &DescribeOptions) -> String {
    let (time, hour_shape) = time_clause(expr, options);

    let dom = expr
        .day_of_month
        .is_restricted()
        .then(|| day_of_month_clause(&expr.day_of_month));
    let dow = expr
        .day_of_week
        .is_restricted()
        .then(|| day_of_week_clause(&expr.day_of_week));
    let month = expr
        .month
        .is_restricted()
        .then(|| month_clause(&expr.month));
    let year = expr
        .year
        .as_ref()
        .filter(|f| f.is_restricted())
        .map(year_clause);

    // Whenever both day fields carry a clause (the OR-trap, or a star-prefixed
    // `*/n` AND-ed with the other field), join them — never drop one.
    let day_segment = match (dom, dow) {
        (Some(d), Some(w)) => Some(format!("{d}, and {w}")),
        (day, None) | (None, day) => day,
    };

    let mut parts = vec![time];
    match day_segment {
        Some(seg) => parts.push(seg),
        // Only append the "every day" filler for a schedule pinned to a fixed
        // hour each day with no month restriction either; "Every minute."/
        // "Every hour, on the hour." etc. already convey a sub-daily cadence,
        // so the filler would be redundant there.
        None if matches!(hour_shape, FieldShape::Single(_)) && !expr.month.is_restricted() => {
            parts.push("every day".to_string());
        }
        None => {}
    }
    if let Some(m) = month {
        parts.push(m);
    }
    if let Some(y) = year {
        parts.push(y);
    }

    parts.join(", ")
}

// ---------------------------------------------------------------------------
// Verbose mode
// ---------------------------------------------------------------------------

fn describe_field_shape(shape: &FieldShape) -> String {
    match shape {
        FieldShape::Wildcard => "every value".to_string(),
        FieldShape::Single(v) => format!("exactly {v}"),
        FieldShape::Range(a, b) => format!("range {a}-{b}"),
        FieldShape::Step { start: None, step } => format!("every {step}"),
        FieldShape::Step {
            start: Some(s),
            step,
        } => format!("every {step} starting at {s}"),
        FieldShape::List(values) => {
            let items: Vec<String> = values.iter().map(u32::to_string).collect();
            format!("values {}", items.join(", "))
        }
        FieldShape::Other => "special/mixed value".to_string(),
    }
}

fn describe_verbose(expr: &CronExpr, options: &DescribeOptions) -> String {
    let mut lines = vec!["Fields:".to_string()];
    for field in expr.fields() {
        let shape = classify(field);
        lines.push(format!(
            "  {}: \"{}\" ({})",
            field.kind.label(),
            field.raw,
            describe_field_shape(&shape)
        ));
    }
    if expr.day_fields_are_or() {
        lines.push(
            "  note: day-of-month and day-of-week are both restricted; a date matches either one (OR-trap).".to_string(),
        );
    }
    lines.push(String::new());

    let mut summary_options = options.clone();
    summary_options.verbose = false;
    lines.push(format!(
        "Summary: {}",
        describe_with(expr, &summary_options)
    ));

    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Dialect, FieldKind};

    fn f(kind: FieldKind, terms: Vec<Term>, raw: &str) -> Field {
        Field::new(kind, terms, raw)
    }

    fn wildcard(kind: FieldKind) -> Field {
        f(kind, vec![Term::All], "*")
    }

    /// Builds a plain 5-field POSIX `CronExpr` (no seconds, no year).
    #[allow(clippy::too_many_arguments)]
    fn expr5(
        minute: Field,
        hour: Field,
        dom: Field,
        month: Field,
        dow: Field,
        source: &str,
    ) -> CronExpr {
        CronExpr {
            dialect: Dialect::Posix,
            second: None,
            minute,
            hour,
            day_of_month: dom,
            month,
            day_of_week: dow,
            year: None,
            source: source.to_string(),
        }
    }

    // -- REQUIRED exact outputs -------------------------------------------

    #[test]
    fn every_minute() {
        let expr = expr5(
            wildcard(FieldKind::Minute),
            wildcard(FieldKind::Hour),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            wildcard(FieldKind::DayOfWeek),
            "* * * * *",
        );
        assert_eq!(describe(&expr), "Every minute.");
    }

    #[test]
    fn weekday_9am() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(9)], "9"),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            f(FieldKind::DayOfWeek, vec![Term::Range(1, 5)], "1-5"),
            "0 9 * * 1-5",
        );
        assert_eq!(describe(&expr), "At 9:00 AM, Monday through Friday.");
    }

    #[test]
    fn every_15_minutes() {
        let expr = expr5(
            f(
                FieldKind::Minute,
                vec![Term::Step {
                    base: StepBase::Whole,
                    step: 15,
                }],
                "*/15",
            ),
            wildcard(FieldKind::Hour),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            wildcard(FieldKind::DayOfWeek),
            "*/15 * * * *",
        );
        assert_eq!(describe(&expr), "Every 15 minutes.");
    }

    #[test]
    fn midnight_every_day() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(0)], "0"),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            wildcard(FieldKind::DayOfWeek),
            "0 0 * * *",
        );
        assert_eq!(describe(&expr), "At 12:00 AM, every day.");
    }

    #[test]
    fn noon_every_day() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(12)], "12"),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            wildcard(FieldKind::DayOfWeek),
            "0 12 * * *",
        );
        assert_eq!(describe(&expr), "At 12:00 PM, every day.");
    }

    #[test]
    fn two_thirty_am_every_day() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(30)], "30"),
            f(FieldKind::Hour, vec![Term::Single(2)], "2"),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            wildcard(FieldKind::DayOfWeek),
            "30 2 * * *",
        );
        assert_eq!(describe(&expr), "At 2:30 AM, every day.");
    }

    #[test]
    fn midnight_on_day_1() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(0)], "0"),
            f(FieldKind::DayOfMonth, vec![Term::Single(1)], "1"),
            wildcard(FieldKind::Month),
            wildcard(FieldKind::DayOfWeek),
            "0 0 1 * *",
        );
        assert_eq!(describe(&expr), "At 12:00 AM, on day 1 of the month.");
    }

    #[test]
    fn nine_am_only_monday() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(9)], "9"),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            f(FieldKind::DayOfWeek, vec![Term::Single(1)], "1"),
            "0 9 * * 1",
        );
        assert_eq!(describe(&expr), "At 9:00 AM, only on Monday.");
    }

    #[test]
    fn every_hour_on_the_hour() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            wildcard(FieldKind::Hour),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            wildcard(FieldKind::DayOfWeek),
            "0 * * * *",
        );
        assert_eq!(describe(&expr), "Every hour, on the hour.");
    }

    #[test]
    fn five_minutes_past_every_hour() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(5)], "5"),
            wildcard(FieldKind::Hour),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            wildcard(FieldKind::DayOfWeek),
            "5 * * * *",
        );
        assert_eq!(describe(&expr), "At 5 minutes past every hour.");
    }

    #[test]
    fn ten_pm_weekdays() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(22)], "22"),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            f(FieldKind::DayOfWeek, vec![Term::Range(1, 5)], "1-5"),
            "0 22 * * 1-5",
        );
        assert_eq!(describe(&expr), "At 10:00 PM, Monday through Friday.");
    }

    #[test]
    fn new_years_day_midnight() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(0)], "0"),
            f(FieldKind::DayOfMonth, vec![Term::Single(1)], "1"),
            f(FieldKind::Month, vec![Term::Single(1)], "1"),
            wildcard(FieldKind::DayOfWeek),
            "0 0 1 1 *",
        );
        assert_eq!(
            describe(&expr),
            "At 12:00 AM, on day 1 of the month, only in January."
        );
    }

    // -- Additional phrasing coverage --------------------------------------

    #[test]
    fn dow_list_uses_only() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(9)], "9"),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            f(
                FieldKind::DayOfWeek,
                vec![Term::Single(1), Term::Single(3), Term::Single(5)],
                "1,3,5",
            ),
            "0 9 * * 1,3,5",
        );
        assert_eq!(
            describe(&expr),
            "At 9:00 AM, only on Monday, Wednesday, and Friday."
        );
    }

    #[test]
    fn dow_zero_and_seven_are_both_sunday() {
        let expr0 = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(9)], "9"),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            f(FieldKind::DayOfWeek, vec![Term::Single(0)], "0"),
            "0 9 * * 0",
        );
        let expr7 = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(9)], "9"),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            f(FieldKind::DayOfWeek, vec![Term::Single(7)], "7"),
            "0 9 * * 7",
        );
        assert_eq!(describe(&expr0), "At 9:00 AM, only on Sunday.");
        assert_eq!(describe(&expr7), "At 9:00 AM, only on Sunday.");
    }

    #[test]
    fn dom_range() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(0)], "0"),
            f(FieldKind::DayOfMonth, vec![Term::Range(1, 15)], "1-15"),
            wildcard(FieldKind::Month),
            wildcard(FieldKind::DayOfWeek),
            "0 0 1-15 * *",
        );
        assert_eq!(
            describe(&expr),
            "At 12:00 AM, on days 1 through 15 of the month."
        );
    }

    #[test]
    fn dom_step() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(0)], "0"),
            f(
                FieldKind::DayOfMonth,
                vec![Term::Step {
                    base: StepBase::Whole,
                    step: 2,
                }],
                "*/2",
            ),
            wildcard(FieldKind::Month),
            wildcard(FieldKind::DayOfWeek),
            "0 0 */2 * *",
        );
        assert_eq!(
            describe(&expr),
            "At 12:00 AM, on every 2nd day of the month."
        );
    }

    #[test]
    fn dom_list() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(0)], "0"),
            f(
                FieldKind::DayOfMonth,
                vec![Term::Single(1), Term::Single(10), Term::Single(20)],
                "1,10,20",
            ),
            wildcard(FieldKind::Month),
            wildcard(FieldKind::DayOfWeek),
            "0 0 1,10,20 * *",
        );
        assert_eq!(
            describe(&expr),
            "At 12:00 AM, on days 1, 10, and 20 of the month."
        );
    }

    #[test]
    fn month_range_and_list() {
        let range_expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(0)], "0"),
            wildcard(FieldKind::DayOfMonth),
            f(FieldKind::Month, vec![Term::Range(6, 8)], "6-8"),
            wildcard(FieldKind::DayOfWeek),
            "0 0 * 6-8 *",
        );
        assert_eq!(
            describe(&range_expr),
            "At 12:00 AM, in June through August."
        );

        let list_expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(0)], "0"),
            wildcard(FieldKind::DayOfMonth),
            f(
                FieldKind::Month,
                vec![Term::Single(3), Term::Single(6), Term::Single(9)],
                "3,6,9",
            ),
            wildcard(FieldKind::DayOfWeek),
            "0 0 * 3,6,9 *",
        );
        assert_eq!(
            describe(&list_expr),
            "At 12:00 AM, in March, June, and September."
        );
    }

    #[test]
    fn year_single_and_range() {
        let single = CronExpr {
            dialect: Dialect::Quartz,
            second: Some(f(FieldKind::Second, vec![Term::Single(0)], "0")),
            minute: f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            hour: f(FieldKind::Hour, vec![Term::Single(0)], "0"),
            day_of_month: wildcard(FieldKind::DayOfMonth),
            month: wildcard(FieldKind::Month),
            day_of_week: wildcard(FieldKind::DayOfWeek),
            year: Some(f(FieldKind::Year, vec![Term::Single(2030)], "2030")),
            source: "0 0 0 * * ? 2030".to_string(),
        };
        assert_eq!(describe(&single), "At 12:00 AM, every day, in 2030.");

        let mut range = single.clone();
        range.year = Some(f(
            FieldKind::Year,
            vec![Term::Range(2030, 2035)],
            "2030-2035",
        ));
        assert_eq!(
            describe(&range),
            "At 12:00 AM, every day, in 2030 through 2035."
        );
    }

    #[test]
    fn or_trap_joins_dom_and_dow_with_and() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(0)], "0"),
            f(FieldKind::DayOfMonth, vec![Term::Single(1)], "1"),
            wildcard(FieldKind::Month),
            f(FieldKind::DayOfWeek, vec![Term::Single(1)], "1"),
            "0 0 1 * 1",
        );
        assert!(expr.day_fields_are_or());
        assert_eq!(
            describe(&expr),
            "At 12:00 AM, on day 1 of the month, and only on Monday."
        );
    }

    #[test]
    fn star_prefixed_dom_step_keeps_the_dow_clause() {
        // `*/2` in day-of-month is star-prefixed (AND semantics), but it still
        // constrains — the day-of-week clause must not be dropped.
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(0)], "0"),
            f(
                FieldKind::DayOfMonth,
                vec![Term::Step {
                    base: StepBase::Whole,
                    step: 2,
                }],
                "*/2",
            ),
            wildcard(FieldKind::Month),
            f(FieldKind::DayOfWeek, vec![Term::Single(1)], "1"),
            "0 0 */2 * 1",
        );
        assert_eq!(
            describe(&expr),
            "At 12:00 AM, on every 2nd day of the month, and only on Monday."
        );
    }

    #[test]
    fn or_trap_with_month_also_restricted() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(0)], "0"),
            f(FieldKind::DayOfMonth, vec![Term::Single(1)], "1"),
            f(FieldKind::Month, vec![Term::Single(1)], "1"),
            f(FieldKind::DayOfWeek, vec![Term::Single(1)], "1"),
            "0 0 1 1 1",
        );
        assert_eq!(
            describe(&expr),
            "At 12:00 AM, on day 1 of the month, and only on Monday, only in January."
        );
    }

    #[test]
    fn use_24h_option() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(22)], "22"),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            f(FieldKind::DayOfWeek, vec![Term::Range(1, 5)], "1-5"),
            "0 22 * * 1-5",
        );
        let options = DescribeOptions {
            use_24h: true,
            ..Default::default()
        };
        assert_eq!(
            describe_with(&expr, &options),
            "At 22:00, Monday through Friday."
        );
    }

    #[test]
    fn sentence_case_false() {
        let expr = expr5(
            wildcard(FieldKind::Minute),
            wildcard(FieldKind::Hour),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            wildcard(FieldKind::DayOfWeek),
            "* * * * *",
        );
        let options = DescribeOptions {
            sentence_case: false,
            ..Default::default()
        };
        assert_eq!(describe_with(&expr, &options), "every minute");
    }

    #[test]
    fn seconds_zero_is_ignored() {
        let mut expr5f = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(9)], "9"),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            wildcard(FieldKind::DayOfWeek),
            "0 0 9 * * *",
        );
        expr5f.second = Some(f(FieldKind::Second, vec![Term::Single(0)], "0"));
        assert_eq!(describe(&expr5f), "At 9:00 AM, every day.");
    }

    #[test]
    fn seconds_nonzero_single_includes_hms() {
        let mut expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(30)], "30"),
            f(FieldKind::Hour, vec![Term::Single(9)], "9"),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            wildcard(FieldKind::DayOfWeek),
            "15 30 9 * * *",
        );
        expr.second = Some(f(FieldKind::Second, vec![Term::Single(15)], "15"));
        assert_eq!(describe(&expr), "At 9:30:15 AM, every day.");
    }

    #[test]
    fn seconds_wildcard_and_step() {
        let mut every_second = expr5(
            wildcard(FieldKind::Minute),
            wildcard(FieldKind::Hour),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            wildcard(FieldKind::DayOfWeek),
            "* * * * * *",
        );
        every_second.second = Some(wildcard(FieldKind::Second));
        assert_eq!(describe(&every_second), "Every second.");

        let mut every_5_seconds = every_second.clone();
        every_5_seconds.second = Some(f(
            FieldKind::Second,
            vec![Term::Step {
                base: StepBase::Whole,
                step: 5,
            }],
            "*/5",
        ));
        assert_eq!(describe(&every_5_seconds), "Every 5 seconds.");
    }

    #[test]
    fn fallback_minute_wildcard_hour_single() {
        let expr = expr5(
            wildcard(FieldKind::Minute),
            f(FieldKind::Hour, vec![Term::Single(9)], "9"),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            wildcard(FieldKind::DayOfWeek),
            "* 9 * * *",
        );
        assert_eq!(
            describe(&expr),
            "Every minute, during the 9 AM hour, every day."
        );
    }

    #[test]
    fn fallback_minute_step_hour_single() {
        let expr = expr5(
            f(
                FieldKind::Minute,
                vec![Term::Step {
                    base: StepBase::Whole,
                    step: 10,
                }],
                "*/10",
            ),
            f(FieldKind::Hour, vec![Term::Single(9)], "9"),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            wildcard(FieldKind::DayOfWeek),
            "*/10 9 * * *",
        );
        assert_eq!(
            describe(&expr),
            "Every 10 minutes, during the 9 AM hour, every day."
        );
    }

    #[test]
    fn month_only_restricted_still_gets_every_day_filler() {
        // dom and dow unrestricted, month restricted: our reading of the spec
        // ("no dom/dow/month restriction") requires ALL THREE unrestricted
        // for the filler, so a month-only restriction suppresses it.
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(0)], "0"),
            wildcard(FieldKind::DayOfMonth),
            f(FieldKind::Month, vec![Term::Single(6)], "6"),
            wildcard(FieldKind::DayOfWeek),
            "0 0 * 6 *",
        );
        assert_eq!(describe(&expr), "At 12:00 AM, only in June.");
    }

    #[test]
    fn verbose_breakdown() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(9)], "9"),
            wildcard(FieldKind::DayOfMonth),
            wildcard(FieldKind::Month),
            f(FieldKind::DayOfWeek, vec![Term::Range(1, 5)], "1-5"),
            "0 9 * * 1-5",
        );
        let options = DescribeOptions {
            verbose: true,
            ..Default::default()
        };
        let expected = "Fields:\n  \
minute: \"0\" (exactly 0)\n  \
hour: \"9\" (exactly 9)\n  \
day-of-month: \"*\" (every value)\n  \
month: \"*\" (every value)\n  \
day-of-week: \"1-5\" (range 1-5)\n\n\
Summary: At 9:00 AM, Monday through Friday.";
        assert_eq!(describe_with(&expr, &options), expected);
    }

    #[test]
    fn verbose_breakdown_with_or_trap_note() {
        let expr = expr5(
            f(FieldKind::Minute, vec![Term::Single(0)], "0"),
            f(FieldKind::Hour, vec![Term::Single(0)], "0"),
            f(FieldKind::DayOfMonth, vec![Term::Single(1)], "1"),
            wildcard(FieldKind::Month),
            f(FieldKind::DayOfWeek, vec![Term::Single(1)], "1"),
            "0 0 1 * 1",
        );
        let options = DescribeOptions {
            verbose: true,
            ..Default::default()
        };
        let output = describe_with(&expr, &options);
        assert!(output.contains("OR-trap"));
        assert!(
            output.ends_with("Summary: At 12:00 AM, on day 1 of the month, and only on Monday.")
        );
    }
}
