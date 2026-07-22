//! # cronlens-core
//!
//! The pure, I/O-free engine behind [cronlens](https://github.com/SNO7E-G/CronLens):
//! it parses cron expressions from several dialects into one
//! [normalized AST](ast::CronExpr), turns that AST into plain English, computes
//! timezone- and DST-aware next runs, and flags the failure modes that break
//! cron in production.
//!
//! The crate performs **no I/O** and reads no clock of its own except where you
//! hand it a reference time — which keeps it usable from a CLI, a WASM
//! playground, a pre-commit hook, and a language server alike.
//!
//! ```
//! use cronlens_core::{parse, describe};
//!
//! let expr = parse("0 9 * * 1-5").unwrap();
//! assert_eq!(describe(&expr), "At 9:00 AM, Monday through Friday.");
//! ```

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod analyze;
pub mod ast;
pub mod convert;
pub mod describe;
pub mod error;
pub mod parser;
pub mod schedule;

pub use ast::{CronExpr, Dialect, Field, FieldKind, Term};
pub use describe::{describe, describe_with, DescribeOptions};
pub use error::{CronError, FieldError, Result};
pub use parser::{parse, parse_with, ParseOptions};
pub use schedule::{next_runs, DstStatus, Run, RunIterator};

// Re-export the date/time crates so downstream code (the CLI, tests, other
// consumers) uses exactly the versions the engine was built against.
pub use chrono;
pub use chrono_tz;

/// The crate version, from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
