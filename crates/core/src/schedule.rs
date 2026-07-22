//! Timezone- and DST-aware next-run computation.
//!
//! Given a [`CronExpr`] and an anchor in a target IANA timezone, this module
//! answers *when will it fire next?* — correctly across daylight-saving
//! transitions and honoring the day-of-month / day-of-week
//! [OR-trap](crate::ast::CronExpr::day_fields_are_or).
//!
//! # Correctness model
//!
//! Matching is evaluated on **local wall-clock** time. For each matching
//! wall-clock candidate the engine resolves it against the zone via chrono-tz's
//! three-way local resolution and classifies the run:
//!
//! - a *nonexistent* time (spring-forward gap) is [`RunKind::Nonexistent`] — it
//!   never fires;
//! - an *ambiguous* time (fall-back overlap) is [`RunKind::Ambiguous`], carrying
//!   both instants;
//! - otherwise it is [`RunKind::Normal`].
//!
//! Offsets are never computed by hand. [`RunIterator`] yields **every** matching
//! wall time classified this way, so higher layers (DST warnings, `--dst-policy`)
//! are a pure map/filter over the stream; [`next_runs`] is the convenience that
//! returns only the runs that actually fire.
//!
//! # Day semantics (the OR-trap)
//!
//! Following Vixie cron: if **either** day field is star-prefixed the two are
//! AND-ed; if **neither** is, a date matches when day-of-month **or**
//! day-of-week matches. See [`CronExpr::day_fields_are_or`]. Day-of-week folds
//! `7` onto `0` (both Sunday).

use std::iter::FusedIterator;

use chrono::offset::MappedLocalTime;
use chrono::{DateTime, Datelike, Duration, NaiveDateTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;

use crate::ast::{CronExpr, Field, FieldKind, StepBase, Term};

/// How far ahead (in days) the iterator scans before giving up on a
/// never-firing expression (e.g. `0 0 30 2 *`). Generous enough to reach many
/// occurrences of even a Feb-29 schedule; the day-level skip below keeps the
/// scan cheap for sparse and impossible dates alike.
const SCAN_DAYS: i64 = 366 * 500;

/// How a matching wall-clock time resolves against the target zone's DST rules.
///
/// This is a property of the wall time, not a policy outcome: a
/// [`RunKind::Nonexistent`] time is one that does not exist on the clock, which
/// a given daemon may skip *or* shift — that decision belongs to a later policy
/// layer, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RunKind {
    /// The wall time exists exactly once.
    Normal {
        /// The instant in the target zone.
        local: DateTime<Tz>,
        /// The same instant in UTC.
        utc: DateTime<Utc>,
    },
    /// A spring-forward gap: the wall time does not exist, so nothing fires.
    Nonexistent,
    /// A fall-back overlap: the wall time occurs twice.
    Ambiguous {
        /// The earlier of the two instants.
        first: DateTime<Tz>,
        /// The later of the two instants.
        second: DateTime<Tz>,
    },
}

/// A single matched fire time: the calendar wall-clock time plus its resolution
/// against DST.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Run {
    /// The matched local wall-clock time (always a valid calendar time, even
    /// when [`Run::kind`] is [`RunKind::Nonexistent`]).
    pub wall: NaiveDateTime,
    /// How `wall` resolves against the zone.
    pub kind: RunKind,
}

impl Run {
    /// Whether this run actually fires. Only [`RunKind::Nonexistent`] does not.
    pub fn fires(&self) -> bool {
        !matches!(self.kind, RunKind::Nonexistent)
    }

    /// The UTC instant of the (earliest) firing, or `None` if it does not fire.
    pub fn utc(&self) -> Option<DateTime<Utc>> {
        match self.kind {
            RunKind::Normal { utc, .. } => Some(utc),
            RunKind::Ambiguous { first, .. } => Some(first.with_timezone(&Utc)),
            RunKind::Nonexistent => None,
        }
    }

    /// The local instant of the (earliest) firing, or `None` if it does not fire.
    pub fn local(&self) -> Option<DateTime<Tz>> {
        match self.kind {
            RunKind::Normal { local, .. } => Some(local),
            RunKind::Ambiguous { first, .. } => Some(first),
            RunKind::Nonexistent => None,
        }
    }
}

