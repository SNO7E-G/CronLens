//! Production-safety analysis: the differentiators.
//!
//! v0.3 ships **DST warnings** — the runs that a daylight-saving transition
//! will silently skip or fire twice. They fall straight out of the scheduler:
//! [`crate::schedule::RunIterator`] already classifies every run, so this is a
//! scan that collects the anomalies over the window the run list spans.
//!
//! Overlap detection and crontab linting arrive in later phases.

use chrono::{DateTime, NaiveDateTime};
use chrono_tz::Tz;

use crate::ast::CronExpr;
use crate::schedule::{RunIterator, RunKind};

/// A DST hazard among upcoming runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DstWarningKind {
    /// The wall-clock time does not exist (spring-forward gap): the run is
    /// skipped.
    Skipped,
    /// The wall-clock time occurs twice (fall-back overlap): the run may fire
    /// twice.
    Doubled,
}

/// A single DST warning: the offending local wall-clock time and what happens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DstWarning {
    pub wall: NaiveDateTime,
    pub kind: DstWarningKind,
}

/// Scan upcoming runs for DST hazards over the window that the next
/// `firing_runs` fires span, in the anchor's timezone.
///
/// Zones without DST (or `--utc`) never produce warnings. `firing_runs` bounds
/// the scan so a schedule that never fires can't loop.
pub fn dst_warnings(expr: &CronExpr, after: DateTime<Tz>, firing_runs: usize) -> Vec<DstWarning> {
    let mut warnings = Vec::new();
    let mut fired = 0usize;

    for run in RunIterator::new(expr, after) {
        match run.kind {
            RunKind::Nonexistent => warnings.push(DstWarning {
                wall: run.wall,
                kind: DstWarningKind::Skipped,
            }),
            RunKind::Ambiguous { .. } => {
                warnings.push(DstWarning {
                    wall: run.wall,
                    kind: DstWarningKind::Doubled,
                });
                fired += 1;
            }
            RunKind::Normal { .. } => fired += 1,
        }
        if fired >= firing_runs {
            break;
        }
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;
    use chrono::TimeZone;
    use chrono_tz::America::New_York;
    use chrono_tz::UTC;

    fn ny(y: i32, m: u32, d: u32) -> DateTime<Tz> {
        New_York.with_ymd_and_hms(y, m, d, 12, 0, 0).unwrap()
    }

    #[test]
    fn spring_forward_is_skipped() {
        // 2027-03-14 02:30 America/New_York does not exist.
        let expr = parse("30 2 * * *").unwrap();
        let w = dst_warnings(&expr, ny(2027, 3, 13), 5);
        assert!(w.iter().any(|w| w.kind == DstWarningKind::Skipped
            && w.wall.to_string().contains("2027-03-14 02:30")));
    }

    #[test]
    fn fall_back_is_doubled() {
        // 2026-11-01 01:30 America/New_York occurs twice.
        let expr = parse("30 1 * * *").unwrap();
        let w = dst_warnings(&expr, ny(2026, 10, 31), 5);
        assert!(w.iter().any(|w| w.kind == DstWarningKind::Doubled
            && w.wall.to_string().contains("2026-11-01 01:30")));
    }

    #[test]
    fn utc_never_warns() {
        let expr = parse("30 2 * * *").unwrap();
        let after = UTC.with_ymd_and_hms(2027, 3, 13, 12, 0, 0).unwrap();
        assert!(dst_warnings(&expr, after, 10).is_empty());
    }
}
