# deadline-machines

Monotonic deadline arithmetic and cooperative cancellation trees. `no_std`;
`Deadline` is allocation-free and has zero dependencies.

Every instant is a `core::time::Duration` since an epoch the caller picks, and
every span is a `Duration`. `Deadline` holds no clock: the caller reads its
own and passes `now` in.

```rust
use core::time::Duration;
use deadline_machines::{Deadline, Refusal};

let now = Duration::ZERO;
let request = Deadline::after(now, Duration::from_secs(5));

// A child never outlives its parent.
let attempt = request.derive(now, Duration::from_secs(9));
assert_eq!(attempt, request);

// A backoff that cannot overrun the budget.
let later = Duration::from_secs(4);
assert_eq!(request.clamp_sleep(later, Duration::from_secs(3)), Duration::from_secs(1));

// The checkpoint. A refusal records nothing: it answers for that instant.
assert_eq!(request.check(Duration::from_secs(5)), Err(Refusal::Exceeded));
assert_eq!(request.check(later), Ok(()));
```

With the `alloc` feature, `CancelToken` is a cancellation tree that flows down
and never up, and `Scope` pairs a deadline with a token, which is what a retry
policy or a circuit breaker actually consults:

```rust
use core::time::Duration;
use deadline_machines::cancel::Cancellation;
use deadline_machines::{Deadline, Refusal, Scope};

let now = Duration::ZERO;
let request = Scope::new(Deadline::after(now, Duration::from_secs(5)));
let attempt = request.derive(now, Duration::from_secs(9));

// A child cannot outlive its parent.
assert_eq!(attempt.deadline(), request.deadline());

assert_eq!(request.cancel(), Cancellation::Propagated);
assert_eq!(request.cancel(), Cancellation::AlreadyCancelled);
assert_eq!(attempt.check(now), Err(Refusal::Cancelled));

// A budget that paused should not pay for the wait.
let resumed = attempt.extend(Duration::from_secs(2));
assert_eq!(resumed.deadline().expires_at(), Some(Duration::from_secs(7)));
```

`after`, `derive` and finite `extend` saturate an overflowing sum at
`Duration::MAX`; they never refuse or produce `never`, and extending `never`
leaves it `never`. `derive` then applies the parent's tighter bound.

`Deadline` builds on bare metal with no features; `CancelToken` and `Scope`
need `alloc`.

The `tokio` feature adds `TokioClock`, the one clock this crate reads. It
follows Tokio's time, paused test time included, so one deadline drives both
the arithmetic and the executor's timer:

```rust
use core::time::Duration;
use deadline_machines::tokio_support::{TokioClock, timeout};
use deadline_machines::{Deadline, Refusal};

# async fn example() -> Result<(), Refusal> {
let clock = TokioClock::new();
let deadline = Deadline::after(clock.now(), Duration::from_secs(2));
timeout(&clock, deadline, async { do_work().await }).await?;
# Ok(())
# }
# async fn do_work() {}
```

A `Scope` runs a future under its remaining budget and its cancellation at
once, and `cancelled()` is a cancel-safe future for `select!`:

```rust
use core::time::Duration;
use deadline_machines::tokio_support::TokioClock;
use deadline_machines::{Deadline, Refusal, Scope};

# async fn example() -> Result<(), Refusal> {
let clock = TokioClock::new();
let request = Scope::new(Deadline::after(clock.now(), Duration::from_secs(2)));
let attempt = request.child();

let output = attempt.tokio_timeout(&clock, async { do_work().await }).await?;
# let () = output;
# Ok(())
# }
# async fn do_work() {}
```

A deadline past the last instant Tokio can represent starts no timer: no
timer would ever reach it.

The `tokio-util` feature bridges both ways with `tokio_util::CancellationToken`:
`CancelToken::linked_to(&token)` is cancelled whenever `token` is, and
`to_cancellation_token()` hands out one that follows this tree.

This crate is the time contract the rest of the machines family builds on:
`chrono-machines` clamps its backoff to it, `breaker-machines` fails fast when it
is already spent.
