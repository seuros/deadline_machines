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

Requires **Ruby 4.0+**. The budget arithmetic is native: platform gems ship
precompiled for macOS and Linux, and a source install builds the extension
with a Rust toolchain.

---

## 🎯 Mission Brief

Every instant and span is **Integer nanoseconds**, as
`Process.clock_gettime(Process::CLOCK_MONOTONIC, :nanosecond)` reads them.

```ruby
SECOND = 1_000_000_000

DeadlineMachines.with_deadline(5 * SECOND) do |request|
  request.remaining        # => 5_000_000_000

  request.derive(SECOND) do |attempt|
    attempt.remaining      # => 1_000_000_000
    attempt.check          # => nil, or :exceeded / :cancelled
    attempt.check!         # raises DeadlineMachines::Exceeded when spent
  end

  DeadlineMachines.with_deadline(60 * SECOND) do
    DeadlineMachines.remaining # => 5_000_000_000, a nested scope cannot widen the budget
  end

  request.cancel('client hung up') # => :propagated (:already_cancelled on a repeat)
  DeadlineMachines.check!  # raises DeadlineMachines::Cancelled
end
```

The yielded object is a `Scope`: a frozen `Deadline` value plus a
`CancelToken`, read against `DeadlineMachines.clock`. `remaining` is `nil`
when there is no limit.

---

## 🛰️ Why the Fleet Needs It

Without a shared budget, a retry policy will happily spend thirty seconds on a
request the caller abandoned twenty-eight seconds ago. `clamp_sleep` is the hook
that stops it:

```ruby
delay = scope.clamp_sleep(backoff.next_delay_ns) # never overruns what is left
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
scope = DeadlineMachines::Scope.new(DeadlineMachines::Deadline.after(clock.now_ns, 1_000))
clock.advance(2_000)
scope.expired? # => true
```

`CLOCK_MONOTONIC` naps when the laptop does. To measure how long something
held across a suspend, use `DeadlineMachines::ContinuousClock.now_ns`, the
core's `ContinuousClock`: a kernel clock that keeps counting on Linux and
macOS, and on FreeBSD uptime plus suspend **estimated from the wall clock**
(no FreeBSD clock counts it; see the core's README). Nanoseconds since the
extension loaded. Not defined anywhere else: a stand-in that stops would lie.

`Deadline` itself holds no clock and mirrors the Rust core, `now` first:

```ruby
deadline = DeadlineMachines::Deadline.after(0, 100)
deadline.remaining(40)          # => 60
deadline.check(100)             # => :exceeded
deadline.clamp_sleep(80, 30)    # => 20
deadline.tighten(DeadlineMachines::Deadline.never) # => the same deadline
deadline.extend_by(50)          # the core's `extend`, renamed past Object#extend
```

`after`, `derive` and `extend_by` saturate an overflowing sum at the largest
instant the core holds (`Duration::MAX`); they never raise for it or produce
`never`, and `extend_by` on `never` stays `never`.

A `Deadline` is valid or does not exist: `expires_at` is `nil` or an Integer
in the core's `Duration` range, checked natively whether the gem built it or
you did. TypeError for the wrong type, RangeError out of range.

---

## 🔻 Cancellation Runs Down the Chain of Command

Cancellation flows down a tree and never up. A child derived after its parent was
cancelled is already cancelled, and cancelling it answers `:already_cancelled`
and changes nothing, its reason included. `on_cancel` fires once, and a callback
registered on an already-cancelled scope fires immediately. The block forms of
`derive` and `child` unlink the child on exit, so a long-lived parent does not
accumulate children. `tighten` and `extend_by` return a scope sharing the same
token.

---

## 🔩 The Rust Core

The arithmetic ships as the `deadline-machines` crate under
`ext/deadline_machines_native/core`: `no_std`, `Deadline` allocation-free with
zero dependencies, verified on bare metal. Every `Deadline` operation in this
gem goes through it. `CancelToken` and `Scope` there (feature `alloc`) are the
peers of the Ruby classes of the same names; the Ruby ones stay in Ruby
because they hold Ruby observers, reasons and fiber-bound state.

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
