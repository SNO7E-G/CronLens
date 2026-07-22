//! Error types shared across the engine.
//!
//! Parse errors carry enough structure — the field, the offending token, and
//! (when available) its [`Span`] — for the CLI to render a precise, underlined
//! diagnostic and for machine-readable `--format json` output. Payloads use the
//! [`Dialect`] and [`FieldKind`] enums rather than pre-rendered strings so that
//! JSON consumers get stable identifiers, not display labels.

use crate::ast::{Dialect, FieldKind, Span};
use thiserror::Error;

/// The result type used throughout the core.
pub type Result<T> = std::result::Result<T, CronError>;

/// Everything that can go wrong while parsing or converting a cron expression.
///
/// `#[non_exhaustive]`: later phases (conversion, linting) add variants, so
/// downstream matches must include a wildcard arm.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum CronError {
    /// The input was empty or contained only whitespace/comments.
    #[error("empty expression")]
    Empty,

    /// The expression had the wrong number of fields for its dialect. `dialect`
    /// is `None` when the layout was being auto-detected.
    #[error(
        "expected {expected} fields for {}, found {found}",
        .dialect.map(Dialect::label).unwrap_or("an auto-detected dialect")
    )]
    FieldCount {
        dialect: Option<Dialect>,
        expected: FieldCount,
        found: usize,
    },

    /// A named dialect string (from `--dialect`) was not recognized.
    #[error("unknown dialect: {0}")]
    UnknownDialect(String),

    /// A nickname such as `@yearly` was not recognized.
    #[error("unknown nickname: {0}")]
    UnknownNickname(String),

    /// A recognized but non-time-based schedule (e.g. `@reboot`) that has no
    /// calculable next run.
    #[error("'{nickname}' has no time-based schedule (it fires on a system event, not the clock)")]
    Unschedulable { nickname: String },

    /// A single field failed to parse or validate.
    #[error("{field} field '{token}': {kind}")]
    Field {
        field: FieldKind,
        /// The exact offending token, e.g. `"60"` or `"1-"`.
        token: String,
        span: Option<Span>,
        kind: FieldError,
    },

    /// Several independent errors from one validation pass (used by the
    /// error-collecting validator rather than the fail-fast parser).
    #[error("{}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))]
    Multiple(Vec<CronError>),
}

impl CronError {
    /// Convenience constructor for a field-level error.
    pub fn field(
        field: FieldKind,
        token: impl Into<String>,
        span: Option<Span>,
        kind: FieldError,
    ) -> Self {
        CronError::Field {
            field,
            token: token.into(),
            span,
            kind,
        }
    }

    /// The span to underline in the source, if this error has one.
    pub fn span(&self) -> Option<Span> {
        match self {
            CronError::Field { span, .. } => *span,
            _ => None,
        }
    }
}

/// The specific reason a field was rejected.
///
/// `#[non_exhaustive]` for the same forward-compatibility reason as
/// [`CronError`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum FieldError {
    #[error("value {value} out of range ({min}–{max})")]
    OutOfRange { value: i64, min: u32, max: u32 },

    #[error("'{0}' is not a valid number, range, or step")]
    Malformed(String),

    #[error("range start {start} is greater than end {end}")]
    ReversedRange { start: u32, end: u32 },

    #[error("step must be a positive number, got '{0}'")]
    InvalidStep(String),

    #[error("unknown name '{0}'")]
    UnknownName(String),

    #[error("'{token}' is not allowed in the {field} field")]
    NotAllowedHere { token: &'static str, field: FieldKind },

    #[error("'{token}' requires the {dialect} dialect")]
    RequiresDialect {
        token: &'static str,
        dialect: Dialect,
    },
}

/// How many fields a dialect expects — a fixed count or one of a set.
///
/// `#[non_exhaustive]` so new shapes can be described without a break.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldCount {
    Exactly(usize),
    OneOf(&'static [usize]),
    Range(usize, usize),
}

impl std::fmt::Display for FieldCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldCount::Exactly(n) => write!(f, "{n}"),
            FieldCount::OneOf(ns) => {
                let parts: Vec<String> = ns.iter().map(|n| n.to_string()).collect();
                write!(f, "one of {}", parts.join(", "))
            }
            FieldCount::Range(a, b) => write!(f, "{a}–{b}"),
        }
    }
}
