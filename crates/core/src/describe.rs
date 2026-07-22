//! AST → plain English.
//!
//! The describer is deterministic and i18n-ready: all human strings flow
//! through here so a locale layer can be added later without touching the
//! parser or scheduler. It reads the same [`CronExpr`] the scheduler runs, so a
//! description can never drift from what actually fires — including the day
//! [OR-trap](CronExpr::day_fields_are_or).

use crate::ast::CronExpr;

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
    let _ = (expr, options);
    todo!("describer is implemented in phase 0 (v0.1)")
}
