//! Timezone- and DST-aware next-run computation.
//!
//! Fills in during phase 1 (v0.2, timing) and phase 2 (v0.4, DST warnings).
//! The engine resolves each candidate wall-clock time against the target zone
//! via chrono-tz's three-way local resolution — `None` (spring-forward gap,
//! a *skipped* run) / `Single` / `Ambiguous` (fall-back repeat, a *double* run)
//! — rather than computing offsets by hand, and honors the day-field
//! [OR-trap](crate::ast::CronExpr::day_fields_are_or).
