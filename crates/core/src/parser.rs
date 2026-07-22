//! Dialect-aware parsing: concrete cron text → [`CronExpr`].
//!
//! The parser auto-detects the dialect from field count and token shape unless
//! one is pinned via [`ParseOptions::dialect`]. It resolves nicknames
//! (`@daily`, …), name aliases (`JAN`, `MON`), and the Quartz/AWS extension
//! tokens (`? L W #`), producing precise [`CronError`]s with the offending
//! token and its span on failure.

use crate::ast::{CronExpr, Dialect, Field, FieldKind, Span, StepBase, Term};
use crate::error::{CronError, FieldCount, FieldError, Result};

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
    if input.trim().is_empty() {
        return Err(CronError::Empty);
    }

    let normalized = normalize_whitespace(input);

    if normalized.starts_with('@') {
        return parse_nickname(&normalized);
    }

    let tokens: Vec<(&str, Option<Span>)> = tokenize_with_spans(input)
        .into_iter()
        .map(|(tok, span)| (tok, Some(span)))
        .collect();

    build_expr(&tokens, options.dialect, normalized)
}

/// Trim the input and collapse any run of whitespace to a single space.
fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Split `input` into whitespace-delimited tokens, pairing each with its byte
/// [`Span`] in `input` itself. Runs of whitespace act as a single delimiter,
/// so this never produces empty tokens.
fn tokenize_with_spans(input: &str) -> Vec<(&str, Span)> {
    let mut tokens = Vec::new();
    let mut start: Option<usize> = None;
    for (i, ch) in input.char_indices() {
        if ch.is_whitespace() {
            if let Some(s) = start.take() {
                tokens.push((&input[s..i], Span::new(s, i)));
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        tokens.push((&input[s..], Span::new(s, input.len())));
    }
    tokens
}

/// Expand a `@nickname` (already trimmed/whitespace-normalized, confirmed to
/// start with `@`) and parse the result as [`Dialect::Posix`].
fn parse_nickname(normalized: &str) -> Result<CronExpr> {
    let lower = normalized.to_ascii_lowercase();
    let expanded: &str = match lower.as_str() {
        "@yearly" | "@annually" => "0 0 1 1 *",
        "@monthly" => "0 0 1 * *",
        "@weekly" => "0 0 * * 0",
        "@daily" | "@midnight" => "0 0 * * *",
        "@hourly" => "0 * * * *",
        "@reboot" => {
            // `@reboot` fires at daemon startup, not on the clock, so it has no
            // schedule to translate. The CLI catches this variant and prints the
            // explanation with a clean (non-error) exit.
            return Err(CronError::Unschedulable {
                nickname: "@reboot".to_string(),
            });
        }
        _ => return Err(CronError::UnknownNickname(normalized.to_string())),
    };

    // Spans are meaningless once we've swapped in the expanded literal: it no
    // longer corresponds byte-for-byte to what the user typed.
    let tokens: Vec<(&str, Option<Span>)> = expanded.split(' ').map(|tok| (tok, None)).collect();
    build_expr(&tokens, Some(Dialect::Posix), expanded.to_string())
}

const POSIX5: [FieldKind; 5] = [
    FieldKind::Minute,
    FieldKind::Hour,
    FieldKind::DayOfMonth,
    FieldKind::Month,
    FieldKind::DayOfWeek,
];
const AWS6: [FieldKind; 6] = [
    FieldKind::Minute,
    FieldKind::Hour,
    FieldKind::DayOfMonth,
    FieldKind::Month,
    FieldKind::DayOfWeek,
    FieldKind::Year,
];
const UNIX_SECONDS6: [FieldKind; 6] = [
    FieldKind::Second,
    FieldKind::Minute,
    FieldKind::Hour,
    FieldKind::DayOfMonth,
    FieldKind::Month,
    FieldKind::DayOfWeek,
];
const QUARTZ6: [FieldKind; 6] = [
    FieldKind::Second,
    FieldKind::Minute,
    FieldKind::Hour,
    FieldKind::DayOfMonth,
    FieldKind::Month,
    FieldKind::DayOfWeek,
];
const QUARTZ7: [FieldKind; 7] = [
    FieldKind::Second,
    FieldKind::Minute,
    FieldKind::Hour,
    FieldKind::DayOfMonth,
    FieldKind::Month,
    FieldKind::DayOfWeek,
    FieldKind::Year,
];

/// Decide the dialect and per-position [`FieldKind`] layout for a field count,
/// honoring a pinned dialect when given.
fn resolve_layout(
    pinned: Option<Dialect>,
    found: usize,
) -> Result<(Dialect, &'static [FieldKind])> {
    if let Some(d) = pinned {
        return match d {
            Dialect::Posix | Dialect::Kubernetes | Dialect::GithubActions => {
                if found == 5 {
                    Ok((d, &POSIX5))
                } else {
                    Err(CronError::FieldCount {
                        dialect: Some(d),
                        expected: FieldCount::Exactly(5),
                        found,
                    })
                }
            }
            Dialect::Aws => {
                if found == 6 {
                    Ok((d, &AWS6))
                } else {
                    Err(CronError::FieldCount {
                        dialect: Some(d),
                        expected: FieldCount::Exactly(6),
                        found,
                    })
                }
            }
            Dialect::UnixSeconds => {
                if found == 6 {
                    Ok((d, &UNIX_SECONDS6))
                } else {
                    Err(CronError::FieldCount {
                        dialect: Some(d),
                        expected: FieldCount::Exactly(6),
                        found,
                    })
                }
            }
            Dialect::Quartz => match found {
                6 => Ok((d, &QUARTZ6)),
                7 => Ok((d, &QUARTZ7)),
                _ => Err(CronError::FieldCount {
                    dialect: Some(d),
                    expected: FieldCount::OneOf(&[6, 7]),
                    found,
                }),
            },
            Dialect::Systemd => Err(CronError::UnknownDialect(
                "systemd OnCalendar parsing is not yet supported".into(),
            )),
        };
    }

    match found {
        5 => Ok((Dialect::Posix, &POSIX5)),
        6 => Ok((Dialect::UnixSeconds, &UNIX_SECONDS6)),
        7 => Ok((Dialect::Quartz, &QUARTZ7)),
        _ => Err(CronError::FieldCount {
            dialect: None,
            expected: FieldCount::OneOf(&[5, 6, 7]),
            found,
        }),
    }
}

/// Parse every token against its layout-assigned [`FieldKind`] and assemble
/// the [`CronExpr`].
fn build_expr(
    tokens: &[(&str, Option<Span>)],
    pinned: Option<Dialect>,
    source: String,
) -> Result<CronExpr> {
    let (dialect, kinds) = resolve_layout(pinned, tokens.len())?;

    // Collect every field error, not just the first, so validation can report
    // all of a line's problems at once (SPEC §4.4).
    let mut fields = Vec::with_capacity(kinds.len());
    let mut errors = Vec::new();
    for (&kind, &(raw, span)) in kinds.iter().zip(tokens.iter()) {
        match parse_field_token(raw, span, kind) {
            Ok(field) => fields.push(field),
            Err(err) => errors.push(err),
        }
    }
    match errors.len() {
        0 => {}
        1 => return Err(errors.pop().expect("len == 1")),
        _ => return Err(CronError::Multiple(errors)),
    }

    let mut iter = fields.into_iter();
    let mut next = || iter.next().expect("layout length matches token count");

    let (second, minute, hour, day_of_month, month, day_of_week, year) = match dialect {
        Dialect::Posix | Dialect::Kubernetes | Dialect::GithubActions => {
            (None, next(), next(), next(), next(), next(), None)
        }
        Dialect::Aws => {
            let minute = next();
            let hour = next();
            let dom = next();
            let month = next();
            let dow = next();
            let year = next();
            (None, minute, hour, dom, month, dow, Some(year))
        }
        Dialect::UnixSeconds => {
            let second = next();
            let minute = next();
            let hour = next();
            let dom = next();
            let month = next();
            let dow = next();
            (Some(second), minute, hour, dom, month, dow, None)
        }
        Dialect::Quartz => {
            let second = next();
            let minute = next();
            let hour = next();
            let dom = next();
            let month = next();
            let dow = next();
            let year = iter.next();
            (Some(second), minute, hour, dom, month, dow, year)
        }
        Dialect::Systemd => unreachable!("systemd is rejected during layout resolution"),
    };

    Ok(CronExpr {
        dialect,
        second,
        minute,
        hour,
        day_of_month,
        month,
        day_of_week,
        year,
        source,
    })
}

/// Parse one whole field (a comma-separated list of terms) into a [`Field`].
fn parse_field_token(raw: &str, span: Option<Span>, kind: FieldKind) -> Result<Field> {
    let mut terms = Vec::new();
    let mut offset = 0usize;

    for part in raw.split(',') {
        let part_span = span.map(|s| Span::new(s.start + offset, s.start + offset + part.len()));
        offset += part.len() + 1; // +1 to skip the comma

        if part.is_empty() {
            return Err(CronError::field(
                kind,
                part.to_string(),
                part_span,
                FieldError::Malformed(part.to_string()),
            ));
        }

        let term = parse_term(part, kind)
            .map_err(|err| CronError::field(kind, part.to_string(), part_span, err))?;
        terms.push(term);
    }

    let mut field = Field::new(kind, terms, raw);
    if let Some(s) = span {
        field = field.with_span(s);
    }
    Ok(field)
}

type FieldResult<T> = std::result::Result<T, FieldError>;

/// Parse a single comma-delimited term (`*`, `?`, a value, a range, or a step).
fn parse_term(token: &str, kind: FieldKind) -> FieldResult<Term> {
    if token == "*" {
        return Ok(Term::All);
    }
    if token == "?" {
        return match kind {
            FieldKind::DayOfMonth | FieldKind::DayOfWeek => Ok(Term::NoSpecific),
            _ => Err(FieldError::NotAllowedHere {
                token: "?",
                field: kind,
            }),
        };
    }

    if let Some((base, step)) = token.split_once('/') {
        let step = parse_step(step)?;
        let base = parse_step_base(base, kind)?;
        return Ok(Term::Step { base, step });
    }

    if let Some((a, b)) = token.split_once('-') {
        let a = resolve_value(a, kind)?;
        let b = resolve_value(b, kind)?;
        return if a > b {
            Err(FieldError::ReversedRange { start: a, end: b })
        } else {
            Ok(Term::Range(a, b))
        };
    }

    resolve_value(token, kind).map(Term::Single)
}

/// Parse the `/n` portion of a step term: a plain positive integer.
fn parse_step(step: &str) -> FieldResult<u32> {
    match step.parse::<u32>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(FieldError::InvalidStep(step.to_string())),
    }
}

/// Parse the base of a step term (`*`, `a`, or `a-b`).
fn parse_step_base(base: &str, kind: FieldKind) -> FieldResult<StepBase> {
    if base == "*" {
        return Ok(StepBase::Whole);
    }
    if let Some((a, b)) = base.split_once('-') {
        let a = resolve_value(a, kind)?;
        let b = resolve_value(b, kind)?;
        return if a > b {
            Err(FieldError::ReversedRange { start: a, end: b })
        } else {
            Ok(StepBase::Range(a, b))
        };
    }
    resolve_value(base, kind).map(StepBase::From)
}

/// Resolve a bare value token (no `*`, `?`, `-`, or `/`) to a bounds-checked
/// number, trying a numeric literal first and then a name alias
/// (`JAN`, `MON`, …). Tokens containing the Quartz/AWS extension markers
/// (`L`, `W`, `#`) that don't otherwise resolve are reported as requiring
/// that dialect rather than as a generic parse failure.
fn resolve_value(token: &str, kind: FieldKind) -> FieldResult<u32> {
    if let Ok(n) = token.parse::<i64>() {
        let (min, max) = kind.bounds();
        return if n < i64::from(min) || n > i64::from(max) {
            Err(FieldError::OutOfRange { value: n, min, max })
        } else {
            Ok(n as u32)
        };
    }

    if let Some(v) = kind.name_to_value(token) {
        return Ok(v);
    }

    if let Some(marker) = special_marker(token) {
        return Err(FieldError::RequiresDialect {
            token: marker,
            dialect: Dialect::Quartz,
        });
    }

    if !token.is_empty() && token.chars().all(|c| c.is_ascii_alphabetic()) {
        Err(FieldError::UnknownName(token.to_string()))
    } else {
        Err(FieldError::Malformed(token.to_string()))
    }
}

/// Whether `token` contains one of the Quartz/AWS extension markers, and
/// which one to report (checked in `#`, `L`, `W` priority when more than one
/// is present, e.g. `LW`).
fn special_marker(token: &str) -> Option<&'static str> {
    if token.contains('#') {
        Some("#")
    } else if token.to_ascii_uppercase().contains('L') {
        Some("L")
    } else if token.to_ascii_uppercase().contains('W') {
        Some("W")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{DOW_NAMES, MONTH_NAMES};

    fn field(expr: &CronExpr, kind: FieldKind) -> Field {
        expr.fields().find(|f| f.kind == kind).cloned().unwrap()
    }

    // --- nicknames ---------------------------------------------------

    #[test]
    fn nickname_yearly_and_annually_expand_the_same() {
        for nick in ["@yearly", "@annually", "@ANNUALLY"] {
            let expr = parse(nick).unwrap();
            assert_eq!(expr.dialect, Dialect::Posix);
            assert_eq!(expr.source, "0 0 1 1 *");
            assert_eq!(field(&expr, FieldKind::Minute).terms, vec![Term::Single(0)]);
            assert_eq!(field(&expr, FieldKind::Hour).terms, vec![Term::Single(0)]);
            assert_eq!(
                field(&expr, FieldKind::DayOfMonth).terms,
                vec![Term::Single(1)]
            );
            assert_eq!(field(&expr, FieldKind::Month).terms, vec![Term::Single(1)]);
            assert_eq!(field(&expr, FieldKind::DayOfWeek).terms, vec![Term::All]);
        }
    }

    #[test]
    fn nickname_monthly() {
        let expr = parse("@monthly").unwrap();
        assert_eq!(expr.source, "0 0 1 * *");
        assert_eq!(
            field(&expr, FieldKind::DayOfMonth).terms,
            vec![Term::Single(1)]
        );
        assert_eq!(field(&expr, FieldKind::Month).terms, vec![Term::All]);
    }

    #[test]
    fn nickname_weekly() {
        let expr = parse("@weekly").unwrap();
        assert_eq!(expr.source, "0 0 * * 0");
        assert_eq!(
            field(&expr, FieldKind::DayOfWeek).terms,
            vec![Term::Single(0)]
        );
    }

    #[test]
    fn nickname_daily_and_midnight() {
        for nick in ["@daily", "@midnight"] {
            let expr = parse(nick).unwrap();
            assert_eq!(expr.source, "0 0 * * *");
        }
    }

    #[test]
    fn nickname_hourly() {
        let expr = parse("@hourly").unwrap();
        assert_eq!(expr.source, "0 * * * *");
        assert_eq!(field(&expr, FieldKind::Minute).terms, vec![Term::Single(0)]);
        assert_eq!(field(&expr, FieldKind::Hour).terms, vec![Term::All]);
    }

    #[test]
    fn nickname_reboot_is_unschedulable() {
        match parse("@reboot") {
            Err(CronError::Unschedulable { nickname }) => {
                assert_eq!(nickname, "@reboot");
            }
            other => panic!("expected Unschedulable, got {other:?}"),
        }
    }

    #[test]
    fn unknown_nickname() {
        match parse("@fortnightly") {
            Err(CronError::UnknownNickname(msg)) => assert_eq!(msg, "@fortnightly"),
            other => panic!("expected UnknownNickname, got {other:?}"),
        }
    }

    #[test]
    fn nickname_expanded_fields_have_no_span() {
        let expr = parse("@daily").unwrap();
        for f in expr.fields() {
            assert_eq!(f.span, None);
        }
    }

    // --- plain parsing -------------------------------------------------

    #[test]
    fn plain_five_field_parse() {
        let expr = parse("0 9 * * 1-5").unwrap();
        assert_eq!(expr.dialect, Dialect::Posix);
        assert!(expr.second.is_none());
        assert!(expr.year.is_none());
        assert_eq!(expr.minute.terms, vec![Term::Single(0)]);
        assert_eq!(expr.hour.terms, vec![Term::Single(9)]);
        assert_eq!(expr.day_of_month.terms, vec![Term::All]);
        assert_eq!(expr.month.terms, vec![Term::All]);
        assert_eq!(expr.day_of_week.terms, vec![Term::Range(1, 5)]);
        assert_eq!(expr.source, "0 9 * * 1-5");
    }

    #[test]
    fn list_of_terms() {
        let expr = parse("0,15,30,45 * * * *").unwrap();
        assert_eq!(
            expr.minute.terms,
            vec![
                Term::Single(0),
                Term::Single(15),
                Term::Single(30),
                Term::Single(45)
            ]
        );
    }

    #[test]
    fn range_with_names() {
        let expr = parse("0 9 * * MON-FRI").unwrap();
        assert_eq!(expr.day_of_week.terms, vec![Term::Range(1, 5)]);
    }

    #[test]
    fn step_over_whole_range() {
        let expr = parse("*/15 * * * *").unwrap();
        assert_eq!(
            expr.minute.terms,
            vec![Term::Step {
                base: StepBase::Whole,
                step: 15
            }]
        );
    }

    #[test]
    fn step_over_explicit_range() {
        let expr = parse("1-30/5 * * * *").unwrap();
        assert_eq!(
            expr.minute.terms,
            vec![Term::Step {
                base: StepBase::Range(1, 30),
                step: 5
            }]
        );
    }

    #[test]
    fn month_name_single() {
        let expr = parse("0 0 1 JAN *").unwrap();
        assert_eq!(expr.month.terms, vec![Term::Single(1)]);
    }

    #[test]
    fn all_month_and_dow_names_resolve() {
        for (i, name) in MONTH_NAMES.iter().enumerate() {
            let expr = parse(&format!("0 0 1 {name} *")).unwrap();
            assert_eq!(expr.month.terms, vec![Term::Single(i as u32 + 1)]);
        }
        for (i, name) in DOW_NAMES.iter().enumerate() {
            let expr = parse(&format!("0 0 * * {name}")).unwrap();
            let expected = if i == 7 { 0 } else { i as u32 };
            // DOW_NAMES[0] and DOW_NAMES[7] are both "SUN"; either resolves to 0.
            assert!(expr.day_of_week.terms == vec![Term::Single(expected)] || name == &"SUN");
        }
    }

    // --- `?` handling ----------------------------------------------------

    #[test]
    fn question_mark_ok_in_day_of_week() {
        let expr = parse("0 0 * * ?").unwrap();
        assert_eq!(expr.day_of_week.terms, vec![Term::NoSpecific]);
    }

    #[test]
    fn question_mark_ok_in_day_of_month() {
        let expr = parse("0 0 ? * MON").unwrap();
        assert_eq!(expr.day_of_month.terms, vec![Term::NoSpecific]);
    }

    #[test]
    fn question_mark_rejected_in_minute() {
        match parse("? 0 * * *") {
            Err(CronError::Field {
                field: FieldKind::Minute,
                kind: FieldError::NotAllowedHere { token: "?", .. },
                ..
            }) => {}
            other => panic!("expected NotAllowedHere in minute, got {other:?}"),
        }
    }

    // --- field-count detection -------------------------------------------

    #[test]
    fn six_fields_auto_detect_as_unix_seconds() {
        let expr = parse("30 0 9 * * MON-FRI").unwrap();
        assert_eq!(expr.dialect, Dialect::UnixSeconds);
        assert_eq!(expr.second.unwrap().terms, vec![Term::Single(30)]);
        assert!(expr.year.is_none());
    }

    #[test]
    fn seven_fields_auto_detect_as_quartz() {
        let expr = parse("0 0 9 * * MON-FRI 2030").unwrap();
        assert_eq!(expr.dialect, Dialect::Quartz);
        assert_eq!(expr.year.unwrap().terms, vec![Term::Single(2030)]);
    }

    #[test]
    fn quartz_pinned_accepts_six_or_seven() {
        let opts = ParseOptions::new().with_dialect(Dialect::Quartz);
        assert!(parse_with("0 0 9 * * MON-FRI", &opts).is_ok());
        assert!(parse_with("0 0 9 * * MON-FRI 2030", &opts).is_ok());
    }

    #[test]
    fn aws_pinned_requires_year() {
        let opts = ParseOptions::new().with_dialect(Dialect::Aws);
        let expr = parse_with("0 9 * * MON-FRI 2030", &opts).unwrap();
        assert_eq!(expr.dialect, Dialect::Aws);
        assert_eq!(expr.year.unwrap().terms, vec![Term::Single(2030)]);
    }

    #[test]
    fn kubernetes_and_github_actions_are_five_field() {
        let k8s = ParseOptions::new().with_dialect(Dialect::Kubernetes);
        let gha = ParseOptions::new().with_dialect(Dialect::GithubActions);
        assert!(parse_with("0 9 * * 1-5", &k8s).is_ok());
        assert!(parse_with("0 9 * * 1-5", &gha).is_ok());
    }

    // --- error cases -------------------------------------------------------

    #[test]
    fn out_of_range_minute() {
        match parse("60 0 * * *") {
            Err(CronError::Field {
                field: FieldKind::Minute,
                kind:
                    FieldError::OutOfRange {
                        value: 60,
                        min: 0,
                        max: 59,
                    },
                ..
            }) => {}
            other => panic!("expected OutOfRange minute, got {other:?}"),
        }
    }

    #[test]
    fn out_of_range_hour() {
        match parse("0 24 * * *") {
            Err(CronError::Field {
                field: FieldKind::Hour,
                kind:
                    FieldError::OutOfRange {
                        value: 24,
                        min: 0,
                        max: 23,
                    },
                ..
            }) => {}
            other => panic!("expected OutOfRange hour, got {other:?}"),
        }
    }

    #[test]
    fn reversed_range() {
        match parse("0 0 * * 5-2") {
            Err(CronError::Field {
                field: FieldKind::DayOfWeek,
                kind: FieldError::ReversedRange { start: 5, end: 2 },
                ..
            }) => {}
            other => panic!("expected ReversedRange, got {other:?}"),
        }
    }

    #[test]
    fn zero_step_is_invalid() {
        match parse("*/0 * * * *") {
            Err(CronError::Field {
                field: FieldKind::Minute,
                kind: FieldError::InvalidStep(s),
                ..
            }) => assert_eq!(s, "0"),
            other => panic!("expected InvalidStep, got {other:?}"),
        }
    }

    #[test]
    fn l_token_rejected_pending_quartz_support() {
        match parse("0 0 L * *") {
            Err(CronError::Field {
                field: FieldKind::DayOfMonth,
                kind:
                    FieldError::RequiresDialect {
                        token: "L",
                        dialect: Dialect::Quartz,
                    },
                ..
            }) => {}
            other => panic!("expected RequiresDialect, got {other:?}"),
        }
    }

    #[test]
    fn hash_token_rejected_pending_quartz_support() {
        match parse("0 0 * * 6#3") {
            Err(CronError::Field {
                field: FieldKind::DayOfWeek,
                kind:
                    FieldError::RequiresDialect {
                        token: "#",
                        dialect: Dialect::Quartz,
                    },
                ..
            }) => {}
            other => panic!("expected RequiresDialect, got {other:?}"),
        }
    }

    #[test]
    fn w_token_rejected_pending_quartz_support() {
        match parse("0 0 3W * *") {
            Err(CronError::Field {
                field: FieldKind::DayOfMonth,
                kind:
                    FieldError::RequiresDialect {
                        token: "W",
                        dialect: Dialect::Quartz,
                    },
                ..
            }) => {}
            other => panic!("expected RequiresDialect, got {other:?}"),
        }
    }

    #[test]
    fn month_name_containing_l_still_resolves() {
        // "JUL" contains an 'L' but must resolve as July (7), not be rejected
        // as a Quartz-only token.
        let expr = parse("0 0 1 JUL *").unwrap();
        assert_eq!(expr.month.terms, vec![Term::Single(7)]);
    }

    #[test]
    fn wrong_field_count_auto_detect() {
        match parse("0 0 * *") {
            Err(CronError::FieldCount {
                dialect: None,
                expected: FieldCount::OneOf(&[5, 6, 7]),
                found: 4,
            }) => {}
            other => panic!("expected FieldCount, got {other:?}"),
        }
    }

    #[test]
    fn wrong_field_count_pinned_dialect() {
        let opts = ParseOptions::new().with_dialect(Dialect::Posix);
        match parse_with("0 0 * * * *", &opts) {
            Err(CronError::FieldCount {
                dialect: Some(Dialect::Posix),
                expected: FieldCount::Exactly(5),
                found: 6,
            }) => {}
            other => panic!("expected FieldCount, got {other:?}"),
        }
    }

    #[test]
    fn unknown_name_error() {
        match parse("0 0 1 FOO *") {
            Err(CronError::Field {
                field: FieldKind::Month,
                kind: FieldError::UnknownName(name),
                ..
            }) => assert_eq!(name, "FOO"),
            other => panic!("expected UnknownName, got {other:?}"),
        }
    }

    #[test]
    fn trailing_comma_is_malformed() {
        match parse("0,1, 5 * * *") {
            Err(CronError::Field {
                field: FieldKind::Minute,
                kind: FieldError::Malformed(tok),
                ..
            }) => assert_eq!(tok, ""),
            other => panic!("expected Malformed, got {other:?}"),
        }
    }

    #[test]
    fn empty_input_is_empty_error() {
        assert_eq!(parse(""), Err(CronError::Empty));
        assert_eq!(parse("   \t  "), Err(CronError::Empty));
    }

    #[test]
    fn dow_value_seven_is_valid_sunday() {
        let expr = parse("0 0 * * 7").unwrap();
        assert_eq!(expr.day_of_week.terms, vec![Term::Single(7)]);
    }

    // --- spans ---------------------------------------------------------

    #[test]
    fn error_span_points_at_offending_token_in_original_input() {
        let input = "0 60 * * *";
        match parse(input) {
            Err(CronError::Field {
                span: Some(span), ..
            }) => {
                assert_eq!(&input[span.start..span.end], "60");
            }
            other => panic!("expected a span-carrying error, got {other:?}"),
        }
    }

    #[test]
    fn field_span_and_raw_are_recorded_on_success() {
        let input = "0 9 * * 1-5";
        let expr = parse(input).unwrap();
        let span = expr.day_of_week.span.expect("span present");
        assert_eq!(&input[span.start..span.end], "1-5");
        assert_eq!(expr.day_of_week.raw, "1-5");
    }

    #[test]
    fn extra_whitespace_is_collapsed_in_source() {
        let expr = parse("  0   9  *  *  1-5  ").unwrap();
        assert_eq!(expr.source, "0 9 * * 1-5");
    }
}
