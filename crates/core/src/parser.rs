//! Dialect-aware parsing: concrete cron text → [`CronExpr`].
//!
//! The parser auto-detects the dialect from field count and token shape unless
//! one is pinned via [`ParseOptions::dialect`]. It resolves nicknames
//! (`@daily`, …), name aliases (`JAN`, `MON`), and the Quartz/AWS extension
//! tokens (`? L W #`), producing precise [`CronError`](crate::CronError)s with the offending
//! token and its span on failure.

use crate::ast::{CronExpr, Dialect};
use crate::error::Result;

/// Knobs controlling how an expression is parsed.
#[derive(Debug, Clone, Default)]
pub struct ParseOptions {
    /// Pin a dialect. `None` auto-detects from field count and tokens.
    pub dialect: Option<Dialect>,
    /// For an ambiguous 6-field expression, treat the leading field as
    /// **seconds** (`true`) rather than a trailing **year** (`false`).
    pub prefer_seconds: bool,
}

impl ParseOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_dialect(mut self, dialect: Dialect) -> Self {
        self.dialect = Some(dialect);
        self
    }
}

/// Parse an expression, auto-detecting the dialect.
pub fn parse(input: &str) -> Result<CronExpr> {
    parse_with(input, &ParseOptions::default())
}

/// Parse an expression under the given [`ParseOptions`].
pub fn parse_with(input: &str, options: &ParseOptions) -> Result<CronExpr> {
    let _ = (input, options);
    todo!("parser is implemented in phase 0 (v0.1)")
}
