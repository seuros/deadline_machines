# deadline-machines

Monotonic deadline arithmetic and cooperative cancellation trees. `no_std`, zero
dependencies.

```rust
use deadline_machines::{cancel::CancelToken, Deadline, Instant};

let now = Instant(0);
let request = Deadline::after(now, 5 * 1_000_000_000);

// A child never outlives its parent.
let attempt = request.derive(now, 9 * 1_000_000_000);
assert_eq!(attempt.expires_at(), request.expires_at());

// A backoff that cannot overrun the budget.
assert_eq!(request.clamp_sleep_ns(Instant(4_000_000_000), 3_000_000_000), 1_000_000_000);

let token = CancelToken::root();
let child = token.child();
token.cancel();
assert!(child.is_cancelled());
```

`Scope` pairs the two, which is what a retry policy or a circuit breaker
actually consults:

```rust
use deadline_machines::{Deadline, Instant, Scope, Stopped};

let now = Instant(0);
let request = Scope::new(Deadline::after(now, 5_000));
let attempt = request.derive(now, 9_000);

// A child cannot outlive its parent.
assert_eq!(attempt.deadline(), request.deadline());

request.cancel();
assert_eq!(attempt.check(now), Err(Stopped::Cancelled));
```

`Deadline` is `Copy` and holds no clock, so callers keep their own time source
and pass `Instant` in. `Scope` and `CancelToken` need the `alloc` feature;
`SystemClock` needs `std`. Everything builds on bare metal.

This crate is the time contract the rest of the machines family builds on:
`chrono-machines` clamps its backoff to it, `breaker-machines` fails fast when it
is already spent.
