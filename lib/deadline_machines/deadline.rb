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
        (seconds.to_r * NS_PER_SECOND).round
      end
    end

    def initialize(expires_at_ns:, clock:, parent: nil)
      @expires_at_ns = expires_at_ns
      @clock = clock
      @parent = parent
      @cancelled = false
      @detached = false
      @reason = nil
      @children = ObjectSpace::WeakMap.new
      @retained = {}
      @callbacks = []
      @mutex = parent ? parent.mutex : Mutex.new
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
        @parent&.drop(self)
        [@callbacks.dup, @children.keys]
      end

      failures = []
      children.each { |child| notify(failures) { child.cancel(reason) } }
      callbacks.each { |callback| notify(failures) { callback.call(self) } }
      raise failures.first unless failures.empty?

      self
    end

    def on_cancel(&block)
      already = @mutex.synchronize do
        settled = cancelled?
        unless settled
          @callbacks << block
          @parent&.hold(self) unless @detached
        end
        settled
      end

      block.call(self) if already
      self
    end

    def derive(seconds = nil)
      child = self.class.new(expires_at_ns: child_expiry_ns(seconds), clock: @clock, parent: self)
      @mutex.synchronize { @children[child] = true }
      return child unless block_given?

      begin
        yield child
      ensure
        @mutex.synchronize { detach(child) }
      end
    end

    def inspect
      state = cancelled? ? 'cancelled' : (expired? ? 'expired' : format('%.3fs left', remaining))
      "#<#{self.class} #{state}>"
    end

    protected

    attr_reader :mutex
    attr_writer :detached

    # A derived deadline is the caller's to keep, so the parent follows it
    # weakly. Something to deliver is the exception, and it has to be held all
    # the way to the root: an intermediate deadline nobody else references is
    # the one link whose collection would strand the delivery below it.
    #
    # Holding and dropping walk the chain, so they cannot be made atomic out of
    # one lock per link: every deadline in a tree shares the root's lock, and
    # both run inside a section the caller already holds.
    def hold(child)
      @retained[child] = true
      @parent&.hold(self)
    end

    def drop(child)
      @retained.delete(child)
      @parent.drop(self) if @parent && !pending?
    end

    # A scope that has ended owns its child outright: the tree will not reach
    # it again, so the parent holds nothing for it and an observer registered
    # afterwards is the caller's own to keep alive.
    def detach(child)
      @children.delete(child)
      child.detached = true
      drop(child)
    end

    # Whether anything is still owed through this deadline.
    def pending?
      !@cancelled && !(@callbacks.empty? && @retained.empty?)
    end

    private

    # Every observer and descendant is told, then the first failure is raised.
    # One raising observer must not silence its peers.
    def notify(failures)
      yield
    rescue StandardError => e
      failures << e
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
