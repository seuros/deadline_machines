# frozen_string_literal: true

require 'test_helper'

module DeadlineMachines
  # The ambient scope bound per fiber.
  class AmbientTest < Test
    def test_nesting_tightens_and_restores_the_ambient_scope
      DeadlineMachines.with_deadline(seconds(5)) do |outer|
        DeadlineMachines.with_deadline(seconds(1)) do |inner|
          assert_same inner, DeadlineMachines.current
          assert_equal seconds(1), DeadlineMachines.remaining
        end

        assert_same outer, DeadlineMachines.current
        assert_equal seconds(5), DeadlineMachines.remaining
      end

      assert_nil DeadlineMachines.current
    end

    def test_a_nested_scope_without_a_limit_inherits_the_budget_and_cancellation
      DeadlineMachines.with_deadline(seconds(5)) do |outer|
        DeadlineMachines.with_deadline do |inner|
          refute_same outer, inner
          assert_equal seconds(5), inner.remaining
          outer.cancel('client hung up')

          assert_raises(Cancelled) { inner.check! }
        end
      end
    end

    def test_a_root_scope_without_a_limit_never_runs_out
      DeadlineMachines.with_deadline do |scope|
        assert_nil scope.remaining
        assert_same @clock, scope.clock
      end
    end

    def test_a_nested_scope_cannot_widen_the_budget
      DeadlineMachines.with_deadline(seconds(1)) do
        DeadlineMachines.with_deadline(seconds(60)) do
          assert_equal seconds(1), DeadlineMachines.remaining
        end
      end
    end

    def test_the_ambient_scope_is_restored_when_the_body_raises
      assert_raises(RuntimeError) do
        DeadlineMachines.with_deadline(seconds(5)) { raise 'boom' }
      end

      assert_nil DeadlineMachines.current
    end

    def test_checks_outside_a_scope_are_free_but_still_typed
      assert_nil DeadlineMachines.check!
      assert_nil DeadlineMachines.remaining
      assert_equal seconds(7), DeadlineMachines.clamp_sleep(seconds(7))
      assert_raises(TypeError) { DeadlineMachines.clamp_sleep(7.0) }
      assert_raises(TypeError) { DeadlineMachines.with_deadline(1.5) { nil } }
      assert_raises(RangeError) { DeadlineMachines.with_deadline(-1) { nil } }
    end

    def test_bind_takes_only_a_scope
      assert_raises(TypeError) { DeadlineMachines.bind(Deadline.never) { nil } }
      scope = Scope.never
      assert_equal :bound, DeadlineMachines.bind(scope) { |bound| bound.equal?(scope) && :bound }
    end

    def test_a_child_fiber_inherits_the_ambient_scope
      DeadlineMachines.with_deadline(seconds(5)) do |outer|
        assert_same outer, Fiber.new { DeadlineMachines.current }.resume
      end
    end

    def test_a_fiber_cannot_leak_its_scope_back_to_the_parent
      DeadlineMachines.with_deadline(seconds(5)) do |outer|
        Fiber.new { DeadlineMachines.bind(Scope.never) { nil } }.resume

        assert_same outer, DeadlineMachines.current
      end
    end

    def test_cancelling_the_ambient_scope_stops_the_next_checkpoint
      DeadlineMachines.with_deadline(seconds(5)) do |scope|
        scope.cancel('shutting down')

        error = assert_raises(Cancelled) { DeadlineMachines.check! }
        assert_equal 'shutting down', error.reason
      end
    end

    def test_an_exhausted_ambient_scope_stops_the_next_checkpoint
      DeadlineMachines.with_deadline(seconds(5)) do
        @clock.advance(seconds(6))

        assert_raises(Exceeded) { DeadlineMachines.check! }
        assert_equal 0, DeadlineMachines.clamp_sleep(seconds(1))
      end
    end
  end
end
