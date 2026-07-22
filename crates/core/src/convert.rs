//! Cross-dialect conversion: emit an expression in another flavor.
//!
//! Fills in during phase 4 (v0.8). Targets: POSIX, Quartz, AWS EventBridge,
//! Kubernetes, systemd `OnCalendar`, and GitHub Actions. Lossy conversions
//! (a dropped seconds field, a UTC-only target) are reported rather than
//! silently applied.
