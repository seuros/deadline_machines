# frozen_string_literal: true

require 'test_helper'

module DeadlineMachines
  # The runtime scope: a deadline read against its clock, and a token.
  class ScopeTest < Test
    def test_derive_keeps_the_tighter_bound_and_child_keeps_the_deadline
      parent = Scope.new(Deadline.after(@clock.now_ns, seconds(5)))

      assert_equal seconds(1), parent.derive(seconds(1)).remaining
      assert_equal seconds(5), parent.derive(seconds(9)).remaining
      assert_equal parent.deadline, parent.child.deadline
      assert_equal seconds(2), Scope.never.derive(seconds(2)).remaining
      assert_nil Scope.never.child.remaining
    end

    def test_remaining_floors_at_zero_once_expired
      scope = Scope.new(Deadline.after(@clock.now_ns, seconds(1)))
      @clock.advance(seconds(10))

      assert_equal 0, scope.remaining
      assert_predicate scope, :expired?
      assert_predicate scope, :done?
      refute_predicate Scope.never, :done?
    end

    def test_check_answers_a_symbol_and_check_bang_raises_cancellation_first
      scope = Scope.new(Deadline.after(@clock.now_ns, seconds(1)))

      assert_nil scope.check
      assert_same scope, scope.check!
      @clock.advance(seconds(10))
      assert_equal :exceeded, scope.check
      error = assert_raises(Exceeded) { scope.check! }
      assert_same scope, error.scope

      assert_equal :propagated, scope.cancel('shutting down')
      assert_equal :cancelled, scope.check
      error = assert_raises(Cancelled) { scope.check! }
      assert_equal 'shutting down', error.reason
      assert_equal 'deadline cancelled: shutting down', error.message
    end

    def test_cancellation_reaches_children_derived_later_but_not_the_parent
      root = Scope.never
      child = root.derive(seconds(1))
      child.cancel

      refute_predicate root, :cancelled?
      root.cancel('client hung up')

      assert_predicate root.child, :cancelled?
      assert_equal 'client hung up', root.derive(seconds(1)).reason
      assert_equal :already_cancelled, root.child.cancel
    end

    def test_tighten_and_extend_by_share_cancellation
      scope = Scope.new(Deadline.after(@clock.now_ns, seconds(5)))
      tightened = scope.tighten(Deadline.after(@clock.now_ns, seconds(1)))
      extended = scope.extend_by(seconds(2))

      assert_equal seconds(1), tightened.remaining
      assert_equal seconds(7), extended.remaining
      assert_same scope.token, tightened.token

      tightened.cancel

      assert_predicate scope, :cancelled?
      assert_predicate extended, :cancelled?
    end

    def test_on_cancel_yields_the_scope_and_the_block_form_unlinks
      root = Scope.never
      seen = []
      root.on_cancel { |scope| seen << scope }
      assert_equal :done, root.derive(seconds(1)) { |child| child.on_cancel { seen << :child } && :done }

      root.cancel

      assert_equal [root], seen
    end

    def test_clamp_sleep_never_overruns_the_budget
      scope = Scope.new(Deadline.after(@clock.now_ns, seconds(1)))

      assert_equal 300, scope.clamp_sleep(300)
      @clock.advance(seconds(1) - 200)
      assert_equal 200, scope.clamp_sleep(300)
      @clock.advance(seconds(1))
      assert_equal 0, scope.clamp_sleep(300)
      assert_raises(TypeError) { scope.clamp_sleep(0.3) }
    end

    def test_a_scope_is_frozen_and_built_only_from_exact_parts
      scope = Scope.new(Deadline.never, clock: TestClock.new(5))

      assert_predicate scope, :frozen?
      assert_equal 5, scope.clock.now_ns
      assert_raises(TypeError) { Scope.new(nil) }
      assert_raises(TypeError) { Scope.with_token(Deadline.never, Object.new) }
      assert_raises(TypeError) { Scope.new(Deadline.never, clock: Object.new) }
      assert_equal '#<DeadlineMachines::Scope unbounded>', scope.inspect
      assert_equal '#<DeadlineMachines::Scope 7ns left>', Scope.new(Deadline.at(12), clock: scope.clock).inspect
      assert_equal '#<DeadlineMachines::Scope expired>', Scope.new(Deadline.at(5), clock: scope.clock).inspect
    end

    # The deadline is read natively, as `tighten` reads one: a redefined
    # `instance_of?` decides nothing, either way, and the message is the
    # binding's own.
    def test_a_scope_reads_its_deadline_natively
      impostor = Object.new
      impostor.define_singleton_method(:instance_of?) { |_| true }
      impostor.define_singleton_method(:expires_at) { nil }
      error = assert_raises(TypeError) { Scope.new(impostor) }
      assert_equal 'expected a DeadlineMachines::Deadline, not Object', error.message
      expected = assert_raises(TypeError) { Deadline.never.tighten(nil) }
      error = assert_raises(TypeError) { Scope.new(nil) }
      assert_equal expected.message, error.message

      deadline = Deadline.at(12)
      kept = disowned { Scope.new(deadline, clock: TestClock.new(5)) }

      assert_equal 7, kept.remaining
      assert_equal deadline, kept.deadline
    end

    # A singleton class opened on a genuine value changes nothing, and the
    # scope keeps a value rebuilt from the struct, not the one it was handed.
    def test_a_scope_keeps_a_rebuilt_deadline
      deadline = Deadline.at(12).tap(&:singleton_class)
      kept = Scope.new(deadline, clock: TestClock.new(5)).deadline

      assert_equal Deadline.at(12), kept
      refute_same deadline, kept
    end

    def test_a_marshal_edited_deadline_gets_into_no_scope
      forged = Marshal.load(Marshal.dump(Deadline.at(1)).sub("i\x06".b, "i\xFA".b))

      assert_equal(-1, forged.expires_at)
      assert_raises(RangeError) { Scope.new(forged) }
      assert_raises(RangeError) { Scope.with_token(forged, CancelToken.root) }
    end

    def test_with_token_shares_an_existing_token
      token = CancelToken.root
      scope = Scope.with_token(Deadline.never, token.child)

      token.cancel

      assert_predicate scope, :cancelled?
    end

    def test_test_clock_moves_forward_in_integer_nanoseconds_only
      clock = TestClock.new
      clock.now_ns = 10

      assert_equal 15, clock.advance(5).now_ns
      assert_raises(TypeError) { clock.advance(0.5) }
      assert_raises(RangeError) { clock.advance(-1) }
      assert_raises(TypeError) { TestClock.new(1.0) }
      assert_kind_of Integer, MonotonicClock.now_ns
    end

    private

    # Runs the block with Deadline#instance_of? answering false and the
    # reader raising if called.
    def disowned
      Deadline.alias_method(:__real_instance_of?, :instance_of?)
      Deadline.define_method(:instance_of?) { |_| false }
      Deadline.alias_method(:__honest, :expires_at)
      Deadline.define_method(:expires_at) { raise 'expires_at reader called' }
      yield
    ensure
      Deadline.alias_method(:instance_of?, :__real_instance_of?)
      Deadline.remove_method(:__real_instance_of?)
      Deadline.alias_method(:expires_at, :__honest)
      Deadline.remove_method(:__honest)
    end
  end
end
