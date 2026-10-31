# frozen_string_literal: true

module DeadlineMachines
  class Error < StandardError; end

  class Exceeded < Error
    attr_reader :deadline

    def initialize(deadline)
      @deadline = deadline
      super('deadline exceeded')
    end
  end

  class Cancelled < Error
    attr_reader :deadline, :reason

    def initialize(deadline, reason = nil)
      @deadline = deadline
      @reason = reason
      super(reason ? "deadline cancelled: #{reason}" : 'deadline cancelled')
    end
  end
end
