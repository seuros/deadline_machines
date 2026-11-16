# frozen_string_literal: true

module DeadlineMachines
  # The process's monotonic clock, in Integer nanoseconds.
  module MonotonicClock
    def self.now_ns
      Process.clock_gettime(Process::CLOCK_MONOTONIC, :nanosecond)
    end
  end

  # A clock that moves only when told to, for deterministic tests.
  class TestClock
    attr_reader :now_ns

    def initialize(now_ns = 0)
      @now_ns = nanoseconds(now_ns)
    end

    # Sets the clock to +now_ns+.
    def now_ns=(now_ns)
      @now_ns = nanoseconds(now_ns)
    end

    # Moves the clock forward by +nanos+.
    def advance(nanos)
      @now_ns += nanoseconds(nanos)
      self
    end

    private

    def nanoseconds(value)
      raise TypeError, 'a TestClock reads Integer nanoseconds' unless value.is_a?(Integer)
      raise RangeError, 'a TestClock reads no negative nanoseconds' if value.negative?

      value
    end
  end
end
