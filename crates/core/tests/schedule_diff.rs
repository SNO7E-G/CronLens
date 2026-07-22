//! Differential test: the next-run engine vs. croniter (the reference oracle).
//!
//! Loads `tests/fixtures/next_runs.json` (188 cases generated from croniter
//! across five zones and DST-crossing anchors) and asserts our engine produces
//! the identical UTC fire instants. Correctness is the product, so these must
//! all pass.

use cronlens_core::chrono::{DateTime, Utc};
use cronlens_core::{next_runs, parse, Tz};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Case {
    expr: String,
    tz: String,
    after: String,
    runs: Vec<String>,
}

fn parse_utc(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .unwrap_or_else(|e| panic!("bad fixture timestamp {s:?}: {e}"))
        .with_timezone(&Utc)
}

#[test]
fn matches_croniter_reference() {
    let raw = include_str!("fixtures/next_runs.json");
    let cases: Vec<Case> = serde_json::from_str(raw).expect("fixtures parse");
    assert!(
        cases.len() >= 180,
        "expected the full corpus, got {}",
        cases.len()
    );

    let mut checked = 0usize;
    for case in &cases {
        let expr =
            parse(&case.expr).unwrap_or_else(|e| panic!("`{}` failed to parse: {e}", case.expr));
        let tz: Tz = case
            .tz
            .parse()
            .unwrap_or_else(|_| panic!("unknown zone {}", case.tz));
        let after = parse_utc(&case.after).with_timezone(&tz);

        let runs = next_runs(&expr, after, case.runs.len());
        assert_eq!(
            runs.len(),
            case.runs.len(),
            "expr `{}` tz {} after {}: produced {} runs, want {}",
            case.expr,
            case.tz,
            case.after,
            runs.len(),
            case.runs.len()
        );

        for (i, (run, expected)) in runs.iter().zip(&case.runs).enumerate() {
            let got = run
                .utc()
                .unwrap_or_else(|| panic!("run {i} did not fire for `{}`", case.expr))
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string();
            assert_eq!(
                &got, expected,
                "mismatch at run {i} for expr `{}` tz {} after {}",
                case.expr, case.tz, case.after
            );
            checked += 1;
        }
    }
    eprintln!(
        "verified {checked} fire instants across {} cases",
        cases.len()
    );
}
