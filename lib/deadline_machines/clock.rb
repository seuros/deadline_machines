# frozen_string_literal: true

module DeadlineMachines
  module MonotonicClock
    def self.now_ns
      Process.clock_gettime(Process::CLOCK_MONOTONIC, :nanosecond)
    end
  end

  class TestClock
    attr_accessor :now_ns

    def initialize(now_ns: 0)
      @now_ns = now_ns
    end

    def advance(seconds)
      @now_ns += (seconds.to_f * 1_000_000_000).round
      self
    end
  end
end
