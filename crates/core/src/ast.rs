//! The dialect-independent abstract syntax tree for a cron schedule.
//!
//! Every part of the engine — [`crate::parser`], [`crate::describe`],
//! [`crate::schedule`], [`crate::analyze`] and [`crate::convert`] — operates on
//! this single representation. Parsers translate a concrete dialect (POSIX,
//! Quartz, AWS, …) into a [`CronExpr`]; everything downstream stays
//! dialect-agnostic.
//!
//! # The OR-trap
//!
//! The most consequential piece of cron semantics lives here: when **both** the
//! day-of-month and day-of-week fields are *restricted* (neither `*` nor `?`),
//! a date matches when it satisfies **either** field, not both. See
//! [`CronExpr::day_fields_are_or`]. Getting this into the AST — rather than
//! bolting it on as a lint — is what keeps the scheduler correct.

use std::fmt;

/// A byte range into the original expression, used to underline the offending
/// token in error messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

/// The concrete cron flavor an expression was written in.
///
/// Dialects differ in field count, the meaning of the sixth/seventh field,
/// day-of-week numbering, and which special tokens (`? L W #`) are legal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialect {
    /// Classic Vixie/POSIX 5-field crontab. The default.
    Posix,
    /// Quartz scheduler: 6 or 7 fields (leading seconds, optional trailing
    /// year), and the `? L W #` extensions.
    Quartz,
    /// AWS EventBridge / CloudWatch: 6 fields with a required `?` in exactly one
    /// of the day fields, and a trailing year.
    Aws,
    /// Kubernetes `CronJob`: POSIX 5-field, but scheduled against
    /// `.spec.timeZone` rather than the node clock.
    Kubernetes,
    /// GitHub Actions `on.schedule`: POSIX 5-field, **UTC only**.
    GithubActions,
    /// A 6-field expression with a leading **seconds** field (no year).
    UnixSeconds,
}

impl Dialect {
    /// Whether this dialect supports the Quartz `? L W #` extension tokens.
    pub const fn supports_special_tokens(self) -> bool {
        matches!(self, Dialect::Quartz | Dialect::Aws)
    }

    /// Human-facing name.
    pub const fn label(self) -> &'static str {
        match self {
            Dialect::Posix => "POSIX/Vixie",
            Dialect::Quartz => "Quartz",
            Dialect::Aws => "AWS EventBridge",
            Dialect::Kubernetes => "Kubernetes CronJob",
            Dialect::GithubActions => "GitHub Actions",
            Dialect::UnixSeconds => "Unix (with seconds)",
        }
    }
}

impl fmt::Display for Dialect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Which position a [`Field`] occupies. Carries the inclusive value bounds and
/// the naming scheme for that position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldKind {
    Second,
    Minute,
    Hour,
    DayOfMonth,
    Month,
    DayOfWeek,
    Year,
}

impl FieldKind {
    /// The inclusive `(min, max)` range of raw numeric values accepted in this
    /// field. Day-of-week uses `0..=7` so that both `0` and `7` mean Sunday.
    pub const fn bounds(self) -> (u32, u32) {
        match self {
            FieldKind::Second => (0, 59),
            FieldKind::Minute => (0, 59),
            FieldKind::Hour => (0, 23),
            FieldKind::DayOfMonth => (1, 31),
            FieldKind::Month => (1, 12),
            FieldKind::DayOfWeek => (0, 7),
            FieldKind::Year => (1970, 2099),
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            FieldKind::Second => "second",
            FieldKind::Minute => "minute",
            FieldKind::Hour => "hour",
            FieldKind::DayOfMonth => "day-of-month",
            FieldKind::Month => "month",
            FieldKind::DayOfWeek => "day-of-week",
            FieldKind::Year => "year",
        }
    }

    /// Look up a three-letter alias (`JAN`, `MON`, case-insensitive) for this
    /// field, returning its numeric value. Only months and days-of-week have
    /// names.
    pub fn name_to_value(self, name: &str) -> Option<u32> {
        let up = name.to_ascii_uppercase();
        match self {
            FieldKind::Month => MONTH_NAMES
                .iter()
                .position(|n| *n == up)
                .map(|i| i as u32 + 1),
            FieldKind::DayOfWeek => DOW_NAMES.iter().position(|n| *n == up).map(|i| i as u32),
            _ => None,
        }
    }
}

impl fmt::Display for FieldKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Month abbreviations, indexed so that `MONTH_NAMES[0] == "JAN"` → 1.
pub const MONTH_NAMES: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// Full month names for descriptions, `MONTH_FULL[0] == "January"`.
pub const MONTH_FULL: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Day-of-week abbreviations, indexed so that `DOW_NAMES[0] == "SUN"` → 0.
pub const DOW_NAMES: [&str; 8] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];

