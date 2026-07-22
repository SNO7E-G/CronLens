//! End-to-end translation golden tests: `parse` → `describe`.
//!
//! Unlike the unit tests inside each module (which hand-build ASTs or check one
//! stage), these run the full pipeline, so they catch any mismatch between the
//! shape the parser produces and the shape the describer expects.

use cronlens_core::error::{CronError, FieldError};
use cronlens_core::{describe, parse};

/// `(expression, expected English)` — the golden corpus.
const GOLDEN: &[(&str, &str)] = &[
    ("* * * * *", "Every minute."),
    ("0 9 * * 1-5", "At 9:00 AM, Monday through Friday."),
    ("*/15 * * * *", "Every 15 minutes."),
    ("0 0 * * *", "At 12:00 AM, every day."),
    ("0 12 * * *", "At 12:00 PM, every day."),
    ("30 2 * * *", "At 2:30 AM, every day."),
    ("0 0 1 * *", "At 12:00 AM, on day 1 of the month."),
    ("0 9 * * 1", "At 9:00 AM, only on Monday."),
    ("0 * * * *", "Every hour, on the hour."),
    ("5 * * * *", "At 5 minutes past every hour."),
    ("0 22 * * 1-5", "At 10:00 PM, Monday through Friday."),
    (
        "0 0 1 1 *",
        "At 12:00 AM, on day 1 of the month, only in January.",
    ),
    // Named values resolve identically to their numbers.
    ("0 9 * * MON", "At 9:00 AM, only on Monday."),
    ("0 9 * * mon-fri", "At 9:00 AM, Monday through Friday."),
    ("0 0 * JAN *", "At 12:00 AM, only in January."),
    // Nicknames.
    ("@daily", "At 12:00 AM, every day."),
    ("@hourly", "Every hour, on the hour."),
    ("@weekly", "At 12:00 AM, only on Sunday."),
    ("@monthly", "At 12:00 AM, on day 1 of the month."),
    (
        "@yearly",
        "At 12:00 AM, on day 1 of the month, only in January.",
    ),
    // Sunday is both 0 and 7.
    ("0 9 * * 0", "At 9:00 AM, only on Sunday."),
    ("0 9 * * 7", "At 9:00 AM, only on Sunday."),
    // The OR-trap: both day fields restricted.
    (
        "0 0 1 * 1",
        "At 12:00 AM, on day 1 of the month, and only on Monday.",
    ),
    // Extra whitespace is normalized.
    ("  0   9  *  *  1-5 ", "At 9:00 AM, Monday through Friday."),
];

#[test]
fn golden_translations() {
    for (expr, expected) in GOLDEN {
        let parsed = parse(expr).unwrap_or_else(|e| panic!("`{expr}` failed to parse: {e}"));
        assert_eq!(describe(&parsed), *expected, "for expression `{expr}`");
    }
}

#[test]
fn empty_is_rejected() {
    assert!(matches!(parse("   "), Err(CronError::Empty)));
}

#[test]
fn out_of_range_minute() {
    let err = parse("60 * * * *").unwrap_err();
    assert!(
        matches!(
            err,
            CronError::Field {
                kind: FieldError::OutOfRange { value: 60, .. },
                ..
            }
        ),
        "expected an out-of-range minute error, got {err:?}"
    );
}

#[test]
fn reversed_range_is_rejected() {
    let err = parse("5-2 * * * *").unwrap_err();
    assert!(
        matches!(
            err,
            CronError::Field {
                kind: FieldError::ReversedRange { .. },
                ..
            }
        ),
        "expected a reversed-range error, got {err:?}"
    );
}

#[test]
fn wrong_field_count_is_rejected() {
    assert!(matches!(
        parse("* * *"),
        Err(CronError::FieldCount { found: 3, .. })
    ));
}

#[test]
fn unknown_nickname_is_rejected() {
    assert!(matches!(
        parse("@bogus"),
        Err(CronError::UnknownNickname(_))
    ));
}

#[test]
fn reboot_is_unschedulable() {
    assert!(matches!(
        parse("@reboot"),
        Err(CronError::Unschedulable { .. })
    ));
}
