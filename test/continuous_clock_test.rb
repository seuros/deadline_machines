# frozen_string_literal: true

require_relative 'test_helper'

module DeadlineMachines
  class ContinuousClockTest < Test
    SUPPORTED = /linux|darwin|freebsd/

    def test_readings_are_integer_nanoseconds_that_never_go_backwards
      require_continuous_clock
      last = ContinuousClock.now_ns

      assert_kind_of Integer, last
      1_000.times do
        now = ContinuousClock.now_ns

        assert_operator now, :>=, last
        last = now
      end
    end

    def test_keeps_pace_with_the_monotonic_clock_while_awake
      require_continuous_clock
      continuous = ContinuousClock.now_ns
      monotonic = MonotonicClock.now_ns
      sleep 0.01

      assert_operator ContinuousClock.now_ns - continuous, :>=, (MonotonicClock.now_ns - monotonic) - 5_000_000
    end

    def test_drives_a_scope
      require_continuous_clock
      scope = Scope.new(Deadline.after(ContinuousClock.now_ns, seconds(30)), clock: ContinuousClock)

      assert_nil scope.check
    end

    def test_is_undefined_where_no_clock_counts_suspend
      assert_equal RUBY_PLATFORM.match?(SUPPORTED), DeadlineMachines.const_defined?(:ContinuousClock)
    end

    private

    def require_continuous_clock
      skip 'no clock on this platform counts suspend' unless DeadlineMachines.const_defined?(:ContinuousClock)
    end
  end
end
