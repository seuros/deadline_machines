# Changelog

## 0.1.0

- Initial release.
- `DeadlineMachines::Deadline`: monotonic budgets, `derive` for child scopes that
  cannot outlive their parent, cancellation that flows down a tree, and `check!`
  as the only place an error is raised.
- `DeadlineMachines.with_deadline` for a fiber-scoped ambient deadline.
- `DeadlineMachines::TestClock` for deterministic tests across the family.
- `deadline-machines` core crate: the same arithmetic and cancellation tree in
  `no_std` Rust with zero dependencies, plus `Scope` pairing a budget with a
  cancellation handle. Builds and lints clean on bare metal.
- RBS signatures for the whole public API.