/// Full weekday names, `DOW_FULL[0] == "Sunday"`. Index 7 also maps to Sunday.
pub const DOW_FULL: [&str; 8] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// The base a `/step` term iterates over.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StepBase {
    /// `*/n` — the whole field range.
    Whole,
    /// `m/n` — from `m` up to the field maximum.
    From(u32),
    /// `a-b/n` — from `a` to `b`.
    Range(u32, u32),
}

/// A single term within a field. A field's matching set is the union of its
/// terms.
///
/// Ordinary numeric terms ([`Term::Single`], [`Term::Range`], [`Term::Step`],
/// [`Term::All`]) are legal in every field. The remaining variants are the
/// Quartz/AWS extensions and are only legal in the day fields.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Term {
    /// `*` — every value in the field's range.
    All,
    /// A single value, already resolved from any name alias.
    Single(u32),
    /// An inclusive range `start-end`.
    Range(u32, u32),
    /// A stepped term such as `*/15`, `10/5`, or `1-30/5`.
    Step { base: StepBase, step: u32 },
    /// `?` — "no specific value" (Quartz/AWS). Semantically equivalent to `*`
    /// for matching, but signals that the field is intentionally unrestricted.
    NoSpecific,

    // --- day-of-month extensions ---
    /// `L` — the last day of the month.
    LastDayOfMonth,
    /// `L-n` — the n-th day before the last day of the month.
    LastDayOfMonthOffset(u32),
    /// `nW` — the weekday (Mon–Fri) nearest to day `n`.
    NearestWeekday(u32),
    /// `LW` — the last weekday (Mon–Fri) of the month.
    LastWeekday,

    // --- day-of-week extensions ---
    /// `d#n` — the n-th weekday `d` of the month (e.g. `6#3` = 3rd Friday).
    NthWeekday { weekday: u32, nth: u32 },
    /// `dL` — the last weekday `d` of the month (e.g. `5L` = last Thursday).
    LastWeekdayOfMonth(u32),
}

impl Term {
    /// Whether this term is one of the Quartz/AWS day-field extensions.
    pub const fn is_special(&self) -> bool {
        matches!(
            self,
            Term::LastDayOfMonth
                | Term::LastDayOfMonthOffset(_)
                | Term::NearestWeekday(_)
                | Term::LastWeekday
                | Term::NthWeekday { .. }
                | Term::LastWeekdayOfMonth(_)
        )
    }

    /// Whether this term leaves the field effectively unrestricted (`*` or `?`).
    pub const fn is_wildcard(&self) -> bool {
        matches!(self, Term::All | Term::NoSpecific)
    }
}

/// One parsed field of a cron expression: the union of its [`Term`]s plus the
/// original text for diagnostics and lossless round-tripping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub kind: FieldKind,
    pub terms: Vec<Term>,
    /// The exact source token, e.g. `"1-5"`. Kept for error messages and
    /// format conversion.
    pub raw: String,
    /// Where `raw` sits in the original expression, when known.
    pub span: Option<Span>,
}

impl Field {
    pub fn new(kind: FieldKind, terms: Vec<Term>, raw: impl Into<String>) -> Self {
        Self {
            kind,
            terms,
            raw: raw.into(),
            span: None,
        }
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// A field is *restricted* when it constrains the schedule — i.e. it is not
    /// a bare `*` or `?`. This is the predicate the day-field OR-trap turns on.
    pub fn is_restricted(&self) -> bool {
        !self.terms.iter().all(Term::is_wildcard)
    }

    /// Whether any term uses a Quartz/AWS extension token.
    pub fn has_special(&self) -> bool {
        self.terms.iter().any(Term::is_special)
    }
}

/// A fully parsed cron expression, normalized across dialects.
///
/// Optional fields ([`CronExpr::second`], [`CronExpr::year`]) are `None` for
/// plain 5-field POSIX expressions and `Some` for the wider dialects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronExpr {
    pub dialect: Dialect,
    pub second: Option<Field>,
    pub minute: Field,
    pub hour: Field,
    pub day_of_month: Field,
    pub month: Field,
    pub day_of_week: Field,
    pub year: Option<Field>,
    /// The original, whitespace-normalized source expression.
    pub source: String,
}

impl CronExpr {
    /// The classic cron gotcha, encoded once: when **both** day fields are
    /// restricted, a date fires if it matches day-of-month **OR** day-of-week.
    /// When only one is restricted, that field alone decides (the other is
    /// `*`/`?`). This is what [`crate::schedule`] must honor and what the
    /// OR-trap lint reports.
    pub fn day_fields_are_or(&self) -> bool {
        self.day_of_month.is_restricted() && self.day_of_week.is_restricted()
    }

    /// Iterate the present fields in canonical order (seconds → year), skipping
    /// the optional ones that are absent.
    pub fn fields(&self) -> impl Iterator<Item = &Field> {
        [
            self.second.as_ref(),
            Some(&self.minute),
            Some(&self.hour),
            Some(&self.day_of_month),
            Some(&self.month),
            Some(&self.day_of_week),
            self.year.as_ref(),
        ]
        .into_iter()
        .flatten()
    }
}
