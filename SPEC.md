# cronlens — Specification

> **Working name:** `cronlens` *(a lens on your cron)*.
> Alternatives if you want something else: `whenwill`, `cronx`, `krono`, `cronwise`.
> Pick one before you publish and search npm / PyPI / crates.io to make sure it's free.

**One-liner:** Paste a crontab, get plain English, the next N runs in any timezone, and warnings for the things that actually break cron jobs in production — DST transitions, overlapping runs, and thundering-herd scheduling.

**Tagline for the README:** *"cron for humans — with a safety net."*

---

## 1. Why this project exists

Cron syntax is famously unreadable, and the tooling around it is split into two camps that never meet:

1. **Translators** (`cronstrue`, crontab.guru, a dozen web tools) turn `0 9 * * 1-5` into "At 9:00 AM, Monday through Friday." They stop there. No timing math, no timezone, no warnings.
2. **Schedulers / next-run libraries** (`croniter`, `cronsim`, `CronDst`, `tzcron`) compute *when* a job fires and some handle DST — but they output raw `datetime` objects, not human explanations, and they're libraries, not tools you run in a terminal.

The result: developers still deploy cron jobs that fire at the wrong time twice a year (DST), pile every job onto `0 0 * * *` and hammer the box at midnight, or schedule a 40-minute job every 30 minutes and silently stack overlapping runs. No single tool warns them.

**cronlens is the missing middle:** one CLI that reads the schedule the way a human would, tells you exactly when it will run, and flags the failure modes before they hit production.

---

## 2. Competitive landscape

| Tool | Type | Translation | Next runs | Timezone | DST warnings | Overlap warnings | Crontab-file lint |
|------|------|:-----------:|:---------:|:--------:|:------------:|:----------------:|:-----------------:|
| **cronstrue** | JS lib | ✅ (30+ langs) | ❌ | ❌ | ❌ | ❌ | ❌ |
| crontab.guru | Web | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| CronSignal / crontranslator.com | Web | ✅ | ✅ | partial | ❌ | ❌ | partial |
| drumbeats.io | Web | ✅ | ✅ | ❌ | edge-case notes | ❌ | ❌ |
| croniter / cronsim / CronDst / tzcron | Py libs | ❌ | ✅ | ✅ | ✅ (math only) | ❌ | ❌ |
| DST debuggers (cronmonitor etc.) | Web | partial | ✅ | ✅ | ✅ | ❌ | ❌ |
| **cronlens (this project)** | **CLI + lib** | ✅ | ✅ | ✅ | ✅ | ✅ **(unique)** | ✅ **(rare)** |

**Where cronlens is genuinely differentiated:**

1. **Overlap detection.** Give cronlens the schedule *and* an estimated job duration (or let it observe real durations from logs) and it warns: *"This runs every 30 min but averages 42 min — expect overlapping executions."* No existing tool does this.
2. **Whole-crontab linting.** Point it at a real `crontab` file and it lints *across* jobs: thundering-herd detection (too many jobs at `0 0`), duplicate schedules, jobs clustered on DST-dangerous minutes.
3. **DST-aware, human-readable warnings in a CLI.** The math exists in libraries; the plain-English "this job will be skipped on March 9" warning in a terminal does not.
4. **Multi-dialect input, one mental model.** Vixie/POSIX, Quartz (6–7 field), AWS EventBridge, systemd `OnCalendar`, Kubernetes CronJob, GitHub Actions — parse them all, explain them the same way, and convert between them.
5. **Actually a CLI** (plus an importable library core). The dominant translator is a library with no CLI; the web tools can't go in your CI pipeline. cronlens can run as a pre-commit hook or CI gate.

---

## 3. Target users

- **DevOps / SRE** auditing crontabs, K8s CronJobs, and CI schedules before deploy.
- **Backend developers** who write a cron line every few months and don't want to re-learn the syntax.
- **Anyone reviewing a PR** that touches `.github/workflows/*.yml` or a `CronJob` manifest and wants a plain-English diff.
- **Teams** who want a CI check that blocks dangerous schedules.

---

## 4. Core features (MVP — v0.1 → v0.3)

### 4.1 Translate
```
$ cronlens "0 9 * * 1-5"
At 9:00 AM, Monday through Friday.
```
- Full support for `* / , - ? L W #` and named values (`JAN`, `MON`).
- Nicknames: `@yearly @annually @monthly @weekly @daily @hourly @reboot`.
- 5-field (POSIX), 6-field (seconds or year), 7-field (Quartz) detection.
- `--verbose` for a field-by-field breakdown.

