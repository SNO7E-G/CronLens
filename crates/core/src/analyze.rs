//! Production-safety analysis: the differentiators.
//!
//! Fills in across phases 2–3:
//! - **DST warnings** (v0.4): runs that will be skipped or fired twice.
//! - **Overlap detection** (v0.6): interval vs. job duration, backlog growth.
//! - **Crontab linting** (v0.6): thundering-herd, duplicate schedules,
//!   DST-dangerous minutes, the day-field OR-trap, `@reboot` misuse.
//!
//! Every finding is *actionable* — it names the fix, not just the problem.
