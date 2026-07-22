# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Workspace scaffolding: a pure, I/O-free `cronlens-core` crate and a thin
  `cronlens-cli` binary.
- The normalized cron AST shared across the engine, including first-class
  handling of the day-of-month / day-of-week **OR-trap**.
- Structured, span-carrying parse errors for precise CLI diagnostics.
- Continuous integration: rustfmt, clippy (deny warnings), a Linux/macOS/Windows
  test matrix, and a documentation build.
- Dual MIT / Apache-2.0 licensing, contributing guide, and code of conduct.