### 4.2 Next runs
```
$ cronlens "0 9 * * 1-5" --next 10 --tz America/New_York
Next 10 runs (America/New_York):
  1.  Mon 2026-07-27  09:00 EDT
  2.  Tue 2026-07-28  09:00 EDT
  ...
```
- `--next N` (default 5), `--tz <IANA zone>` (default: system tz).
- `--from <datetime>` to compute from an arbitrary anchor.
- `--utc` shortcut.

### 4.3 DST warnings
```
$ cronlens "30 2 * * *" --tz America/New_York --next 200
⚠ 2027-03-14 02:30 does not exist (spring-forward). Run will be SKIPPED.
⚠ 2026-11-01 02:30 occurs twice (fall-back). Run may fire TWICE.
```
- Detect non-existent local times (spring-forward gap) and ambiguous times (fall-back repeat).
- Make the daemon's behavior explicit and configurable (`--dst-policy vixie|utc|both`), because implementations differ.

### 4.4 Validation
```
$ cronlens "60 25 * * *"
✗ Invalid: minute 60 out of range (0–59); hour 25 out of range (0–23).
```
- Precise, field-level error messages with the offending token highlighted.
- Non-zero exit code so it works as a CI gate.

---

## 5. Advanced features (the "wow" — v0.4 → v1.0)

### 5.1 Overlap detection *(signature feature)*
```
$ cronlens "*/30 * * * *" --duration 42m
⚠ Interval is 30m but the job averages 42m.
  Runs will overlap: run #2 starts 12m before run #1 finishes.
  Suggested fixes: increase interval to ≥45m, add a lock (flock),
  or switch to a queue-based trigger.
```
- Accept a `--duration` estimate, or a `--from-logs <file>` to infer real durations.
- Model the timeline and report worst-case backlog depth.

### 5.2 Crontab-file linting
```
$ cronlens lint /etc/crontab
crontab: 14 jobs
⚠ Thundering herd: 9 jobs fire at 00:00 — stagger with jitter.
⚠ 3 jobs share schedule "0 3 * * *" — possible copy-paste duplication.
⚠ Line 22 "30 2 * * *": DST-dangerous minute (see --explain).
✓ No syntax errors.
```
- Rule set: thundering-herd, duplicate schedules, DST-dangerous windows, suspiciously frequent jobs, `@reboot` misuse.
- `--fix` to auto-suggest jittered alternatives.
- Machine-readable `--format json|sarif` for CI annotations.

### 5.3 Cross-format conversion
```
$ cronlens "0 9 * * 1-5" --to github-actions
on:
  schedule:
    - cron: '0 9 * * 1-5'   # ⚠ GitHub Actions runs in UTC only

$ cronlens "0 9 * * 1-5" --to systemd
OnCalendar=Mon..Fri *-*-* 09:00:00

$ cronlens "0 9 * * 1-5" --to aws
cron(0 9 ? * MON-FRI *)     # AWS uses 6 fields + '?'
```
- Targets: `posix`, `quartz`, `aws`, `systemd`, `k8s`, `github-actions`.
- Warn on lossy conversions (e.g. seconds field dropped, UTC-only platforms).

### 5.4 Reverse mode (English → cron)
```
$ cronlens gen "every weekday at 9am"
0 9 * * 1-5
```
- Constrained natural-language parser (not an LLM dependency for the core — deterministic grammar first; optional LLM plug-in later).

### 5.5 Explain-a-diff
```
$ cronlens diff "0 9 * * 1-5" "0 9 * * *"
Before: At 9:00 AM, Monday through Friday.
After:  At 9:00 AM, every day.
Δ Now also runs on Saturday and Sunday (+2 days/week, +104 runs/year).
```
Perfect for PR review of schedule changes.

### 5.6 Calendar / visual output
- `--calendar` renders an ASCII heatmap of when the job fires across a week/month in the terminal.
- `--ics` exports the next N runs as an `.ics` file you can drop into a calendar.

---

## 6. Command surface (UX design)

```
cronlens <expr>                     # translate + next 5 runs (default)
cronlens explain <expr>             # verbose field-by-field
cronlens next <expr> [--next N]     # just the run list
cronlens gen "<english>"            # english → cron
cronlens diff <expr> <expr>         # compare two schedules
cronlens convert <expr> --to <fmt>  # cross-format
cronlens lint <file>                # whole-crontab audit
cronlens check <expr>               # validate only, exit code for CI

Global flags:
  --tz <IANA>        target timezone (default: system)
  --utc              shortcut for --tz UTC
  --next N           number of upcoming runs (default 5)
  --from <datetime>  anchor point
  --duration <dur>   job runtime for overlap analysis
  --dst-policy       vixie | utc | both
  --format           text | json | sarif
  --no-color / --color
  --quiet / --verbose
```

