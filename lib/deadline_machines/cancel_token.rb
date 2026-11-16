# frozen_string_literal: true

module DeadlineMachines
  # A node in a cancellation tree, the runtime peer of the core's
  # `CancelToken`: cancellation flows down and never up, a child of a
  # cancelled token is born cancelled, and observers registered with
  # {#on_cancel} fire once. Pure Ruby, because it holds Ruby callbacks.
  class CancelToken
    class << self
      # A token with no parent.
      def root
        new(nil)
      end

      private :new
    end

    def initialize(parent)
      @parent = parent
      @cancelled = false
      @detached = false
      @reason = nil
      @children = ObjectSpace::WeakMap.new
      @retained = {}
      @callbacks = []
      @mutex = parent ? parent.mutex : Mutex.new
    end

    # Whether this token or an ancestor was cancelled.
    def cancelled?
      @cancelled || !!@parent&.cancelled?
    end

    # Why this token, or the nearest cancelled ancestor, was cancelled.
    def reason
      @reason || @parent&.reason
    end

    # Cancels this token and every descendant, now or later, then tells every
    # observer. Answers :propagated, or :already_cancelled when this token or
    # an ancestor already was: that repeat changes nothing, +reason+ included
    # (read {#reason} for the one that stands). When observers raise, all of
    # them still run and the first error is raised afterwards.
    def cancel(reason = nil)
      propagate(reason, true) ? :propagated : :already_cancelled
    end

    # Calls the block with this token once it is cancelled, at once if it
    # already is.
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

    # A token cancelled whenever this one is, cancellable on its own. With a
    # block, the child is yielded and unlinked from this token's bookkeeping
    # when the block exits; the block's value is returned.
    def child
      token = CancelToken.__send__(:new, self)
      @mutex.synchronize { @children[token] = true }
      return token unless block_given?

      begin
        yield token
      ensure
        @mutex.synchronize { detach(token) }
      end
    end

    def inspect
      "#<#{self.class} #{cancelled? ? 'cancelled' : 'live'}>"
    end

    protected

    attr_reader :mutex
    attr_writer :detached

    # Sets this token's own flag and cascades. False when it was already set
    # (a racing cancel got there first) or, for a cancel called on this token
    # (+inherited+), when an ancestor already was: decided under the tree's
    # one lock, so the answer and the effect agree.
    def propagate(reason, inherited)
      callbacks, children = @mutex.synchronize do
        return false if @cancelled || (inherited && @parent&.cancelled?)

        @cancelled = true
        @reason = reason
        @parent&.drop(self)
        [@callbacks.dup, @children.keys]
      end

      failures = []
      children.each { |token| notify(failures) { token.propagate(reason, false) } }
      callbacks.each { |callback| notify(failures) { callback.call(self) } }
      raise failures.first unless failures.empty?

      true
    end

    # A child token is the caller's to keep, so the parent follows it weakly.
    # Something to deliver is the exception, and it has to be held all the
    # way to the root: an intermediate token nobody else references is the
    # one link whose collection would strand the delivery below it.
    #
    # Holding and dropping walk the chain, so they cannot be made atomic out
    # of one lock per link: every token in a tree shares the root's lock, and
    # both run inside a section the caller already holds.
    def hold(token)
      @retained[token] = true
      @parent.hold(self) if @parent && !@detached
    end

    def drop(token)
      @retained.delete(token)
      @parent.drop(self) if @parent && !pending?
    end

    # A block that has ended owns its child outright: the tree will not reach
    # it again, so the parent holds nothing for it and an observer registered
    # afterwards is the caller's own to keep alive.
    def detach(token)
      @children.delete(token)
      token.detached = true
      drop(token)
    end

    # Whether anything is still owed through this token.
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
  end
end