/// Compute the next `count` runs that actually fire, strictly after `after`.
///
/// Nonexistent (spring-forward) runs are filtered out; to see them (for DST
/// warnings) iterate a [`RunIterator`] directly.
pub fn next_runs(expr: &CronExpr, after: DateTime<Tz>, count: usize) -> Vec<Run> {
    RunIterator::new(expr, after)
        .filter(Run::fires)
        .take(count)
        .collect()
}

/// A lazy iterator over an expression's runs, strictly after an anchor.
///
/// Yields a classified [`Run`] for **every** matching wall-clock time, including
/// nonexistent and ambiguous ones. It is fused; `next()` returning `None` means
/// either the year field is exhausted *or* the internal scan horizon was reached
/// without a match (a never-firing expression) — the two are not distinguished.
#[derive(Debug)]
pub struct RunIterator<'a> {
    expr: &'a CronExpr,
    tz: Tz,
    cursor: NaiveDateTime,
    step: Duration,
    limit: NaiveDateTime,
    done: bool,
}

impl<'a> RunIterator<'a> {
    /// Create an iterator yielding runs strictly after `after`. The timezone is
    /// taken from the anchor.
    pub fn new(expr: &'a CronExpr, after: DateTime<Tz>) -> Self {
        let tz = after.timezone();
        let with_seconds = expr.second.is_some();
        let step = if with_seconds {
            Duration::seconds(1)
        } else {
            Duration::minutes(1)
        };

        let anchor = after.naive_local();
        // Truncate to the granularity, then advance one step so the first
        // candidate is strictly after the anchor.
        let truncated = if with_seconds {
            anchor.with_nanosecond(0).unwrap_or(anchor)
        } else {
            anchor
                .with_second(0)
                .and_then(|t| t.with_nanosecond(0))
                .unwrap_or(anchor)
        };
        let cursor = truncated
            .checked_add_signed(step)
            .unwrap_or(NaiveDateTime::MAX);
        let limit = anchor
            .checked_add_signed(Duration::days(SCAN_DAYS))
            .unwrap_or(NaiveDateTime::MAX);

        Self {
            expr,
            tz,
            cursor,
            step,
            limit,
            done: false,
        }
    }
}

impl Iterator for RunIterator<'_> {
    type Item = Run;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        loop {
            if self.cursor > self.limit {
                self.done = true;
                return None;
            }
            let wall = self.cursor;
            // Whole days that can't match the date fields are skipped at once,
            // so sparse schedules (Feb 29) and impossible ones (Feb 30) stay
            // cheap instead of stepping minute by minute.
            if !date_matches(self.expr, wall) {
                match next_midnight(wall) {
                    Some(midnight) => {
                        self.cursor = midnight;
                        continue;
                    }
                    None => {
                        self.done = true;
                        return None;
                    }
                }
            }
            self.cursor = match wall.checked_add_signed(self.step) {
                Some(next) => next,
                None => NaiveDateTime::MAX, // forces termination next round
            };
            if time_matches(self.expr, wall) {
                return Some(Run {
                    wall,
                    kind: classify(self.tz, wall),
                });
            }
        }
    }
}

impl FusedIterator for RunIterator<'_> {}

/// Resolve a matching wall time against the zone into a [`RunKind`].
fn classify(tz: Tz, wall: NaiveDateTime) -> RunKind {
    match tz.from_local_datetime(&wall) {
        MappedLocalTime::Single(local) => RunKind::Normal {
            local,
            utc: local.with_timezone(&Utc),
        },
        MappedLocalTime::Ambiguous(first, second) => RunKind::Ambiguous { first, second },
        MappedLocalTime::None => RunKind::Nonexistent,
    }
}

/// Whether a naive local datetime satisfies the expression's calendar fields.
///
/// Minute, hour, month (and optional second/year) are AND-ed; the two day
/// fields combine via the Vixie OR-trap. Timezone-independent.
pub fn matches(expr: &CronExpr, when: NaiveDateTime) -> bool {
    date_matches(expr, when) && time_matches(expr, when)
}

