# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-07-22

Timing: **when will it actually run?**

### Added

- **Next-run engine.** Timezone- and DST-aware next-run computation that honors
  the day-of-month / day-of-week OR-trap and classifies each run against
  daylight-saving (normal / nonexistent spring-forward / ambiguous fall-back).
  Verified against `croniter` across a 188-case differential corpus spanning
  five zones (including a no-DST `+05:30` zone and 30-minute-DST Lord Howe) and
  DST-crossing anchors.
- The default command now lists upcoming runs under the translation, with:
  - `--next N` (default 5, `0` to show only the translation),
  - `--tz <IANA>` and `--utc` to choose the zone (default: the system zone),
  - `--from <datetime>` to anchor the list at a specific local time.
  Ambiguous fall-back runs are flagged in the list.
- **Multi-error validation.** A line with several bad fields now reports them
  all at once (e.g. `60 25 * * *` flags both the minute and the hour), each with
  its own underline, still exiting `3`.
- `chrono` / `chrono-tz` (and the `Tz` type) are re-exported from
  `cronlens-core` so downstream code can't version-skew.

[0.2.0]: https://github.com/SNO7E-G/CronLens/releases/tag/v0.2.0

## [0.1.0] - 2026-07-22

The first release: **translate a cron expression into plain English.**

### Added

- **Translation.** `cronlens "<expr>"` parses a cron expression and prints a
  plain-English description, e.g. `0 9 * * 1-5` → *"At 9:00 AM, Monday through
  Friday."* Covers `* , - / ?`, steps, ranges, lists, and named months/days
  (`JAN`, `MON`).
- **Nicknames:** `@yearly`/`@annually`, `@monthly`, `@weekly`, `@daily`/
  `@midnight`, `@hourly`, and `@reboot` (reported as unschedulable).
- **Dialect detection:** 5-field POSIX, 6-field (leading seconds), and 7-field
  Quartz (with year) are auto-detected; `--dialect` pins one explicitly.
- **`--verbose`** prints a field-by-field breakdown; **`--use-24h`** switches to
  a 24-hour clock; color output respects `NO_COLOR` and non-TTY pipes.
- **Precise errors:** invalid fields are reported with the offending token
  underlined and a CI-friendly exit code (`3` for invalid input).
- The normalized, dialect-independent cron **AST** shared across the engine,
  with first-class handling of the day-of-month / day-of-week **OR-trap** and
  Vixie star-prefix semantics.
- Structured, span-carrying parse errors (`#[non_exhaustive]`, enum-typed
  payloads) ready for machine-readable output in a later release.
- Workspace scaffolding: a pure, I/O-free `cronlens-core` crate and a thin
  `cronlens-cli` binary; end-to-end golden translation tests.
- Continuous integration (rustfmt, clippy with warnings denied, a
  Linux/macOS/Windows test matrix, and a documentation build), a tag-driven
  cross-platform release workflow, dual MIT / Apache-2.0 licensing, a
  contributing guide, and a code of conduct.

[0.1.0]: https://github.com/SNO7E-G/CronLens/releases/tag/v0.1.0
