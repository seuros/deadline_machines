# frozen_string_literal: true

module DeadlineMachines
  class Deadline
    NS_PER_SECOND = 1_000_000_000

    attr_reader :expires_at_ns, :parent, :clock, :reason

    class << self
      def after(seconds, clock: DeadlineMachines.clock)
        new(expires_at_ns: clock.now_ns + to_ns(seconds), clock: clock)
      end

      def never(clock: DeadlineMachines.clock)
        new(expires_at_ns: nil, clock: clock)
      end

      def to_ns(seconds)
        (seconds.to_f * NS_PER_SECOND).round
      end
    end

    def initialize(expires_at_ns:, clock:, parent: nil)
      @expires_at_ns = expires_at_ns
      @clock = clock
      @parent = parent
      @cancelled = false
      @reason = nil
      @children = []
      @callbacks = []
      @mutex = Mutex.new
    end

    def remaining_ns
      return nil unless @expires_at_ns

      left = @expires_at_ns - @clock.now_ns
      left.negative? ? 0 : left
    end

    def remaining
      ns = remaining_ns
      ns ? ns / NS_PER_SECOND.to_f : Float::INFINITY
    end

    def expired?
      remaining_ns == 0
    end

    def cancelled?
      @cancelled || !!@parent&.cancelled?
    end

    def done?
      cancelled? || expired?
    end

    def check!
      raise Cancelled.new(self, cancel_reason) if cancelled?
      raise Exceeded, self if expired?

      self
    end

    def clamp_sleep(seconds)
      left = remaining
      seconds < left ? seconds : left
    end

    def cancel(reason = nil)
      callbacks, children = @mutex.synchronize do
        return self if @cancelled

        @cancelled = true
        @reason = reason
        [@callbacks.dup, @children.dup]
      end

      children.each { |child| child.cancel(reason) }
      callbacks.each { |callback| callback.call(self) }
      self
    end

    def on_cancel(&block)
      fire = @mutex.synchronize do
        @callbacks << block unless cancelled?
        cancelled?
      end

      block.call(self) if fire
      self
    end

    def derive(seconds = nil)
      child = self.class.new(expires_at_ns: child_expiry_ns(seconds), clock: @clock, parent: self)
      @mutex.synchronize { @children << child }
      return child unless block_given?

      begin
        yield child
      ensure
        @mutex.synchronize { @children.delete(child) }
      end
    end

    def inspect
      state = cancelled? ? 'cancelled' : (expired? ? 'expired' : format('%.3fs left', remaining))
      "#<#{self.class} #{state}>"
    end

    protected

    def cancel_reason
      @reason || @parent&.cancel_reason
    end

    private

    def child_expiry_ns(seconds)
      return @expires_at_ns unless seconds

      candidate = @clock.now_ns + self.class.to_ns(seconds)
      @expires_at_ns ? [candidate, @expires_at_ns].min : candidate
    end
  end
end
