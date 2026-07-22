//! Error types shared across the engine.
//!
//! Parse errors carry enough structure — the field, the offending token, and
//! (when available) its [`Span`] — for the CLI to render a precise, underlined
//! diagnostic and for machine-readable `--format json` output.

use crate::ast::{FieldKind, Span};
use thiserror::Error;

/// The result type used throughout the core.
pub type Result<T> = std::result::Result<T, CronError>;

/// Everything that can go wrong while parsing or converting a cron expression.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CronError {
    /// The input was empty or contained only whitespace/comments.
    #[error("empty expression")]
    Empty,

    /// The expression had the wrong number of fields for its dialect.
    #[error("expected {expected} fields for {dialect}, found {found}")]
    FieldCount {
        dialect: &'static str,
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
    /// calculable next run. The payload is a ready-to-print human explanation.
    #[error("{0}")]
    Unschedulable(String),

    /// A single field failed to parse or validate.
    #[error("{field} field {}: {kind}", .token.as_str())]
    Field {
        field: FieldKind,
        /// The exact offending token, e.g. `"60"` or `"1-"`.
        token: String,
        span: Option<Span>,
        kind: FieldError,
    },
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
#[derive(Debug, Clone, PartialEq, Eq, Error)]
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
    NotAllowedHere {
        token: &'static str,
        field: &'static str,
    },

    #[error("'{token}' requires the {dialect} dialect")]
    RequiresDialect {
        token: &'static str,
        dialect: &'static str,
    },
}

/// How many fields a dialect expects — a fixed count or one of a set.
#[derive(Debug, Clone, PartialEq, Eq)]
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