/// The date half of [`matches`]: month, optional year, and the day fields with
/// the OR-trap. Depends only on the calendar date, so a failing date lets the
/// iterator skip the whole day.
fn date_matches(expr: &CronExpr, when: NaiveDateTime) -> bool {
    if !numeric_matches(&expr.month, when.month()) {
        return false;
    }
    if let Some(year) = &expr.year {
        let y = when.year();
        if y < 0 || !numeric_matches(year, y as u32) {
            return false;
        }
    }

    let dom_ok = day_of_month_matches(&expr.day_of_month, when.day());
    let dow_ok = day_of_week_matches(&expr.day_of_week, when.weekday().num_days_from_sunday());

    if expr.day_fields_are_or() {
        dom_ok || dow_ok
    } else {
        dom_ok && dow_ok
    }
}

/// The time half of [`matches`]: optional second, minute, and hour.
fn time_matches(expr: &CronExpr, when: NaiveDateTime) -> bool {
    if let Some(second) = &expr.second {
        if !numeric_matches(second, when.second()) {
            return false;
        }
    }
    numeric_matches(&expr.minute, when.minute()) && numeric_matches(&expr.hour, when.hour())
}

/// Midnight at the start of the day after `when`, or `None` at the end of the
/// representable range.
fn next_midnight(when: NaiveDateTime) -> Option<NaiveDateTime> {
    when.date().succ_opt()?.and_hms_opt(0, 0, 0)
}

/// Match an ordinary numeric field (second/minute/hour/month/year): no folding,
/// no day extensions.
fn numeric_matches(field: &Field, value: u32) -> bool {
    field
        .terms
        .iter()
        .any(|term| plain_matches(term, value, field.kind, false))
}

/// Match the day-of-month field. Quartz `L`/`W` extensions are not yet
/// evaluated (the parser rejects them before v0.4), so they never match here.
fn day_of_month_matches(field: &Field, day: u32) -> bool {
    field.terms.iter().any(|term| match term {
        Term::LastDayOfMonth
        | Term::LastDayOfMonthOffset(_)
        | Term::NearestWeekday(_)
        | Term::LastWeekday => false, // TODO(v0.4): evaluate against the date
        other => plain_matches(other, day, field.kind, false),
    })
}

/// Match the day-of-week field, folding `7` onto `0`. Quartz `#`/`L` extensions
/// are not yet evaluated.
fn day_of_week_matches(field: &Field, weekday: u32) -> bool {
    field.terms.iter().any(|term| match term {
        Term::NthWeekday { .. } | Term::LastWeekdayOfMonth(_) => false, // TODO(v0.4)
        other => plain_matches(other, weekday, field.kind, true),
    })
}

/// Match one ordinary term (`*`, `?`, single, range, step) against a value,
/// optionally folding `7` onto `0` for day-of-week. Day-extension terms return
/// `false` (their callers handle them).
fn plain_matches(term: &Term, value: u32, kind: FieldKind, fold7: bool) -> bool {
    match term {
        Term::All | Term::NoSpecific => true,
        Term::Single(v) => eq_fold(*v, value, fold7),
        Term::Range(a, b) => (*a..=*b).any(|k| eq_fold(k, value, fold7)),
        Term::Step { base, step } => {
            let (start, end) = step_bounds(base, kind);
            (start..=end)
                .step_by(*step as usize)
                .any(|k| eq_fold(k, value, fold7))
        }
        _ => false,
    }
}

/// Equality with optional day-of-week folding (`7 ≡ 0`).
fn eq_fold(candidate: u32, value: u32, fold7: bool) -> bool {
    if fold7 {
        candidate % 7 == value
    } else {
        candidate == value
    }
}

