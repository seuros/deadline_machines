# frozen_string_literal: true

$LOAD_PATH.unshift File.expand_path('../lib', __dir__)

require 'deadline_machines'
require 'minitest/autorun'

module DeadlineMachines
  class Test < Minitest::Test
    def setup
      @clock = TestClock.new
      DeadlineMachines.clock = @clock
    end

    def teardown
      DeadlineMachines.clock = nil
    end
  end
end
