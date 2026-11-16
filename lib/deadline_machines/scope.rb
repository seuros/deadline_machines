# frozen_string_literal: true

module DeadlineMachines
  # A {Deadline} and a {CancelToken} as one value, the runtime peer of the
  # core's `Scope`. Where the core takes `now`, a scope reads its clock
  # (`clock` keyword, {DeadlineMachines.clock} by default): that clock and
  # the token's Ruby observers are what this runtime adds on top of the core.
  # Frozen; scopes sharing a token share cancellation.
  class Scope
    attr_reader :deadline, :token, :clock

    class << self
      # A scope under +deadline+ with a fresh root token.
      def new(deadline, clock: DeadlineMachines.clock)
        with_token(deadline, CancelToken.root, clock: clock)
      end

      # A scope with no time limit, cancellable only.
      def never(clock: DeadlineMachines.clock)
        new(Deadline.never, clock: clock)
      end

      # A scope under +deadline+ that shares +token+. The deadline is read
      # natively, as every Deadline operation reads one, and the scope keeps
      # the value rebuilt from it. The token and the clock are this runtime's
      # own Ruby objects and are checked here.
      def with_token(deadline, token, clock: DeadlineMachines.clock)
        deadline = DeadlineMachines.__send__(:__deadline_of, deadline)
        raise TypeError, 'token must be a DeadlineMachines::CancelToken' unless token.instance_of?(CancelToken)
        raise TypeError, 'clock must respond to now_ns' unless clock.respond_to?(:now_ns)

        scope = allocate
        scope.__send__(:initialize, deadline, token, clock)
        scope
      end
    end

    def initialize(deadline, token, clock)
      @deadline = deadline
      @token = token
      @clock = clock
      freeze
    end

    # Nanoseconds left, zero once spent, nil when there is no limit.
    def remaining
      @deadline.remaining(now)
    end

    def expired?
      @deadline.expired?(now)
    end

    def cancelled?
      @token.cancelled?
    end

    def done?
      cancelled? || expired?
    end

    # Why this scope or the nearest cancelled ancestor was cancelled.
    def reason
      @token.reason
    end

    # The checkpoint as a value: nil, or the refusal symbol. :cancelled
    # outranks :exceeded.
    def check
      return :cancelled if cancelled?

      @deadline.check(now)
    end

    # The cooperative checkpoint: raises Cancelled (which outranks) or
    # Exceeded, else returns self.
    def check!
      raise Cancelled.new(self, reason) if cancelled?
      raise Exceeded, self if expired?

      self
    end

    # The longest sleep up to +requested+ nanoseconds that still fits.
    def clamp_sleep(requested)
      @deadline.clamp_sleep(now, requested)
    end

    # See {CancelToken#cancel}: :propagated or :already_cancelled.
    def cancel(reason = nil)
      @token.cancel(reason)
    end

    # Calls the block with this scope once it is cancelled, at once if it
    # already is.
    def on_cancel
      @token.on_cancel { yield self }
      self
    end

    # A child scope whose deadline is the tighter of this one and +timeout+
    # nanoseconds from now, cancellable on its own. With a block, the child
    # is yielded and unlinked when the block exits; the block's value is
    # returned.
    def derive(timeout, &)
      below(@deadline.derive(now, timeout), &)
    end

    # A child scope with the same deadline, cancellable on its own; the block
    # form as {#derive}.
    def child(&)
      below(@deadline, &)
    end

    # This scope with the tighter of its deadline and +other+, sharing its
    # cancellation.
    def tighten(other)
      Scope.with_token(@deadline.tighten(other), @token, clock: @clock)
    end

    # This scope with its expiry pushed back by +by+ nanoseconds, sharing its
    # cancellation.
    def extend_by(by)
      Scope.with_token(@deadline.extend_by(by), @token, clock: @clock)
    end

    def inspect
      "#<#{self.class} #{state}>"
    end

    private

    def now
      @clock.now_ns
    end

    def below(deadline)
      return Scope.with_token(deadline, @token.child, clock: @clock) unless block_given?

      @token.child { |token| yield Scope.with_token(deadline, token, clock: @clock) }
    end

    def state
      return 'cancelled' if cancelled?

      left = remaining
      return 'unbounded' if left.nil?

      left.zero? ? 'expired' : "#{left}ns left"
    end
  end
end
