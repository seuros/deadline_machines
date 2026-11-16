# frozen_string_literal: true

$LOAD_PATH.unshift File.expand_path('../lib', __dir__)

require 'deadline_machines'
require 'minitest/autorun'

module DeadlineMachines
  class Test < Minitest::Test
    # One second, in the nanoseconds every time in this gem is.
    SECOND = 1_000_000_000

    def setup
      @clock = TestClock.new
      DeadlineMachines.clock = @clock
    end

    def teardown
      DeadlineMachines.clock = nil
    end

    private

    def seconds(count)
      count * SECOND
    end
  end
end
