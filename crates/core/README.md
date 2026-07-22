# cronlens-core

The pure, I/O-free engine behind [cronlens](https://github.com/SNO7E-G/CronLens).

It parses cron expressions from several dialects (POSIX, Quartz, AWS EventBridge,
Kubernetes, GitHub Actions) into one normalized AST, turns that AST into plain
English, computes timezone- and DST-aware next runs, and flags the failure modes
that break cron in production — overlapping runs, thundering-herd scheduling, and
the day-of-month / day-of-week OR-trap.

The crate does no I/O and reads no clock of its own except where you hand it a
reference time, which keeps it usable from a CLI, a WASM playground, a
pre-commit hook, and a language server alike.

```rust
use cronlens_core::{parse, describe};

let expr = parse("0 9 * * 1-5").unwrap();
assert_eq!(describe(&expr), "At 9:00 AM, Monday through Friday.");
```

Dual-licensed under MIT or Apache-2.0.
