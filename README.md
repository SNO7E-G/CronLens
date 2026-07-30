<div align="center">

# cronlens

**cron for humans — with a safety net.**

Paste a cron expression and get plain English, the next runs in any timezone, and
warnings for the things that actually break cron jobs in production: DST
transitions, overlapping runs, and thundering-herd scheduling.

[![CI](https://github.com/SNO7E-G/CronLens/actions/workflows/ci.yml/badge.svg)](https://github.com/SNO7E-G/CronLens/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
![Status](https://img.shields.io/badge/status-alpha-orange.svg)

</div>

---

## Why cronlens

Cron syntax is famously unreadable, and the tooling splits into two camps that
never meet. **Translators** (`cronstrue`, crontab.guru) turn `0 9 * * 1-5` into
"At 9:00 AM, Monday through Friday" and stop there — no timing, no timezone, no
warnings. **Scheduler libraries** (`croniter`, `cronsim`) compute *when* a job
fires but hand you raw datetimes, not explanations, and live inside your code,
not your terminal.

So developers still ship cron jobs that:

- **fire at the wrong time twice a year** because nobody modeled the DST
  transition — the 2:30 AM job that silently *skips* every spring and runs
  *twice* every fall;
- **hammer the box at midnight** because every job landed on `0 0 * * *` — a
  self-inflicted thundering herd;
- **stack overlapping runs** because a 40-minute job is scheduled every 30
  minutes and nothing warned that it would.

**cronlens is the missing middle:** one CLI that reads the schedule the way a
human would, tells you exactly when it will run, and flags the failure modes
*before* they reach production.

```console
$ cronlens "0 9 * * 1-5"
At 9:00 AM, Monday through Friday.
```

## What makes it different

|                              | cronstrue | crontab.guru | croniter | **cronlens** |
| ---------------------------- | :-------: | :----------: | :------: | :----------: |
| Plain-English translation    |     ✅    |      ✅      |    ❌    |      ✅      |
| Next runs                    |     ❌    |      ✅      |    ✅    |      ✅      |
| Timezone aware               |     ❌    |      ❌      |    ✅    |      ✅      |
| **DST skip / double warnings** |   ❌    |      ❌      | math only |    ✅      |
| **Overlap detection**        |     ❌    |      ❌      |    ❌    |    ✅ *(unique)* |
| **Whole-crontab lint**       |     ❌    |    partial   |    ❌    |    ✅ *(rare)* |
| Cross-dialect convert        |     ❌    |      ❌      |    ❌    |      ✅      |
| Runs in your terminal / CI   |     ❌    |      ❌      | library  |      ✅      |

Three capabilities are genuinely rare or unique:

1. **Overlap detection.** Give cronlens a job duration and it warns when runs
   will pile up: *"runs every 30m but averages 42m — run #2 starts 12m before
   run #1 finishes."*
2. **Whole-crontab linting.** Point it at a real crontab and it lints *across*
   jobs — thundering-herd at `0 0`, duplicate schedules, DST-dangerous minutes.
3. **DST-aware, human-readable warnings in a terminal.** The math exists in
   libraries; the plain-English *"this run will be skipped on March 9"* in your
   shell does not.

## Install

> **Status: alpha.** The engine and command surface are being built out in
> public across tagged releases (see [Roadmap](#roadmap)). Prebuilt binaries are
> attached to each GitHub release.

From source (requires a [Rust toolchain](https://rustup.rs)):

```console
cargo install --git https://github.com/SNO7E-G/CronLens cronlens-cli
```

Homebrew, Scoop, `cargo install cronlens`, and a `curl | sh` installer land with
v1.0.

## Usage

```console
# Translate + the next runs (the default)
$ cronlens "0 9 * * 1-5" --tz America/New_York --next 4
At 9:00 AM, Monday through Friday.

Next 4 runs (America/New_York):
   1. Thu 2026-07-23 09:00 AM EDT
   2. Fri 2026-07-24 09:00 AM EDT
   3. Mon 2026-07-27 09:00 AM EDT
   4. Tue 2026-07-28 09:00 AM EDT
```

```console
cronlens --verbose "0 0 1 * *"                # field-by-field breakdown
cronlens "*/15 * * * *" --utc                 # runs in UTC
cronlens "0 3 * * *" --from 2026-03-01        # anchor the run list at a date
cronlens "30 2 * * *" --tz America/New_York   # ⚠ warns about the DST-skipped run
cronlens "0 9 * * 1-5" --format json          # machine-readable output for CI
cronlens --dialect quartz "0 0 12 * * ?"      # pin a dialect
cronlens "0 9 * * 1-5" --next 0               # translation only, no run list
```

When a run in the window falls in a spring-forward gap or a fall-back overlap,
`cronlens` says so and exits `1`:

```console
$ cronlens "30 2 * * *" --tz America/New_York --from 2027-03-12 --next 5
...
⚠ 2027-03-14 02:30 does not exist (spring-forward) — this run will be SKIPPED.
```

Timezone/DST math is correct across transitions, and invalid input is reported
per field with a non-zero exit code, so `cronlens` works as a CI gate. More
commands — `lint`, `convert`, `diff`, `gen` — arrive with the releases below.

## Roadmap

cronlens ships in small, well-tested increments. Each phase is a release.

| Release | Theme          | Highlights                                                        |
| ------- | -------------- | ----------------------------------------------------------------- |
| ✅ v0.1 | Translate      | POSIX parser, nicknames, plain-English `describe`, golden tests   |
| ✅ v0.2 | Timing         | Next-run engine (DST-correct, croniter-verified), `--tz` / `--next` / `--from`, multi-error validation |
| ✅ v0.3 | Safety net     | DST skip/double warnings, `--format json`, CI exit codes         |
| v0.4    | Quartz         | Quartz `L` / `W` / `#` day extensions, `--dst-policy`             |
| v0.6    | Differentiators| Overlap detection, whole-crontab `lint`, SARIF output             |
| v0.8    | Interop        | Cross-dialect `convert`, `diff`, deterministic `gen` (English → cron) |
| v1.0    | Polish & reach | `--calendar` heatmap, `.ics` export, WASM playground, distribution |

## Architecture

A pure, I/O-free core drives everything, so the same engine can power the CLI, a
future WASM playground, a pre-commit hook, and a language server.

```
crates/
├── core/   # parse → describe → schedule → analyze → convert (no I/O)
└── cli/    # thin shell: args, rendering, colors, exit codes
```

Timezone and DST math is delegated to the battle-tested IANA database via
`chrono` / `chrono-tz` — never hand-rolled.

## Contributing

Issues and PRs are welcome. See [CONTRIBUTING](CONTRIBUTING.md) and the
[Code of Conduct](CODE_OF_CONDUCT.md). The whole engine is snapshot- and
property-tested; correctness is the product.

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
