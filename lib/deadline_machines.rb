# frozen_string_literal: true

require_relative 'deadline_machines/version'

# Every instant and span in this gem is Integer nanoseconds: `now` as read
# from `Process.clock_gettime(Process::CLOCK_MONOTONIC, :nanosecond)`, and
# every timeout, extension, remaining budget and sleep. They reach the core
# as a `Duration`, so each is in 0..Duration::MAX nanoseconds; TypeError for
# anything but an Integer, RangeError outside that range.
module DeadlineMachines
  # When a budget runs out: `expires_at` is an instant in Integer
  # nanoseconds, or nil for never. A frozen value, validated through the core
  # whether the gem built it or the caller did. `never`, `at`, `after` and
  # every operation below are native and mirror the core's `Deadline`;
  # `extend_by` is the core's `extend`, renamed so it does not shadow
  # `Object#extend`.
  Deadline = Data.define(:expires_at) do
    def initialize(expires_at:)
      DeadlineMachines.__send__(:__deadline, expires_at)
      super
    end
  end
end

# Native is required: no algorithm fallback. It validates and builds the
# Deadline above and adds its arithmetic, so the class must exist first.
require 'deadline_machines_native/deadline_machines_native'

module DeadlineMachines
  private_class_method :__deadline, :__deadline_of

  # The value is sealed. `initialize` is where it validates, and a subclass
  # could override it, or `allocate` build an instance that never ran it:
  # subclassing raises TypeError and `allocate` is gone. Sealing stops
  # accidents, not reflection (`Marshal.load` skips `initialize`), so the
  # native side is the authority: a Deadline it reads must be exactly that
  # class, and its expiry is checked again.
  Deadline.singleton_class.undef_method(:allocate)
  Deadline.define_singleton_method(:inherited) { |_| raise TypeError, "#{Deadline} cannot be subclassed" }
end

require_relative 'deadline_machines/errors'
require_relative 'deadline_machines/clock'
require_relative 'deadline_machines/cancel_token'
require_relative 'deadline_machines/scope'

# The ambient scope: a runtime layered on the core, bound per fiber.
module DeadlineMachines
  FIBER_KEY = :deadline_machines_scope

  class << self
    attr_writer :clock

    # What every scope built without an explicit clock reads.
    def clock
      @clock ||= MonotonicClock
    end

    # The scope bound to this fiber, or nil.
    def current
      Fiber[FIBER_KEY]
    end

    # Runs the block under a scope bound to this fiber, and returns its value.
    # Under an ambient scope it is a child of that one, tightened to
    # +timeout+ nanoseconds when given; otherwise a root scope that runs out
    # +timeout+ nanoseconds from now, or never.
    def with_deadline(timeout = nil, &)
      ambient = current
      if ambient
        return ambient.child { |scope| bind(scope, &) } if timeout.nil?

        return ambient.derive(timeout) { |scope| bind(scope, &) }
      end
      deadline = timeout.nil? ? Deadline.never : Deadline.after(clock.now_ns, timeout)
      bind(Scope.new(deadline), &)
    end

    # Binds +scope+ to this fiber for the block, restoring the previous one.
    def bind(scope)
      raise TypeError, 'scope must be a DeadlineMachines::Scope' unless scope.instance_of?(Scope)

      previous = Fiber[FIBER_KEY]
      Fiber[FIBER_KEY] = scope
      begin
        yield scope
      ensure
        Fiber[FIBER_KEY] = previous
      end
    end

    # The ambient checkpoint: raises Cancelled or Exceeded, else returns the
    # scope, or nil outside one.
    def check!
      current&.check!
    end

    # Nanoseconds left in the ambient scope, nil when there is no limit.
    def remaining
      current&.remaining
    end

    # The longest sleep up to +requested+ nanoseconds that fits the ambient
    # budget; +requested+ itself outside a scope.
    def clamp_sleep(requested)
      scope = current
      scope ? scope.clamp_sleep(requested) : Deadline.never.clamp_sleep(0, requested)
    end
  end
end
