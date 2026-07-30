# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-07-31

Reviewing schedule changes, and the gotcha nobody knows.

### Added

- **`--diff <expr>`** compares the expression against another one for PR review:
  the before/after plain English plus how often each fires per year, e.g.
  *"Before fires 261 times/year; after fires 365 (+104)."*
- **Day-of-month / day-of-week OR-trap detector.** When both day fields are set,
  cron fires when *either* matches — not both. cronlens now flags this classic
  surprise under the run list, and reports it as `"or_trap": true` in JSON.

### Fixed

- The describer no longer drops the day-of-week clause when day-of-month is a
  star-prefixed step like `*/2` (e.g. `0 0 */2 * 1` now reads "…on every 2nd day
  of the month, and only on Monday").

### Changed

- GitHub release notes are now generated from this changelog's matching section
  instead of an auto-generated commit list.

### Deferred

- Quartz `L` / `W` / `#` day extensions and `--dst-policy vixie|utc|both`.

[0.4.0]: https://github.com/SNO7E-G/CronLens/releases/tag/v0.4.0

## [0.3.0] - 2026-07-31

The safety net: **warn before DST silently breaks a job.**

### Added

- **DST warnings.** The run list now flags upcoming daylight-saving hazards:
  a spring-forward time that does not exist (the run is *skipped*) and a
  fall-back time that occurs twice (the run may fire *twice*). When any warning
  is present the command exits `1`, so it gates in CI.
- **`--format json`** on the default command: a machine-readable object with the
  description, dialect, timezone, runs (local + UTC + ambiguity), and warnings —
  and `{ "valid": false, "error": … }` on invalid input.

### Deferred

- Quartz `L` / `W` / `#` day extensions and `--dst-policy vixie|utc|both` — the
  engine already classifies every run, so both layer on additively later.

[0.3.0]: https://github.com/SNO7E-G/CronLens/releases/tag/v0.3.0

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
