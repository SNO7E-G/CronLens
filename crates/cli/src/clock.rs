//! Resolving the target timezone and the anchor instant from CLI options.
//!
//! The core engine is clock- and I/O-free; reading the system clock and the
//! host timezone happens here, in the shell.

use cronlens_core::chrono::offset::MappedLocalTime;
use cronlens_core::chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use cronlens_core::Tz;

/// Decide the target timezone: `--utc` wins, then an explicit `--tz`, then the
/// host's IANA zone, falling back to UTC if it can't be determined or parsed.
pub fn resolve_tz(explicit: Option<&str>, utc: bool) -> Result<Tz, String> {
    if utc {
        return Ok(Tz::UTC);
    }
    if let Some(name) = explicit {
        return name.parse::<Tz>().map_err(|_| {
            format!("unknown timezone: '{name}' (expected an IANA name like America/New_York)")
        });
    }
    match iana_time_zone::get_timezone() {
        Ok(name) => Ok(name.parse::<Tz>().unwrap_or(Tz::UTC)),
        Err(_) => Ok(Tz::UTC),
    }
}

/// Resolve the anchor the run list is computed from: `--from <datetime>` parsed
/// in the target zone, or the current instant.
pub fn resolve_anchor(from: Option<&str>, tz: Tz) -> Result<DateTime<Tz>, String> {
    match from {
        Some(s) => parse_anchor(s, tz),
        None => Ok(Utc::now().with_timezone(&tz)),
    }
}

/// Parse a `--from` value as local wall-clock time in `tz`. Accepts a date, a
/// date and time, and `T`- or space-separated forms.
fn parse_anchor(input: &str, tz: Tz) -> Result<DateTime<Tz>, String> {
    let s = input.trim();

    let naive = parse_naive(s).ok_or_else(|| {
        format!("could not parse --from '{input}' (try 2026-07-22, '2026-07-22 14:30', or 2026-07-22T14:30:00)")
    })?;

    match tz.from_local_datetime(&naive) {
        MappedLocalTime::Single(dt) => Ok(dt),
        // A time that occurs twice (fall-back): anchor on the earlier instant.
        MappedLocalTime::Ambiguous(first, _) => Ok(first),
        // A time that does not exist (spring-forward gap): nudge forward until
        // it does, so the anchor is still usable.
        MappedLocalTime::None => (0..120)
            .find_map(|mins| {
                let shifted =
                    naive.checked_add_signed(cronlens_core::chrono::Duration::minutes(mins))?;
                match tz.from_local_datetime(&shifted) {
                    MappedLocalTime::Single(dt) => Some(dt),
                    MappedLocalTime::Ambiguous(dt, _) => Some(dt),
                    MappedLocalTime::None => None,
                }
            })
            .ok_or_else(|| {
                format!(
                    "--from '{input}' is a local time that does not exist in {}",
                    tz.name()
                )
            }),
    }
}

fn parse_naive(s: &str) -> Option<NaiveDateTime> {
    const DATETIME_FORMATS: &[&str] = &[
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
    ];
    for fmt in DATETIME_FORMATS {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(dt);
        }
    }
    // Date only -> start of that day.
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return date.and_hms_opt(0, 0, 0);
    }
    None
}