**Design principles**
- Zero-arg-friendly: `cronlens "0 * * * *"` just works.
- Every warning is *actionable* — it names the fix, not just the problem.
- `--format json` on everything, so it composes into scripts and CI.
- Exit codes: `0` ok, `1` warnings, `2` errors, `3` invalid input. (Configurable so warnings can be non-blocking.)
- Respects `NO_COLOR`, pipes cleanly (auto-disable color when not a TTY).

---

## 7. Architecture & tech stack

**Recommended: Rust** (single static binary, trivial `brew`/`cargo`/`scoop` install, no runtime). Alternative: **Go** (same benefits) or **TypeScript/Node** (fastest to build on top of the existing cron ecosystem, but needs a runtime).

```
cronlens/
├── crates/ (or packages/)
│   ├── core/           # parsing, translation, next-run engine — pure, no I/O
│   │   ├── parser/     # dialect-aware tokenizer (posix, quartz, aws, systemd…)
│   │   ├── describe/   # AST → English (i18n-ready)
│   │   ├── schedule/   # next-run iterator (tz + DST aware)
│   │   ├── analyze/    # DST, overlap, thundering-herd rules
│   │   └── convert/    # cross-format emitter
│   ├── cli/            # arg parsing, rendering, colors, exit codes
│   └── wasm/           # optional: compile core to WASM for a web playground
├── tests/
│   ├── fixtures/       # hundreds of (expr → english → next-runs) golden files
│   └── property/       # fuzz + property tests
├── docs/
├── README.md
├── SPEC.md             # this file
└── CHANGELOG.md
```

**Key architectural rule:** the **core is pure and I/O-free** so it can power the CLI, a WASM web playground, a pre-commit hook, and a language-server all from one engine. The CLI is a thin shell.

**Timezone/DST:** use a battle-tested IANA tz database binding (Rust: `chrono-tz` / `jiff`; Go: stdlib `time`; Node: `Temporal` / `luxon`). Do **not** hand-roll DST math.

---

## 8. Supported input dialects

| Dialect | Fields | Notes |
|---------|--------|-------|
| POSIX / Vixie crontab | 5 | The default; `@nicknames` supported |
| Quartz | 6–7 | Seconds + optional year; `? L W #` |
| AWS EventBridge | 6 | Requires `?` in day-of-month **or** day-of-week |
| Kubernetes CronJob | 5 | POSIX; note `.spec.timeZone` |
| GitHub Actions | 5 | POSIX; **UTC only** — always warn |
| systemd `OnCalendar` | n/a | Different grammar; parse & translate |

Auto-detect field count and dialect; let `--dialect` override when ambiguous.

---

## 9. The advanced engine, in detail

