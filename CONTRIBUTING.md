# Contributing to cronlens

Thanks for your interest in improving cronlens. This project lives or dies on
**correctness**, so contributions of test cases and edge-case reports are every
bit as valuable as code.

## Getting started

You need a stable [Rust toolchain](https://rustup.rs) (the repo pins one via
`rust-toolchain.toml`).

```console
git clone https://github.com/SNO7E-G/CronLens
cd CronLens
cargo build
cargo test
```

The workspace has two crates:

- `crates/core` — the pure, I/O-free engine (parsing, describing, scheduling,
  analysis, conversion). No terminal or filesystem access lives here.
- `crates/cli` — a thin shell that parses arguments, renders output, and maps
  results to exit codes.

If you add behavior, it belongs in `core` with tests; the CLI should stay thin.

## Before you open a PR

Run the same checks CI runs:

```console
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

- **Formatting** is enforced (`cargo fmt --all --check`).
- **Clippy** runs with warnings denied.
- **Snapshot tests** use [`insta`](https://insta.rs). When you intentionally
  change output, review and accept snapshots with `cargo insta review`.

## Commit messages

We use [Conventional Commits](https://www.conventionalcommits.org/) so the
changelog and releases can be automated:

```
feat(parser): support the Quartz L/W/# tokens
fix(schedule): honor the dom/dow OR-trap on the boundary day
docs(readme): add the overlap-detection example
```

Common types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `chore`, `ci`.

## Correctness expectations

- Next-run behavior is validated against a reference implementation, not
  hand-authored expectations. If you touch the scheduler, add cases to the
  differential suite rather than asserting values you computed by hand.
- Timezone and DST logic must go through `chrono` / `chrono-tz`. Never hand-roll
  offset math.

## Reporting bugs

A great cron bug report includes the **expression**, the **dialect**, the
**timezone**, and what you **expected** versus what happened. Reduced test cases
are gold.
