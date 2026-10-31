# DeadlineMachines

Deadline propagation and cooperative cancellation for Ruby.

`Timeout.timeout` raises wherever the interpreter happens to be, which is why it
corrupts connection state and why every serious library bans it. A deadline here
is a value you pass down and check at points you choose.

```ruby
DeadlineMachines.with_deadline(5) do |request|
  request.remaining        # => 5.0

  request.derive(1) do |attempt|
    attempt.remaining      # => 1.0
    attempt.check!         # raises DeadlineMachines::Exceeded when spent
  end

  DeadlineMachines.with_deadline(60) do
    DeadlineMachines.remaining # => 5.0, a nested scope cannot widen the budget
  end

  request.cancel('client hung up')
  DeadlineMachines.check!  # raises DeadlineMachines::Cancelled
end
```

## Why the family needs it

Without a shared budget, a retry policy will happily spend thirty seconds on a
request the caller abandoned twenty-eight seconds ago. `clamp_sleep` is the hook
that stops it:

```ruby
delay = deadline.clamp_sleep(backoff.next_delay) # never overruns what is left
```

`DeadlineMachines.current` is fiber-scoped, so `Async` tasks inherit the ambient
deadline without being handed one.

## Clocks

Budgets are built on `CLOCK_MONOTONIC`, so an NTP step cannot stretch them.
Inject `TestClock` and the whole family becomes deterministic:

```ruby
clock = DeadlineMachines::TestClock.new
DeadlineMachines.clock = clock
deadline = DeadlineMachines::Deadline.after(1)
clock.advance(2)
deadline.expired? # => true
```

## Cancellation semantics

Cancellation flows down a tree and never up. A child derived after its parent was
cancelled is already cancelled. `on_cancel` fires once, and a callback registered
on an already-cancelled deadline fires immediately. The block form of `derive`
unlinks the child on exit, so a long-lived parent does not accumulate children.

## Rust

The same arithmetic and cancellation tree ships as the `deadline-machines` crate
under `ext/deadline_machines_native/core`, `no_std` with zero dependencies and
verified on bare metal. `Scope` there is the peer of `Deadline` here: a budget
and a cancellation handle as one value.

The gem itself is pure Ruby. Reading a monotonic clock is already a vDSO read,
so there is nothing here worth a native extension.

## License

MIT.
