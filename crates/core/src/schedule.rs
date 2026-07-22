//! Timezone- and DST-aware next-run computation.
//!
//! Given a [`CronExpr`] and a target IANA timezone, this module answers *when
//! will it fire next?* — correctly across daylight-saving transitions and
//! honoring the day-of-month / day-of-week
//! [OR-trap](crate::ast::CronExpr::day_fields_are_or).
//!
//! # Correctness model
//!
//! Matching is evaluated on **local wall-clock** time. For each candidate the
//! engine resolves the naive local time against the zone via chrono-tz's
//! three-way local resolution:
//!
//! - a *nonexistent* time (spring-forward gap) is a [`DstStatus::Skipped`] run,
//! - an *ambiguous* time (fall-back overlap) is a [`DstStatus::Ambiguous`] run
//!   that fires twice,
//! - otherwise the run is [`DstStatus::Normal`].
//!
//! Offsets are never computed by hand.
//!
//! # Day semantics (the OR-trap)
//!
//! Following Vixie cron: if **either** day field is star-prefixed the two are
//! AND-ed (a star field matches everything, so the other field decides); if
//! **neither** is star-prefixed a date matches when day-of-month **or**
//! day-of-week matches. See [`CronExpr::day_fields_are_or`]. Day-of-week folds
//! `7` onto `0` (both Sunday).

use chrono::{DateTime, NaiveDateTime, Utc};
use chrono_tz::Tz;

use crate::ast::CronExpr;

/// How a run's local wall-clock time resolved against daylight-saving time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DstStatus {
    /// The local time exists exactly once — the ordinary case.
    Normal,
    /// The local time falls in a spring-forward gap and does not exist. Whether
    /// the daemon skips or shifts such a run depends on its DST policy; the run
    /// is surfaced so callers can warn about it.
    Skipped,
    /// The local time falls in a fall-back overlap and occurs twice.
    Ambiguous,
}

/// A single computed run: the same instant expressed in the target zone and in
/// UTC, plus its DST classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    /// The fire time in the target timezone.
    pub local: DateTime<Tz>,
    /// The same instant in UTC.
    pub utc: DateTime<Utc>,
    /// How this run's wall-clock time resolved against DST.
    pub dst: DstStatus,
}

/// Whether a naive local datetime satisfies the expression's calendar fields.
///
/// Evaluates minute, hour, month (and optional second/year) with AND, and the
/// two day fields with the Vixie OR-trap rule. Timezone-independent: this is
/// pure calendar matching, the building block [`next_runs`] steps over.
pub fn matches(expr: &CronExpr, when: NaiveDateTime) -> bool {
    let _ = (expr, when);
    todo!("matcher lands in phase 1 (v0.2)")
}

/// Compute the next `count` runs strictly after `after`, in timezone `tz`.
///
/// Results are in chronological order. Runs whose local time is skipped or
/// ambiguous under DST are included and tagged via [`Run::dst`]; it is up to
/// the caller (and a later DST-policy layer) to decide how to present them.
pub fn next_runs(expr: &CronExpr, tz: Tz, after: DateTime<Tz>, count: usize) -> Vec<Run> {
    RunIterator::new(expr, tz, after).take(count).collect()
}

/// A lazy iterator over the runs of an expression, strictly after an anchor.
// Fields are read once run iteration is implemented in v0.2.
#[allow(dead_code)]
#[derive(Debug)]
pub struct RunIterator<'a> {
    expr: &'a CronExpr,
    tz: Tz,
    cursor: DateTime<Tz>,
}

impl<'a> RunIterator<'a> {
    /// Create an iterator yielding runs strictly after `after`.
    pub fn new(expr: &'a CronExpr, tz: Tz, after: DateTime<Tz>) -> Self {
        Self {
            expr,
            tz,
            cursor: after,
        }
    }
}

impl Iterator for RunIterator<'_> {
    type Item = Run;

    fn next(&mut self) -> Option<Self::Item> {
        todo!("run iteration lands in phase 1 (v0.2)")
    }
}
