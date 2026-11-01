# DeadlineMachines ⏱️

> **One clock for the whole fleet.** The tender that hands every ship the same
> budget, so nobody spends time the mission no longer has.

Deadline propagation and cooperative cancellation for Ruby.

`Timeout.timeout` raises wherever the interpreter happens to be, which is why it
corrupts connection state and why every serious library bans it. A deadline here
is a value you pass down and check at points you choose. **AMS Layland** carries
no weapons and trips no failures; it keeps the time the rest of the fleet
navigates by.

---

## 🚀 Launch Sequence

```bash
# Add to your ship's Gemfile
gem 'deadline_machines'
```

Requires **Ruby 4.0+**. Nothing else: no Rust toolchain, no platform gem, no
compiled extension.

---

## 🎯 Mission Brief

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

---

## 🛰️ Why the Fleet Needs It

Without a shared budget, a retry policy will happily spend thirty seconds on a
request the caller abandoned twenty-eight seconds ago. `clamp_sleep` is the hook
that stops it:

```ruby
delay = deadline.clamp_sleep(backoff.next_delay) # never overruns what is left
```

`DeadlineMachines.current` is fiber-scoped, so `Async` tasks inherit the ambient
deadline without being handed one.

---

## 🕰️ The Ship's Chronometer

Budgets are built on `CLOCK_MONOTONIC`, so an NTP step cannot stretch them.
Inject `TestClock` and the whole family becomes deterministic:

```ruby
clock = DeadlineMachines::TestClock.new
DeadlineMachines.clock = clock
deadline = DeadlineMachines::Deadline.after(1)
clock.advance(2)
deadline.expired? # => true
```

---

## 🔻 Cancellation Runs Down the Chain of Command

Cancellation flows down a tree and never up. A child derived after its parent was
cancelled is already cancelled. `on_cancel` fires once, and a callback registered
on an already-cancelled deadline fires immediately. The block form of `derive`
unlinks the child on exit, so a long-lived parent does not accumulate children.

---

## 🔩 The Rust Core, and Why No Extension

The same arithmetic and cancellation tree ships as the `deadline-machines` crate
under `ext/deadline_machines_native/core`, `no_std` with zero dependencies and
verified on bare metal. `Scope` there is the peer of `Deadline` here: a budget
and a cancellation handle as one value.

The gem itself is pure Ruby. Reading a monotonic clock is already a vDSO read,
so there is nothing here worth a native extension. The crate is there for Rust
consumers, not for this gem's install.

---

## 📡 Credits

**AMS Layland**, NCC-0112, *Layland*-class tender.

Named for C. L. Liu and James W. Layland's *Scheduling Algorithms for
Multiprogramming in a Hard-Real-Time Environment* (1973), which gave
earliest-deadline-first scheduling its proof and made a deadline a value a
system reasons about rather than an alarm it waits for.

---

## License

MIT.