### 9.1 DST correctness
For each candidate run, resolve the *local* wall-clock time against the target zone:
- **Non-existent** (spring-forward gap) → mark as **skipped**, explain.
- **Ambiguous** (fall-back repeat) → mark as **possibly-twice**, explain.
- Offer `--dst-policy` to model different daemon behaviors (Vixie's special >1h-granularity handling vs. pure-UTC scheduling), because real systems disagree here — surface the assumption instead of hiding it.

### 9.2 Overlap model
Given interval `I` and duration `D` (fixed estimate or sampled distribution):
- If `D ≥ I`: guaranteed overlap; compute steady-state backlog growth.
- If `D` distribution has a tail crossing `I`: report probability of overlap and worst observed case from logs.
- Output the concrete fix (bigger interval, `flock`, queue).

### 9.3 Thundering-herd / crontab lint
Bucket all jobs by fire-time; flag buckets above a threshold; recommend jitter (e.g. spread `0 0 * * *` jobs across `0-9 0 * * *`). Detect exact-duplicate schedules and DST-dangerous minutes (00:00–03:59 in DST-observing zones).

---

## 10. Roadmap

### Phase 0 — Foundations (weeks 1–2)
- Repo scaffolding, CI, license (MIT or Apache-2.0), CONTRIBUTING, code of conduct.
- Core parser for POSIX 5-field + nicknames.
- `translate` command with golden-file tests.
- **Ship v0.1** — translation only, but clean and well-tested.

### Phase 1 — Timing (weeks 3–4) → **v0.2**
- Next-run iterator, `--tz`, `--next`, `--utc`, `--from`.
- Validation with field-level errors and CI-friendly exit codes.

### Phase 2 — Safety net (weeks 5–7) → **v0.4**
- DST detection + warnings.
- Quartz / 6–7 field support.
- `--format json`.

### Phase 3 — The differentiators (weeks 8–11) → **v0.6**
- **Overlap detection** (`--duration`, `--from-logs`).
- **`lint`** for whole crontab files (thundering-herd, duplicates, DST-dangerous).
- `--format sarif` for CI annotations.

### Phase 4 — Interop (weeks 12–14) → **v0.8**
- Cross-format `convert` (aws, systemd, k8s, github-actions, quartz).
- `diff` command.
- `gen` (deterministic English → cron).

### Phase 5 — Polish & reach → **v1.0**
- `--calendar` ASCII heatmap, `--ics` export.
- WASM web playground (marketing + SEO magnet).
- i18n for translations (start with the languages cronstrue supports so you're at parity).
- Distribution: Homebrew tap, `cargo install`, scoop, a `curl | sh` installer, prebuilt binaries per-OS via GitHub Releases, a pre-commit hook, and a GitHub Action.

### Beyond v1.0 — moonshots
- **Language Server (LSP)** so editors show the English translation inline as you type a cron line in YAML/crontab.
- **VS Code extension** (hover a cron string → translation + next runs + warnings).
- **Log-driven monitoring mode**: feed it real run logs, it learns durations and predicts future overlaps and misses.
- **`explain --why-didnt-it-run`**: given a schedule, a timezone, and an expected time, diagnose why a job didn't fire (DST, wrong tz, day-of-month vs day-of-week OR-trap).
- **The day-of-month/day-of-week OR-trap detector** — the classic gotcha where specifying both fields makes cron run on *either*, not *both*. Flag it loudly; almost nobody knows this rule.

---

## 11. Testing strategy

- **Golden files:** a large corpus of `expr → expected english → expected next-runs`. Snapshot-tested.
- **Property tests / fuzzing:** random valid expressions round-trip through parse → describe → parse.
- **Differential testing:** compare next-run output against a reference implementation (e.g. `croniter` / Debian cron) across thousands of expressions and a decade of dates, including every DST transition in several zones.
- **Timezone matrix:** run the DST suite against a spread of zones (US, EU, Australia, and a half-hour offset like `Australia/Lord_Howe`).

---

## 12. What makes the GitHub repo *showcase-worthy*

The spec is only half of it — a repo gets stars because the README sells the story in 10 seconds. Do these:

1. **A single killer GIF at the top** of the README: paste a gnarly cron line, watch it translate, list next runs, and throw a DST warning — all in the terminal. This is the whole pitch in one loop.
2. **Lead with the differentiator, not the category.** The first line should be about the *safety net* (DST + overlap warnings), because "cron translator" alone is crowded. "The only cron tool that warns you before DST silently skips your job."
3. **A live WASM playground** linked at the top — visitors try it without installing. Great for SEO and for HN/Reddit launch traffic.
4. **A comparison table** (like §2 here) right in the README, honestly showing where you win.
5. **Copy-paste install** for every platform, and a **GitHub Action** so people adopt it in CI (that's how a dev tool goes viral inside companies).
6. **Badges that matter:** build passing, coverage, crates.io/npm version, "featured on" if you land HN.
7. **A "why" section** with the three horror stories (DST double-run, midnight thundering-herd, overlapping backups) — concrete pain sells.
8. Tidy **`good first issue`** labels and a clear CONTRIBUTING to attract contributors after launch.

**Launch checklist:** post to Hacker News ("Show HN"), r/devops, r/commandline, Lobsters, and a short blog post titled around the DST gotcha (that's the searchable, shareable hook). Submit to `awesome-cli` and `awesome-devops` lists.

---

## 13. Success metrics

- ⭐ GitHub stars (vanity but real for dev-tool credibility).
- Installs (Homebrew analytics, crates.io/npm downloads).
- CI adoption (uses of the GitHub Action).
- Issues/PRs from outside contributors (health of the project).
- "Correctness" bug count trending to zero on the differential test suite.

---

## 14. License & governance

- **License:** MIT or Apache-2.0 (permissive → maximum adoption for a dev tool).
- Semantic versioning; keep a `CHANGELOG.md`.
- Conventional commits so releases can be automated.

---

*This spec is a starting point, not a contract — trim the roadmap to what one person can ship, and get v0.1 (translate only, beautifully tested) out fast. Momentum comes from shipping the small clean core, then layering the differentiators the crowded field is missing.*