/// The inclusive `(start, end)` a step iterates over, given its base and the
/// field's bounds.
fn step_bounds(base: &StepBase, kind: FieldKind) -> (u32, u32) {
    let (min, max) = kind.bounds();
    match base {
        StepBase::Whole => (min, max),
        StepBase::From(m) => (*m, max),
        StepBase::Range(a, b) => (*a, *b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;
    use chrono::TimeZone;
    use chrono_tz::America::New_York;
    use chrono_tz::UTC;

    fn at(tz: Tz, y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Tz> {
        tz.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    fn naive(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, min, 0)
            .unwrap()
    }

    #[test]
    fn matches_basic_and_dow_fold() {
        let expr = parse("0 9 * * 1-5").unwrap();
        assert!(matches(&expr, naive(2026, 7, 22, 9, 0))); // Wednesday
        assert!(!matches(&expr, naive(2026, 7, 25, 9, 0))); // Saturday
        assert!(!matches(&expr, naive(2026, 7, 22, 10, 0))); // wrong hour

        // Sunday reachable as both 0 and 7.
        let sun0 = parse("0 9 * * 0").unwrap();
        let sun7 = parse("0 9 * * 7").unwrap();
        assert!(matches(&sun0, naive(2026, 7, 26, 9, 0)));
        assert!(matches(&sun7, naive(2026, 7, 26, 9, 0)));
    }

    #[test]
    fn matches_or_trap() {
        // Both day fields restricted -> fires on the 1st OR any Monday.
        let expr = parse("0 0 1 * 1").unwrap();
        assert!(expr.day_fields_are_or());
        assert!(matches(&expr, naive(2026, 7, 1, 0, 0))); // the 1st (a Wednesday)
        assert!(matches(&expr, naive(2026, 7, 6, 0, 0))); // a Monday
        assert!(!matches(&expr, naive(2026, 7, 7, 0, 0))); // neither
    }

    #[test]
    fn next_runs_are_strictly_after_and_ordered() {
        let expr = parse("0 9 * * 1-5").unwrap();
        let runs = next_runs(&expr, at(UTC, 2026, 7, 22, 9, 0), 3);
        let utcs: Vec<_> = runs.iter().map(|r| r.utc().unwrap().to_rfc3339()).collect();
        // 07-22 09:00 is excluded (strictly after); next are Thu, Fri, Mon.
        assert_eq!(runs[0].wall, naive(2026, 7, 23, 9, 0));
        assert_eq!(runs[1].wall, naive(2026, 7, 24, 9, 0));
        assert_eq!(runs[2].wall, naive(2026, 7, 27, 9, 0));
        assert!(utcs.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn spring_forward_run_is_nonexistent_and_does_not_fire() {
        // 2027-03-14 02:30 America/New_York does not exist (spring forward).
        let expr = parse("30 2 * * *").unwrap();
        let after = at(New_York, 2027, 3, 13, 12, 0);
        // Scan the raw stream and find the 03-14 02:30 wall time.
        let run = RunIterator::new(&expr, after)
            .find(|r| r.wall == naive(2027, 3, 14, 2, 30))
            .expect("the wall time is matched even though it doesn't exist");
        assert_eq!(run.kind, RunKind::Nonexistent);
        assert!(!run.fires());
        // next_runs skips it: no firing run lands on that wall time.
        let fired = next_runs(&expr, after, 5);
        assert!(fired.iter().all(|r| r.wall != naive(2027, 3, 14, 2, 30)));
    }

    #[test]
    fn fall_back_run_is_ambiguous() {
        // 2026-11-01 01:30 America/New_York occurs twice (fall back).
        let expr = parse("30 1 * * *").unwrap();
        let after = at(New_York, 2026, 10, 31, 12, 0);
        let run = RunIterator::new(&expr, after)
            .find(|r| r.wall == naive(2026, 11, 1, 1, 30))
            .expect("wall time present");
        match run.kind {
            RunKind::Ambiguous { first, second } => assert!(first < second),
            other => panic!("expected Ambiguous, got {other:?}"),
        }
        assert!(run.fires());
    }

    #[test]
    fn never_firing_expression_terminates() {
        // Feb 30 never exists; the iterator must give up, not hang.
        let expr = parse("0 0 30 2 *").unwrap();
        let runs = next_runs(&expr, at(UTC, 2026, 1, 1, 0, 0), 3);
        assert!(runs.is_empty());
    }
}
