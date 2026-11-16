# frozen_string_literal: true

module DeadlineMachines
  # What a checkpoint raises. Refusals as values are the symbols of
  # {REFUSALS}; these exist so `check!` can unwind cooperative work.
  class Error < StandardError; end

  # The budget of +scope+ ran out.
  class Exceeded < Error
    attr_reader :scope

    def initialize(scope)
      @scope = scope
      super('deadline exceeded')
    end
  end

  # +scope+ or an ancestor was cancelled, for +reason+.
  class Cancelled < Error
    attr_reader :scope, :reason

    def initialize(scope, reason = nil)
      @scope = scope
      @reason = reason
      super(reason ? "deadline cancelled: #{reason}" : 'deadline cancelled')
    end
  end
end
